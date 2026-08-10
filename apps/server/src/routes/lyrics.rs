use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use super::auth::require_user_id;
use crate::{
    error::AppError,
    lyrics::{self, ApplyLyricsRequest, LyricsRecord, LyricsSearchRequest, LyricsSearchResponse},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tracks/{id}/lyrics", get(list))
        .route("/api/lyrics/search", post(search))
        .route("/api/lyrics/apply", post(apply))
}
#[utoipa::path(get, path = "/api/tracks/{id}/lyrics", tag = "lyrics", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = Vec<LyricsRecord>)))]
async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LyricsRecord>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(lyrics::list(&state, id).await?))
}
#[utoipa::path(post, path = "/api/lyrics/search", tag = "lyrics", request_body = LyricsSearchRequest, security(("bearerAuth" = [])), responses((status = 200, body = LyricsSearchResponse)))]
async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LyricsSearchRequest>,
) -> Result<Json<LyricsSearchResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(lyrics::search(&state, request).await?))
}
#[utoipa::path(post, path = "/api/lyrics/apply", tag = "lyrics", request_body = ApplyLyricsRequest, security(("bearerAuth" = [])), responses((status = 200, body = LyricsRecord)))]
async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApplyLyricsRequest>,
) -> Result<Json<LyricsRecord>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(lyrics::apply(&state, request).await?))
}
