use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};

use crate::{
    error::AppError,
    state::AppState,
    tag_operations::OperationResponse,
    trash::{
        self, TrashApplyRequest, TrashEntryResponse, TrashPreviewRequest, TrashPurgeApplyRequest,
        TrashPurgePreviewRequest, TrashPurgeResponse,
    },
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/trash", get(list))
        .route("/api/trash/preview", post(preview))
        .route("/api/trash/apply", post(apply))
        .route("/api/trash/restore", post(restore))
        .route("/api/trash/purge/preview", post(preview_purge))
        .route("/api/trash/purge/apply", post(apply_purge))
}

#[utoipa::path(get, path = "/api/trash", tag = "trash", security(("bearerAuth" = [])), responses((status = 200, body = [TrashEntryResponse])))]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TrashEntryResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(trash::list(&state).await?))
}

#[utoipa::path(post, path = "/api/trash/preview", tag = "trash", request_body = TrashPreviewRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TrashPreviewRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(trash::preview(&state, user_id, request).await?))
}

#[utoipa::path(post, path = "/api/trash/apply", tag = "trash", request_body = TrashApplyRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TrashApplyRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(trash::apply(&state, request).await?))
}

#[utoipa::path(post, path = "/api/trash/restore", tag = "trash", request_body = TrashApplyRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn restore(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TrashApplyRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(trash::restore(&state, request).await?))
}

#[utoipa::path(post, path = "/api/trash/purge/preview", tag = "trash", request_body = TrashPurgePreviewRequest, security(("bearerAuth" = [])), responses((status = 200, body = TrashPurgeResponse)))]
pub async fn preview_purge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TrashPurgePreviewRequest>,
) -> Result<Json<TrashPurgeResponse>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(trash::preview_purge(&state, user_id, request).await?))
}

#[utoipa::path(post, path = "/api/trash/purge/apply", tag = "trash", request_body = TrashPurgeApplyRequest, security(("bearerAuth" = [])), responses((status = 200, body = TrashPurgeResponse)))]
pub async fn apply_purge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TrashPurgeApplyRequest>,
) -> Result<Json<TrashPurgeResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(trash::apply_purge(&state, request).await?))
}
