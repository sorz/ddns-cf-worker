mod extract;

use axum::{response::IntoResponse, routing::get, Router};
use axum_extra::{extract::Query, TypedHeader};
use tower_service::Service;
use worker::{event, Context, Env, HttpRequest};

use crate::extract::{CfConnectingIp, IpAddrs};

fn router() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/update", get(update))
}

#[event(start)]
fn start() {
    console_log::init_with_level(log::Level::Debug).unwrap();
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    Ok(router().call(req).await?)
}

pub async fn root() -> &'static str {
    "Hello Axum!"
}

async fn update(
    Query(IpAddrs { ip }): Query<IpAddrs>,
    TypedHeader(CfConnectingIp(client_ip)): TypedHeader<CfConnectingIp>,
) -> impl IntoResponse {
    log::debug!("ip {:?}", ip);
    log::debug!("client ip {:?}", client_ip);
    "ok".into_response()
}
