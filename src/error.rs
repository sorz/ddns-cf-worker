use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures::channel::oneshot;
use thiserror::Error;

pub(crate) type UpdateResult<T> = Result<T, UpdateError>;

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("Cloudflare Worker runtime error: {0}")]
    Worker(#[from] worker::Error),

    #[error("KV store error: {0}")]
    Kv(#[from] worker::kv::KvError),

    #[error("Cloudflare SDK error: {0}")]
    CloudflareSdk(#[from] cloudflare::framework::Error),

    #[error("Cloudflare API error: {0}")]
    CloudflareApi(#[from] cloudflare::framework::response::ApiFailure),

    #[error("Task canceled: {0}")]
    Canceled(#[from] oneshot::Canceled),

    #[error("hostname/password incorrect")]
    Unauthorized,
}

impl IntoResponse for UpdateError {
    fn into_response(self) -> Response {
        let message = self.to_string();
        let status_code = match self {
            Self::Worker(_)
            | Self::Kv(_)
            | Self::CloudflareSdk(_)
            | Self::CloudflareApi(_)
            | Self::Canceled(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
        };
        (status_code, message).into_response()
    }
}
