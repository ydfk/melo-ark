use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use uuid::Uuid;

use super::auth::require_user_id;
use crate::{
    duplicates::{self, AnalyzeRequest, DuplicateGroup, GroupQuery},
    error::AppError,
    jobs::JobResponse,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/duplicates/analyze", post(analyze))
        .route("/api/duplicates/groups", get(groups))
        .route("/api/duplicates/groups/{id}", get(group))
        .route("/api/duplicates/rebuild", post(rebuild))
}

#[utoipa::path(post, path = "/api/duplicates/analyze", tag = "duplicates", request_body = AnalyzeRequest, security(("bearerAuth" = [])), responses((status = 202, body = JobResponse)))]
async fn analyze(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AnalyzeRequest>,
) -> Result<(StatusCode, Json<JobResponse>), AppError> {
    require_user_id(&headers, &state)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(duplicates::create_job(&state, request).await?),
    ))
}
#[utoipa::path(get, path = "/api/duplicates/groups", tag = "duplicates", security(("bearerAuth" = [])), responses((status = 200, body = Vec<DuplicateGroup>)))]
async fn groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GroupQuery>,
) -> Result<Json<Vec<DuplicateGroup>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(
        duplicates::list_groups(&state, query.kind.as_deref()).await?,
    ))
}
#[utoipa::path(get, path = "/api/duplicates/groups/{id}", tag = "duplicates", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = DuplicateGroup)))]
async fn group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<DuplicateGroup>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(duplicates::get_group(&state, id).await?))
}
#[utoipa::path(post, path = "/api/duplicates/rebuild", tag = "duplicates", security(("bearerAuth" = [])), responses((status = 204)))]
async fn rebuild(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_user_id(&headers, &state)?;
    duplicates::rebuild_groups(&state).await?;
    Ok(StatusCode::NO_CONTENT)
}
