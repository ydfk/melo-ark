use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    error::{AppError, Problem},
    jobs::JobResponse,
    review::{
        self, ApplyReviewBatchRequest, MarkedReviewCount, ReviewBatchItemPage, ReviewBatchPreview,
        ReviewBatchPreviewRequest, ReviewItem, ReviewPage, ReviewSelection, UpdateReviewRequest,
    },
    state::AppState,
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/reviews", get(list))
        .route("/api/reviews/{id}", patch(update))
        .route("/api/reviews/marks/clear", post(clear_marks))
        .route("/api/reviews/batch/preview", post(preview_batch))
        .route("/api/reviews/batch/previews/{id}/items", get(preview_items))
        .route("/api/reviews/batch/apply", post(apply_batch))
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQuery {
    pub status: Option<String>,
    pub kind: Option<String>,
    pub marked: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/reviews",
    tag = "reviews",
    security(("bearerAuth" = [])),
    params(ReviewQuery),
    responses(
        (status = 200, body = ReviewPage),
        (status = 401, body = Problem)
    )
)]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewQuery>,
) -> Result<Json<ReviewPage>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(
        review::list(
            &state,
            query.status.as_deref(),
            query.kind.as_deref(),
            query.marked,
            query.page.unwrap_or(1),
            query.per_page.unwrap_or(25),
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/reviews/marks/clear",
    tag = "reviews",
    security(("bearerAuth" = [])),
    request_body = ReviewSelection,
    responses((status = 200, body = MarkedReviewCount), (status = 401, body = Problem))
)]
pub async fn clear_marks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(selection): Json<ReviewSelection>,
) -> Result<Json<MarkedReviewCount>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(review::clear_marks(&state, selection).await?))
}

#[utoipa::path(
    patch,
    path = "/api/reviews/{id}",
    tag = "reviews",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path)),
    request_body = UpdateReviewRequest,
    responses(
        (status = 200, body = ReviewItem),
        (status = 401, body = Problem),
        (status = 404, body = Problem)
    )
)]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateReviewRequest>,
) -> Result<Json<ReviewItem>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(review::update(&state, id, request).await?))
}

#[utoipa::path(
    post,
    path = "/api/reviews/batch/preview",
    tag = "reviews",
    security(("bearerAuth" = [])),
    request_body = ReviewBatchPreviewRequest,
    responses(
        (status = 200, body = ReviewBatchPreview),
        (status = 401, body = Problem)
    )
)]
pub async fn preview_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ReviewBatchPreviewRequest>,
) -> Result<Json<ReviewBatchPreview>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(review::preview_batch(&state, user_id, request).await?))
}

#[utoipa::path(
    get,
    path = "/api/reviews/batch/previews/{id}/items",
    tag = "reviews",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path), PageQuery),
    responses((status = 200, body = ReviewBatchItemPage), (status = 401, body = Problem))
)]
pub async fn preview_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<ReviewBatchItemPage>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    Ok(Json(
        review::list_preview_items(
            &state,
            user_id,
            id,
            query.page.unwrap_or(1),
            query.per_page.unwrap_or(25),
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/reviews/batch/apply",
    tag = "reviews",
    security(("bearerAuth" = [])),
    request_body = ApplyReviewBatchRequest,
    responses(
        (status = 202, body = JobResponse),
        (status = 401, body = Problem)
    )
)]
pub async fn apply_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApplyReviewBatchRequest>,
) -> Result<(StatusCode, Json<JobResponse>), AppError> {
    let user_id = require_user_id(&headers, &state)?;
    let job = review::apply_batch(state, user_id, request).await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}
