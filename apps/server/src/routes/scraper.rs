use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use uuid::Uuid;

use super::auth::require_user_id;
use crate::{
    error::AppError,
    scraper::{
        self, BatchScrapeRequest, ProviderSetting, ScrapeApplyRequest, ScrapeCandidate,
        ScrapeSearchRequest, ScrapeSearchResponse, UpdateProviderRequest,
    },
    state::AppState,
    tag_operations::OperationResponse,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/providers", get(list_providers))
        .route("/api/providers/{id}", patch(update_provider))
        .route("/api/scrape/search", post(search))
        .route("/api/scrape/jobs", post(create_job))
        .route("/api/tracks/{id}/scrape-candidates", get(candidates))
        .route("/api/scrape/apply", post(apply))
}

#[utoipa::path(post, path = "/api/scrape/jobs", tag = "scraper", request_body = BatchScrapeRequest, security(("bearerAuth" = [])), responses((status = 202, body = crate::jobs::JobResponse)))]
async fn create_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BatchScrapeRequest>,
) -> Result<(axum::http::StatusCode, Json<crate::jobs::JobResponse>), AppError> {
    require_user_id(&headers, &state)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(scraper::create_batch_job(&state, request).await?),
    ))
}

#[utoipa::path(get, path = "/api/providers", tag = "providers", security(("bearerAuth" = [])), responses((status = 200, body = Vec<ProviderSetting>)))]
async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderSetting>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(scraper::list_providers(&state).await?))
}

#[utoipa::path(patch, path = "/api/providers/{id}", tag = "providers", params(("id" = String, Path)), request_body = UpdateProviderRequest, security(("bearerAuth" = [])), responses((status = 200, body = ProviderSetting)))]
async fn update_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderSetting>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(scraper::update_provider(&state, &id, request).await?))
}

#[utoipa::path(post, path = "/api/scrape/search", tag = "scraper", request_body = ScrapeSearchRequest, security(("bearerAuth" = [])), responses((status = 200, body = ScrapeSearchResponse)))]
async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScrapeSearchRequest>,
) -> Result<Json<ScrapeSearchResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(scraper::search(&state, request).await?))
}

#[utoipa::path(get, path = "/api/tracks/{id}/scrape-candidates", tag = "scraper", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = Vec<ScrapeCandidate>)))]
async fn candidates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ScrapeCandidate>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(scraper::list_candidates(&state, id).await?))
}

#[utoipa::path(post, path = "/api/scrape/apply", tag = "scraper", request_body = ScrapeApplyRequest, security(("bearerAuth" = [])), responses((status = 200, body = OperationResponse)))]
async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScrapeApplyRequest>,
) -> Result<Json<OperationResponse>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(
        scraper::apply_candidate(&state, user_id, request).await?,
    ))
}
