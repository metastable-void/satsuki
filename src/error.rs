//! Application error helpers and Axum integration.
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// Standard JSON error payload emitted by the API.
#[derive(Debug, Serialize)]
pub struct ErrorResponseBody {
    pub error: String,
}

/// Common error cases surfaced to HTTP clients.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("not found")]
    NotFound,

    #[error("internal server error")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    /// Convenience constructor for `400 Bad Request`.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }

    /// Convenience constructor for `409 Conflict`.
    pub fn conflict(msg: impl Into<String>) -> Self {
        AppError::Conflict(msg.into())
    }

    /// Wrap any error into `500 Internal Server Error`.
    pub fn internal<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        AppError::Internal(anyhow::Error::new(err))
    }

    /// Wrap an existing anyhow error into `AppError`.
    pub fn internal_anyhow(err: anyhow::Error) -> Self {
        AppError::Internal(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".into()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".into(),
            ),
        };

        let body = Json(ErrorResponseBody { error: msg });
        (status, body).into_response()
    }
}
