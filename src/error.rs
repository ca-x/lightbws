use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("csrf validation failed")]
    Csrf,
    #[error("rate limited")]
    RateLimited,
    #[error("validation failed")]
    Validation(String),
    #[error("internal error")]
    Internal(#[source] anyhow::Error),
}

impl AppError {
    pub fn internal(error: impl Into<anyhow::Error>) -> Self {
        Self::Internal(error.into())
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authentication required".to_owned(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Permission denied".to_owned(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Resource not found".to_owned(),
            ),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "CONFLICT",
                "Resource conflict".to_owned(),
            ),
            Self::Csrf => (
                StatusCode::FORBIDDEN,
                "CSRF",
                "Request verification failed".to_owned(),
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Too many requests".to_owned(),
            ),
            Self::Validation(message) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION", message),
            Self::Internal(error) => {
                tracing::error!(error = %error, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL",
                    "Internal server error".to_owned(),
                )
            }
        };
        let mut response = (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("60"),
            );
        }
        response
    }
}
