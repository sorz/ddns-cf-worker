mod extract;

use std::{collections::HashSet, net::IpAddr};

use axum::{extract::State, http::StatusCode, routing::get, Router};
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

use crate::extract::{CfConnectingIp, IpAddrs};

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
) -> (StatusCode, String) {
    // Worker produces non-Send futures while axum requires Send handle
    // Spawn in local thread as a workaround
    let (tx, rx) = oneshot::channel();
    spawn_local(async move {
        let resp = handle_update_request(env, ip, client_ip, auth).await;
        tx.send(resp).unwrap();
    });
    rx.await.unwrap()
}

macro_rules! tryit {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Internal server error: {:?}", err),
                )
            }
        }
    };
}

async fn handle_update_request(
    env: Env,
    ips: HashSet<IpAddr>,
    client_ip: IpAddr,
    auth: Basic,
) -> (StatusCode, String) {
    log::debug!("ip {:?}", ips);
    log::debug!("client ip {:?}", client_ip);

    // TODO: normalize hostname

    // Check credential
    let password = tryit!(env.kv(KV_HOST_PASSWORD))
        .get(auth.username())
        .cache_ttl(KV_HOST_PASSWORD_CACHE_SECS)
        .text()
        .await;
    match tryit!(password) {
        Some(pwd) if constant_time_eq(pwd.as_bytes(), auth.password().as_bytes()) => (),
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                "hostname/password incorrect".to_string(),
            )
        }
    }

    // TODO: udpate records

    (StatusCode::OK, "ok".to_string())
}
