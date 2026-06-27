use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("subscription limit reached")]
    LimitReached,
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::NotFound      => (StatusCode::NOT_FOUND,            "not_found"),
            AppError::Unauthorized  => (StatusCode::UNAUTHORIZED,         "unauthorized"),
            AppError::Forbidden     => (StatusCode::FORBIDDEN,            "forbidden"),
            AppError::LimitReached  => (StatusCode::TOO_MANY_REQUESTS,    "limit_reached"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST,          "bad_request"),
            AppError::Internal(_) | AppError::Db(_)
                                    => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        let body = Json(json!({ "error": msg, "detail": self.to_string() }));
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
