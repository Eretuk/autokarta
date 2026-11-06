//! API errors. The API returns stable machine-readable codes
//! (`{"error": "username_taken"}`); the frontend translates them.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not_found")]
    NotFound,
    #[error("invalid_credentials")]
    InvalidCredentials,
    #[error("too_many_attempts")]
    TooManyAttempts { retry_after_secs: u64 },
    /// Input failed validation; the payload is the error code.
    #[error("{0}")]
    Validation(&'static str),
    /// Request conflicts with existing data; the payload is the error code.
    #[error("{0}")]
    Conflict(&'static str),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Internal(e.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Unauthorized | AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::TooManyAttempts { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let code = match &self {
            AppError::Internal(_) => "internal".to_string(),
            other => other.to_string(),
        };
        let mut response = (status, Json(json!({ "error": code }))).into_response();
        if let AppError::TooManyAttempts { retry_after_secs } = self {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, retry_after_secs.into());
        }
        response
    }
}

/// Turns a unique-constraint violation into a 409 with a specific code.
/// `map` pairs a constraint (or unique index) name with the error code.
pub fn conflict_on(e: sqlx::Error, map: &[(&str, &'static str)]) -> AppError {
    if let sqlx::Error::Database(d) = &e {
        if d.is_unique_violation() {
            let name = d.constraint().unwrap_or_default();
            if let Some((_, code)) = map.iter().find(|(c, _)| *c == name) {
                return AppError::Conflict(code);
            }
        }
    }
    e.into()
}
