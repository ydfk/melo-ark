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

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub event: &'static str,
    pub job: JobResponse,
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

pub async fn create_scan_job(state: &AppState, library_id: Uuid) -> Result<JobResponse, AppError> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let inserted = sqlx::query(
        r#"INSERT INTO jobs (id, kind, status, library_id, created_at, updated_at)
           SELECT ?, 'scan', 'queued', ?, ?, ?
           WHERE NOT EXISTS (
             SELECT 1 FROM jobs
             WHERE library_id = ? AND kind = 'scan' AND status = 'queued'
           )"#,
    )
    .bind(id)
    .bind(library_id)
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
    emit(state, job.clone());
    Ok(job)
}

pub async fn fetch_job(pool: &SqlitePool, id: Uuid) -> Result<JobResponse, AppError> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT id, kind, status, library_id, total_items, processed_items, success_items,
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
        SELECT id, kind, status, library_id, total_items, processed_items, success_items,
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
    emit(state, job.clone());
    Ok(job)
}

pub fn emit(state: &AppState, job: JobResponse) {
    let _ = state.events.send(JobEvent {
        event: "job.updated",
        job,
    });
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
          (id, kind, status, total_items, created_at, started_at, updated_at)
          VALUES (?, ?, 'running', ?, ?, ?, ?)
          ON CONFLICT(id) DO UPDATE SET status = 'running', total_items = excluded.total_items,
            processed_items = 0, success_items = 0, skipped_items = 0, failed_items = 0,
            current_item = NULL, error_message = NULL, started_at = excluded.started_at,
            finished_at = NULL, updated_at = excluded.updated_at"#,
    )
    .bind(operation_id)
    .bind(kind)
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
    Ok(())
}

pub async fn start_single_item_job(
    state: &AppState,
    id: Uuid,
    kind: &str,
    item_key: &str,
) -> Result<Uuid, AppError> {
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        r#"INSERT INTO jobs
           (id, kind, status, total_items, current_item, created_at, started_at, updated_at)
           VALUES (?, ?, 'running', 1, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET status = 'running', total_items = 1,
             processed_items = 0, success_items = 0, skipped_items = 0, failed_items = 0,
             current_item = excluded.current_item, error_message = NULL,
             started_at = excluded.started_at, finished_at = NULL, updated_at = excluded.updated_at"#,
    )
    .bind(id)
    .bind(kind)
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
    emit(state, fetch_job(&state.pool, id).await?);
    Ok(())
}
