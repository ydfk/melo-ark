use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    error::AppError,
    state::AppState,
    tag_operations::{
        self, ApplyOperationRequest, OperationResponse, TagPreviewRequest, UndoOperationRequest,
    },
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tags/preview", post(preview))
        .route("/api/tags/apply", post(apply))
        .route("/api/tags/retry-failed", post(retry_failed))
        .route("/api/tags/undo", post(undo))
        .route("/api/operations/{id}", get(get_operation))
}

#[utoipa::path(post, path = "/api/tags/retry-failed", tag = "tags", request_body = ApplyOperationRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn retry_failed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApplyOperationRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(tag_operations::retry_failed(&state, request).await?))
}

#[utoipa::path(post, path = "/api/tags/undo", tag = "tags", request_body = UndoOperationRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn undo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UndoOperationRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(tag_operations::undo(&state, request).await?))
}

#[utoipa::path(post, path = "/api/tags/preview", tag = "tags", request_body = TagPreviewRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TagPreviewRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(
        tag_operations::preview(&state, user_id, request).await?,
    ))
}

#[utoipa::path(post, path = "/api/tags/apply", tag = "tags", request_body = ApplyOperationRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApplyOperationRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(tag_operations::apply(&state, request).await?))
}

#[utoipa::path(get, path = "/api/operations/{id}", tag = "operations", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn get_operation(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(tag_operations::get_operation(&state, id).await?))
}
