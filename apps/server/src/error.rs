use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub status: u16,
    pub title: &'static str,
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    RateLimited(String),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "请求处理失败");
        Self::Internal("服务器内部错误".to_owned())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, title) = match self {
            Self::BadRequest(_) => (StatusCode::UNPROCESSABLE_ENTITY, "Unprocessable Entity"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "Not Found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "Conflict"),
            Self::RateLimited(_) => (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
        };
        let problem = Problem {
            status: status.as_u16(),
            title,
            detail: self.to_string(),
        };
        (status, Json(problem)).into_response()
    }
}
