use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};

use super::auth::require_user_id;
use crate::{
    ai::{self, AiDuplicateRequest, AiRecommendation, AiRerankRequest, AiStatus},
    error::AppError,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ai/status", get(status))
        .route("/api/ai/duplicates/explain", post(explain))
        .route("/api/ai/scrape/rerank", post(rerank))
}
#[utoipa::path(get, path = "/api/ai/status", tag = "ai", security(("bearerAuth" = [])), responses((status = 200, body = AiStatus)))]
async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AiStatus>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(ai::status(&state)))
}
#[utoipa::path(post, path = "/api/ai/duplicates/explain", tag = "ai", request_body = AiDuplicateRequest, security(("bearerAuth" = [])), responses((status = 200, body = AiRecommendation)))]
async fn explain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AiDuplicateRequest>,
) -> Result<Json<AiRecommendation>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(ai::explain_duplicate(&state, request).await?))
}
#[utoipa::path(post, path = "/api/ai/scrape/rerank", tag = "ai", request_body = AiRerankRequest, security(("bearerAuth" = [])), responses((status = 200, body = AiRecommendation)))]
async fn rerank(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AiRerankRequest>,
) -> Result<Json<AiRecommendation>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(ai::rerank_candidates(&state, request).await?))
}
