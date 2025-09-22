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
use cloudflare::endpoints::dns::dns::DnsContent;
use constant_time_eq::constant_time_eq;
use futures::channel::oneshot;
use itertools::{EitherOrBoth, Itertools};
use tower_service::Service;
use wasm_bindgen_futures::spawn_local;
use worker::{event, Context, Env, HttpRequest};

use crate::{
    dns::ZoneClient,
    error::{UpdateError, UpdateResult},
    extract::{CfConnectingIp, Credential, IpAddrs},
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
        .route("/update", get(update))
        .with_state(env)
        .call(req)
        .await?;
    Ok(resp)
}

async fn update(
    State(env): State<Env>,
    Query(IpAddrs { ip, myip }): Query<IpAddrs>,
    TypedHeader(CfConnectingIp(client_ip)): TypedHeader<CfConnectingIp>,
    auth_header: Option<TypedHeader<Authorization<Basic>>>,
    Query(Credential { hostname, password }): Query<Credential>,
) -> impl IntoResponse {
    // Extract addresses
    let mut ips: HashSet<_> = ip.union(&myip).collect();
    if ips.is_empty() {
        // Fallback to client's IP address
        ips.insert(&client_ip);
    }

    // Extract credential
    let (hostname, password) = auth_header
        .map(|auth| (auth.username().to_string(), auth.password().to_string()))
        .unwrap_or((hostname, password));

    // Worker produces non-Send futures while axum requires Send handle
    // Spawn in local thread as a workaround
    let (tx, rx) = oneshot::channel();
    spawn_local(async move {
        let resp = handle_update_request(env, ip, hostname, password).await;

        tx.send(match resp {
            Ok(Action::Updated) => (StatusCode::OK, "success").into_response(),
            Ok(Action::NoChange) => (StatusCode::OK, "no-change").into_response(),
            Err(err) => err.into_response(),
        })
        .unwrap();
    });
    match rx.await {
        Ok(resp) => resp,
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Task canceled").into_response(),
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Updated,
    NoChange,
}

async fn handle_update_request(
    env: Env,
    mut ips: HashSet<IpAddr>,
    hostname: String,
    password: String,
) -> UpdateResult<Action> {
    // Normalize hostname
    let suffix = env.secret(DOMAIN_SUFFIX)?.to_string();
    let suffix = if suffix.starts_with('.') {
        suffix
    } else {
        format!(".{suffix}")
    };
    let hostname = hostname.trim_end_matches('.').trim_end_matches(&suffix);

    // Check credential
    if hostname.is_empty() || password.is_empty() {
        return Err(UpdateError::Unauthorized);
    }
    let correct_pwd = env
        .kv(KV_HOST_PASSWORD)?
        .get(hostname)
        .cache_ttl(KV_HOST_PASSWORD_CACHE_SECS)
        .text()
        .await?
        .ok_or(UpdateError::Unauthorized)?;
    if !constant_time_eq(password.as_bytes(), correct_pwd.as_bytes()) {
        return Err(UpdateError::Unauthorized);
    }

    // Check existing records
    let fqdn = format!("{hostname}{suffix}");
    let zone = ZoneClient::new(env).await?;
    let records: Vec<_> = zone
        .list_records(fqdn.clone())
        .await?
        .into_iter()
        .filter(|record| {
            let addr = match record.content {
                DnsContent::A { content } => IpAddr::V4(content),
                DnsContent::AAAA { content } => IpAddr::V6(content),
                _ => return false, // Ignore non-A/AAAA records
            };
            // Ignore if record matches updating request
            !ips.remove(&addr)
        })
        .collect();
    if ips.is_empty() {
        return Ok(Action::NoChange);
    }

    // Update records
    for pair in ips.into_iter().zip_longest(records) {
        match pair {
            EitherOrBoth::Both(ip_addr, record) => zone.update_record(&record, ip_addr).await?,
            EitherOrBoth::Left(ip_addr) => zone.create_record(&fqdn, ip_addr).await?,
            EitherOrBoth::Right(record) => zone.delete_record(&record).await?,
        }
    }
    Ok(Action::Updated)
}
