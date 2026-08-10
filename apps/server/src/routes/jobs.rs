use std::{convert::Infallible, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use serde::Deserialize;
use tokio_stream::Stream;
use uuid::Uuid;

use crate::{
    error::{AppError, Problem},
    jobs::{JobEvent, JobResponse, fetch_job, list_jobs, set_status},
    organizer::OrganizerApplyRequest,
    scanner,
    state::AppState,
    tag_operations::ApplyOperationRequest,
    trash::TrashApplyRequest,
};

use super::auth::require_user_id;

#[derive(Debug, Deserialize)]
pub struct JobListQuery {
    pub limit: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/jobs", get(list))
        .route("/api/jobs/{id}", get(get_one))
        .route("/api/jobs/{id}/pause", post(pause))
        .route("/api/jobs/{id}/resume", post(resume))
        .route("/api/jobs/{id}/cancel", post(cancel))
        .route("/api/jobs/{id}/retry-failed", post(retry_failed))
}

#[utoipa::path(
    get,
    path = "/api/jobs",
    tag = "jobs",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "任务列表", body = [JobResponse]))
)]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<JobListQuery>,
) -> Result<Json<Vec<JobResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(
        list_jobs(&state.pool, query.limit.unwrap_or(50)).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/jobs/{id}",
    tag = "jobs",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path, description = "Job ID")),
    responses(
        (status = 200, description = "任务详情", body = JobResponse),
        (status = 404, description = "任务不存在", body = Problem)
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(fetch_job(&state.pool, id).await?))
}

#[utoipa::path(
    post,
    path = "/api/jobs/{id}/pause",
    tag = "jobs",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "任务已暂停", body = JobResponse))
)]
pub async fn pause(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(
        set_status(&state, id, &["queued", "running"], "paused").await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/jobs/{id}/resume",
    tag = "jobs",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "任务已恢复", body = JobResponse))
)]
pub async fn resume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let job = set_status(&state, id, &["paused", "interrupted"], "queued").await?;
    match job.kind.as_str() {
        "scrape" => crate::scraper::spawn_batch_job(state, id),
        "analyze" => crate::duplicates::spawn_job(state, id),
        _ => scanner::spawn_job(state, id),
    }
    Ok(Json(job))
}

#[utoipa::path(
    post,
    path = "/api/jobs/{id}/cancel",
    tag = "jobs",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "已请求取消", body = JobResponse))
)]
pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let current = fetch_job(&state.pool, id).await?;
    let job = if current.status == "running" {
        set_status(&state, id, &["running"], "cancel_requested").await?
    } else {
        set_status(
            &state,
            id,
            &["queued", "paused", "interrupted"],
            "cancelled",
        )
        .await?
    };
    Ok(Json(job))
}

#[utoipa::path(
    post,
    path = "/api/jobs/{id}/retry-failed",
    tag = "jobs",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "失败项重新入队", body = JobResponse))
)]
pub async fn retry_failed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let current = fetch_job(&state.pool, id).await?;
    if !matches!(current.status.as_str(), "completed_with_errors" | "failed") {
        return Err(AppError::Conflict(
            "只有失败或部分失败的任务可以重试失败项".to_owned(),
        ));
    }
    if current.kind != "scan" {
        match current.kind.as_str() {
            "tag_edit" => {
                crate::tag_operations::retry_failed(
                    &state,
                    ApplyOperationRequest {
                        operation_id: id,
                        confirmation: "APPLY".to_owned(),
                    },
                )
                .await?;
            }
            "organize" => {
                crate::organizer::retry_failed(
                    &state,
                    OrganizerApplyRequest {
                        operation_id: id,
                        confirmation: "APPLY".to_owned(),
                    },
                )
                .await?;
            }
            "trash" => {
                crate::trash::retry_failed(
                    &state,
                    TrashApplyRequest {
                        operation_id: id,
                        confirmation: "TRASH".to_owned(),
                    },
                )
                .await?;
            }
            "scrape" => {
                crate::scraper::retry_batch_job(&state, id).await?;
            }
            "analyze" => {
                crate::duplicates::retry_job(&state, id).await?;
            }
            "lyrics" => {
                crate::lyrics::retry_job(&state, id).await?;
            }
            _ => return Err(AppError::Conflict("未知任务类型不能自动重试".to_owned())),
        }
        return Ok(Json(fetch_job(&state.pool, id).await?));
    }
    let now = chrono::Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = 'pending', error_code = NULL, message = NULL, updated_at = ? WHERE job_id = ? AND status = 'failed'",
    )
    .bind(now)
    .bind(id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE jobs SET status = 'queued', processed_items = success_items + skipped_items, failed_items = 0, error_message = NULL, finished_at = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    let job = fetch_job(&state.pool, id).await?;
    scanner::spawn_job(state, id);
    Ok(Json(job))
}

pub async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    require_user_id(&headers, &state)?;
    let mut receiver = state.events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_owned());
                    yield Ok(Event::default().event(event.event).data(payload));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

#[allow(dead_code)]
fn _schema_anchor(_: JobEvent) {}
