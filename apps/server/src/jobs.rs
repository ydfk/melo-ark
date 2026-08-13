use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Clone, Debug, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobResponse {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub library_id: Option<Uuid>,
    pub parent_job_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub total_items: i64,
    pub processed_items: i64,
    pub success_items: i64,
    pub skipped_items: i64,
    pub failed_items: i64,
    pub current_item: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub items_per_second: Option<f64>,
    pub eta_seconds: Option<i64>,
}

#[derive(Clone, Debug, FromRow)]
struct JobRow {
    id: Uuid,
    kind: String,
    status: String,
    library_id: Option<Uuid>,
    parent_job_id: Option<Uuid>,
    source_type: Option<String>,
    source_id: Option<String>,
    total_items: i64,
    processed_items: i64,
    success_items: i64,
    skipped_items: i64,
    failed_items: i64,
    current_item: Option<String>,
    error_message: Option<String>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl JobRow {
    fn into_response(self) -> JobResponse {
        let elapsed_seconds = self
            .started_at
            .map(|started_at| self.finished_at.unwrap_or_else(Utc::now) - started_at);
        let items_per_second = elapsed_seconds
            .filter(|elapsed| elapsed.num_milliseconds() > 0 && self.processed_items > 0)
            .map(|elapsed| {
                self.processed_items as f64 / (elapsed.num_milliseconds() as f64 / 1_000.0)
            });
        let eta_seconds = if self.status == "running" {
            items_per_second.and_then(|speed| {
                let remaining = self.total_items.saturating_sub(self.processed_items);
                (speed > 0.0).then_some((remaining as f64 / speed).ceil() as i64)
            })
        } else {
            None
        };

        JobResponse {
            id: self.id,
            kind: self.kind,
            status: self.status,
            library_id: self.library_id,
            parent_job_id: self.parent_job_id,
            source_type: self.source_type,
            source_id: self.source_id,
            total_items: self.total_items,
            processed_items: self.processed_items,
            success_items: self.success_items,
            skipped_items: self.skipped_items,
            failed_items: self.failed_items,
            current_item: self.current_item,
            error_message: self.error_message,
            created_at: self.created_at,
            started_at: self.started_at,
            finished_at: self.finished_at,
            updated_at: self.updated_at,
            items_per_second,
            eta_seconds,
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobLogResponse {
    pub id: i64,
    pub job_id: Uuid,
    pub level: String,
    pub event_type: String,
    pub item_key: Option<String>,
    pub attempt: Option<i64>,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobLogPage {
    pub items: Vec<JobLogResponse>,
    pub next_before: Option<i64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<JobLogResponse>,
}

pub async fn recover_interrupted(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'interrupted', updated_at = ? WHERE status IN ('running', 'cancel_requested')",
    )
    .bind(Utc::now())
    .execute(pool)
    .await?;
    sqlx::query("UPDATE job_items SET status = 'pending', updated_at = ? WHERE status = 'running'")
        .bind(Utc::now())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cleanup_expired_logs(pool: &SqlitePool) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"DELETE FROM job_logs
           WHERE created_at < ?
             AND job_id IN (
               SELECT id FROM jobs
               WHERE status IN ('cancelled', 'completed', 'completed_with_errors', 'failed', 'interrupted')
             )"#,
    )
    .bind(Utc::now() - chrono::Duration::days(30))
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn create_scan_job(state: &AppState, library_id: Uuid) -> Result<JobResponse, AppError> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let inserted = sqlx::query(
        r#"INSERT INTO jobs
             (id, kind, status, library_id, source_type, source_id, created_at, updated_at)
           SELECT ?, 'scan', 'queued', ?, 'library', ?, ?, ?
           WHERE NOT EXISTS (
             SELECT 1 FROM jobs
             WHERE library_id = ? AND kind = 'scan' AND status = 'queued'
           )"#,
    )
    .bind(id)
    .bind(library_id)
    .bind(library_id.to_string())
    .bind(now)
    .bind(now)
    .bind(library_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if inserted.rows_affected() == 0 {
        let existing: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM jobs
               WHERE library_id = ? AND kind = 'scan'
                 AND status IN ('queued', 'paused', 'running', 'cancel_requested')
               ORDER BY CASE status WHEN 'queued' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END,
                        created_at DESC, rowid DESC
               LIMIT 1"#,
        )
        .bind(library_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        return fetch_job(&state.pool, existing).await;
    }

    let job = fetch_job(&state.pool, id).await?;
    record_log(
        state,
        id,
        "info",
        "queued",
        None,
        None,
        "扫描任务已加入队列",
    )
    .await?;
    emit(state, job.clone());
    Ok(job)
}

pub async fn fetch_job(pool: &SqlitePool, id: Uuid) -> Result<JobResponse, AppError> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT id, kind, status, library_id, parent_job_id, source_type, source_id,
               total_items, processed_items, success_items,
               skipped_items, failed_items, current_item, error_message, created_at,
               started_at, finished_at, updated_at
        FROM jobs WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?
    .map(JobRow::into_response)
    .ok_or_else(|| AppError::NotFound("任务不存在".to_owned()))
}

pub async fn list_jobs(pool: &SqlitePool, limit: i64) -> Result<Vec<JobResponse>, AppError> {
    let rows = sqlx::query_as::<_, JobRow>(
        r#"
        SELECT id, kind, status, library_id, parent_job_id, source_type, source_id,
               total_items, processed_items, success_items,
               skipped_items, failed_items, current_item, error_message, created_at,
               started_at, finished_at, updated_at
        FROM jobs ORDER BY created_at DESC, rowid DESC LIMIT ?
        "#,
    )
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;
    Ok(rows.into_iter().map(JobRow::into_response).collect())
}

pub async fn set_status(
    state: &AppState,
    id: Uuid,
    allowed: &[&str],
    status: &str,
) -> Result<JobResponse, AppError> {
    let current = fetch_job(&state.pool, id).await?;
    if !allowed.contains(&current.status.as_str()) {
        return Err(AppError::Conflict(format!(
            "任务处于 {} 状态，不能切换为 {status}",
            current.status
        )));
    }
    let now = Utc::now();
    sqlx::query("UPDATE jobs SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let job = fetch_job(&state.pool, id).await?;
    let message = status_message(status);
    record_log(state, id, "info", status, None, None, message).await?;
    emit(state, job.clone());
    Ok(job)
}

pub fn emit(state: &AppState, job: JobResponse) {
    let _ = state.events.send(JobEvent {
        event: "job.updated",
        job: Some(job),
        log: None,
    });
}

pub async fn record_log(
    state: &AppState,
    job_id: Uuid,
    level: &str,
    event_type: &str,
    item_key: Option<&str>,
    attempt: Option<i64>,
    message: &str,
) -> Result<JobLogResponse, AppError> {
    let created_at = Utc::now();
    let id = sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO job_logs
             (job_id, level, event_type, item_key, attempt, message, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           RETURNING id"#,
    )
    .bind(job_id)
    .bind(level)
    .bind(event_type)
    .bind(item_key)
    .bind(attempt)
    .bind(message)
    .bind(created_at)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let log = JobLogResponse {
        id,
        job_id,
        level: level.to_owned(),
        event_type: event_type.to_owned(),
        item_key: item_key.map(str::to_owned),
        attempt,
        message: message.to_owned(),
        created_at,
    };
    let _ = state.events.send(JobEvent {
        event: "job.log",
        job: None,
        log: Some(log.clone()),
    });
    Ok(log)
}

pub async fn list_logs(
    pool: &SqlitePool,
    job_id: Uuid,
    before: Option<i64>,
    limit: i64,
    level: Option<&str>,
) -> Result<JobLogPage, AppError> {
    let limit = limit.clamp(1, 200);
    let rows = sqlx::query_as::<_, JobLogResponse>(
        r#"SELECT id, job_id, level, event_type, item_key, attempt, message, created_at
           FROM job_logs
           WHERE job_id = ? AND (? IS NULL OR id < ?) AND (? IS NULL OR level = ?)
           ORDER BY id DESC LIMIT ?"#,
    )
    .bind(job_id)
    .bind(before)
    .bind(before)
    .bind(level)
    .bind(level)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;
    let next_before = (rows.len() == usize::try_from(limit).unwrap_or(200))
        .then(|| rows.last().map(|item| item.id))
        .flatten();
    Ok(JobLogPage {
        items: rows,
        next_before,
    })
}

fn status_message(status: &str) -> &'static str {
    match status {
        "paused" => "任务已暂停",
        "queued" => "任务已进入队列",
        "cancel_requested" => "已请求取消任务",
        "cancelled" => "任务已取消",
        "running" => "任务开始执行",
        _ => "任务状态已更新",
    }
}

pub async fn start_operation_job(
    state: &AppState,
    operation_id: Uuid,
    kind: &str,
) -> Result<(), AppError> {
    let now = Utc::now();
    let items = sqlx::query_as::<_, (Uuid, Option<String>, String)>(
        "SELECT id, source_path, status FROM operation_items WHERE operation_id = ?",
    )
    .bind(operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        r#"INSERT INTO jobs
          (id, kind, status, source_type, source_id, total_items, created_at, started_at, updated_at)
          VALUES (?, ?, 'running', 'operation', ?, ?, ?, ?, ?)
          ON CONFLICT(id) DO UPDATE SET status = 'running', total_items = excluded.total_items,
            processed_items = 0, success_items = 0, skipped_items = 0, failed_items = 0,
            current_item = NULL, error_message = NULL, started_at = excluded.started_at,
            finished_at = NULL, updated_at = excluded.updated_at"#,
    )
    .bind(operation_id)
    .bind(kind)
    .bind(operation_id.to_string())
    .bind(i64::try_from(items.len()).unwrap_or(i64::MAX))
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for (item_id, source_path, _) in items {
        sqlx::query(
            r#"INSERT INTO job_items
              (id, job_id, item_key, status, retryable, updated_at)
              VALUES (?, ?, ?, 'pending', 1, ?)
              ON CONFLICT(id) DO UPDATE SET status = 'pending', error_code = NULL,
                message = NULL, retryable = 1, updated_at = excluded.updated_at"#,
        )
        .bind(item_id)
        .bind(operation_id)
        .bind(source_path.unwrap_or_else(|| item_id.to_string()))
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    }
    let job = fetch_job(&state.pool, operation_id).await?;
    record_log(
        state,
        operation_id,
        "info",
        "started",
        None,
        None,
        "任务开始执行",
    )
    .await?;
    emit(state, job);
    Ok(())
}

pub async fn record_operation_item(
    state: &AppState,
    job_id: Uuid,
    item_id: Uuid,
    item_key: &str,
    succeeded: bool,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    let status = if succeeded { "success" } else { "failed" };
    sqlx::query(
        "UPDATE job_items SET status = ?, message = ?, retryable = ?, attempt_count = attempt_count + 1, updated_at = ? WHERE id = ? AND job_id = ?",
    )
    .bind(status)
    .bind(error_message)
    .bind(!succeeded)
    .bind(Utc::now())
    .bind(item_id)
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        r#"UPDATE jobs SET current_item = ?,
          processed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status IN ('success', 'skipped', 'failed')),
          success_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'success'),
          failed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'),
          updated_at = ? WHERE id = ?"#,
    )
    .bind(item_key)
    .bind(job_id)
    .bind(job_id)
    .bind(job_id)
    .bind(Utc::now())
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    emit(state, fetch_job(&state.pool, job_id).await?);
    let level = if succeeded { "info" } else { "error" };
    let message = error_message.unwrap_or("处理成功");
    record_log(state, job_id, level, status, Some(item_key), None, message).await?;
    Ok(())
}

