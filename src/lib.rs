mod dns;
mod error;
mod extract;

use std::{collections::HashSet, net::IpAddr};

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};
use axum_extra::{
    extract::Query,
    headers::{authorization::Basic, Authorization},
    TypedHeader,
};
use constant_time_eq::constant_time_eq;
use futures::channel::oneshot;
use tower_service::Service;
use wasm_bindgen_futures::spawn_local;
use worker::{event, Context, Env, HttpRequest};

use crate::{
    dns::ZoneClient,
    error::{UpdateError, UpdateResult},
    extract::{CfConnectingIp, IpAddrs},
};

static DOMAIN_SUFFIX: &str = "DOMAIN_SUFFIX";

static KV_HOST_PASSWORD: &str = "ddns_host_password";
static KV_HOST_PASSWORD_CACHE_SECS: u64 = 600;

#[event(start)]
fn start() {
    console_log::init_with_level(log::Level::Debug).unwrap();
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    let resp = Router::<Env>::new()
        .route("/", get(root))
        .route("/update", get(update))
        .with_state(env)
        .call(req)
        .await?;
    Ok(resp)
}

pub async fn root() -> &'static str {
    "Hello Axum!"
}

async fn update(
    State(env): State<Env>,
    Query(IpAddrs { ip }): Query<IpAddrs>,
    TypedHeader(CfConnectingIp(client_ip)): TypedHeader<CfConnectingIp>,
    TypedHeader(Authorization(auth)): TypedHeader<Authorization<Basic>>,
) -> impl IntoResponse {
    // Worker produces non-Send futures while axum requires Send handle
    // Spawn in local thread as a workaround
    let (tx, rx) = oneshot::channel();
    spawn_local(async move {
        let resp = handle_update_request(env, ip, client_ip, auth).await;

        tx.send(match resp {
            Ok(()) => (StatusCode::OK, "ok").into_response(),
            Err(err) => err.into_response(),
        })
        .unwrap();
    });
    match rx.await {
        Ok(resp) => resp,
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Task canceled").into_response(),
    }
}

async fn handle_update_request(
    env: Env,
    ips: HashSet<IpAddr>,
    client_ip: IpAddr,
    auth: Basic,
) -> UpdateResult<()> {
    log::debug!("ip {:?}", ips);
    log::debug!("client ip {:?}", client_ip);

    // Normalize hostname
    let suffix = env.secret(DOMAIN_SUFFIX)?.to_string();
    let suffix = if suffix.starts_with('.') {
        suffix
    } else {
        format!(".{suffix}")
    };
    let hostname = auth.username().trim_end_matches(&suffix);

    // Check credential
    let password = env
        .kv(KV_HOST_PASSWORD)?
        .get(hostname)
        .cache_ttl(KV_HOST_PASSWORD_CACHE_SECS)
        .text()
        .await?
        .ok_or(UpdateError::Unauthorized)?;
    if !constant_time_eq(password.as_bytes(), auth.password().as_bytes()) {
        return Err(UpdateError::Unauthorized);
    }

    // Update records
    let fqdn = format!("{hostname}{suffix}");
    let zone = ZoneClient::new(env).await?;
    let records = zone.list_records(fqdn.clone()).await?;
    log::debug!("resp {:?}", records);

    Ok(())
}
