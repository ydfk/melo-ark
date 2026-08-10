use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};

use crate::{
    error::AppError,
    organizer::{self, OrganizerApplyRequest, OrganizerPreviewRequest, OrganizerUndoRequest},
    state::AppState,
    tag_operations::OperationResponse,
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/organizer/preview", post(preview))
        .route("/api/organizer/apply", post(apply))
        .route("/api/organizer/retry-failed", post(retry_failed))
        .route("/api/organizer/undo", post(undo))
}

#[utoipa::path(post, path = "/api/organizer/retry-failed", tag = "organizer", request_body = OrganizerApplyRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn retry_failed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OrganizerApplyRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(organizer::retry_failed(&state, request).await?))
}

#[utoipa::path(post, path = "/api/organizer/preview", tag = "organizer", request_body = OrganizerPreviewRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OrganizerPreviewRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(organizer::preview(&state, user_id, request).await?))
}

#[utoipa::path(post, path = "/api/organizer/apply", tag = "organizer", request_body = OrganizerApplyRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OrganizerApplyRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(organizer::apply(&state, request).await?))
}

#[utoipa::path(post, path = "/api/organizer/undo", tag = "organizer", request_body = OrganizerUndoRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
pub async fn undo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OrganizerUndoRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(organizer::undo(&state, request).await?))
}