pub async fn finish_operation_job(state: &AppState, id: Uuid) -> Result<(), AppError> {
    let failed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    let status = if failed == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
    sqlx::query(
        "UPDATE jobs SET status = ?, current_item = NULL, finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(Utc::now())
    .bind(Utc::now())
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    emit(state, fetch_job(&state.pool, id).await?);
    record_log(state, id, "info", status, None, None, "任务处理完成").await?;
    Ok(())
}

pub async fn start_single_item_job(
    state: &AppState,
    id: Uuid,
    kind: &str,
    item_key: &str,
    source_type: &str,
    source_id: &str,
) -> Result<Uuid, AppError> {
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        r#"INSERT INTO jobs
           (id, kind, status, source_type, source_id, total_items, current_item, created_at, started_at, updated_at)
           VALUES (?, ?, 'running', ?, ?, 1, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET status = 'running', total_items = 1,
             processed_items = 0, success_items = 0, skipped_items = 0, failed_items = 0,
             current_item = excluded.current_item, error_message = NULL,
             started_at = excluded.started_at, finished_at = NULL, updated_at = excluded.updated_at"#,
    )
    .bind(id)
    .bind(kind)
    .bind(source_type)
    .bind(source_id)
    .bind(item_key)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    let item_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM job_items WHERE job_id = ? AND item_key = ?")
            .bind(id)
            .bind(item_key)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AppError::internal)?
            .unwrap_or_else(Uuid::new_v4);
    sqlx::query(
        r#"INSERT INTO job_items
           (id, job_id, item_key, status, retryable, attempt_count, updated_at)
           VALUES (?, ?, ?, 'running', 1, 1, ?)
           ON CONFLICT(job_id, item_key) DO UPDATE SET status = 'running',
             error_code = NULL, message = NULL, retryable = 1,
             attempt_count = job_items.attempt_count + 1, updated_at = excluded.updated_at"#,
    )
    .bind(item_id)
    .bind(id)
    .bind(item_key)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    record_log(
        state,
        id,
        "info",
        "started",
        Some(item_key),
        Some(1),
        "任务开始执行",
    )
    .await?;
    emit(state, fetch_job(&state.pool, id).await?);
    Ok(item_id)
}

pub async fn finish_single_item_job(
    state: &AppState,
    id: Uuid,
    item_id: Uuid,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    let failed = error_message.is_some();
    let item_status = if failed { "failed" } else { "success" };
    let job_status = if failed {
        "completed_with_errors"
    } else {
        "completed"
    };
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = ?, message = ?, retryable = ?, updated_at = ? WHERE id = ? AND job_id = ?",
    )
    .bind(item_status)
    .bind(error_message)
    .bind(failed)
    .bind(now)
    .bind(item_id)
    .bind(id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        r#"UPDATE jobs SET status = ?, processed_items = 1,
           success_items = ?, failed_items = ?, current_item = NULL,
           error_message = ?, finished_at = ?, updated_at = ? WHERE id = ?"#,
    )
    .bind(job_status)
    .bind(i64::from(!failed))
    .bind(i64::from(failed))
    .bind(error_message)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    let level = if failed { "error" } else { "info" };
    let message = error_message.unwrap_or("处理成功");
    record_log(state, id, level, item_status, None, None, message).await?;
    record_log(state, id, level, job_status, None, None, "任务处理完成").await?;
    emit(state, fetch_job(&state.pool, id).await?);
    Ok(())
}
