use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::{error::AppError, jobs, library::LibraryRecord, state::AppState};

pub(super) async fn wait_until_runnable(state: &AppState, job_id: Uuid) -> Result<bool, AppError> {
    loop {
        let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
        match status.as_str() {
            "running" => return Ok(true),
            "paused" => tokio::time::sleep(Duration::from_millis(400)).await,
            "cancel_requested" | "cancelled" => return Ok(false),
            _ => return Ok(false),
        }
    }
}

pub(super) async fn increment_total(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE jobs SET total_items = total_items + 1, updated_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    Ok(())
}

pub(super) async fn upsert_running_item(
    state: &AppState,
    job_id: Uuid,
    key: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO job_items (id, job_id, item_key, status, attempt_count, updated_at)
        VALUES (?, ?, ?, 'running', 1, ?)
        ON CONFLICT(job_id, item_key) DO UPDATE SET
          status = 'running', attempt_count = attempt_count + 1, updated_at = excluded.updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(job_id)
    .bind(key)
    .bind(Utc::now())
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let attempt: i64 =
        sqlx::query_scalar("SELECT attempt_count FROM job_items WHERE job_id = ? AND item_key = ?")
            .bind(job_id)
            .bind(key)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    jobs::record_log(
        state,
        job_id,
        "info",
        "item_started",
        Some(key),
        Some(attempt),
        "开始处理",
    )
    .await?;
    Ok(())
}

pub(super) async fn record_item_success(
    state: &AppState,
    job_id: Uuid,
    key: &str,
    skipped: bool,
) -> Result<(), AppError> {
    let item_status = if skipped { "skipped" } else { "success" };
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = ?, updated_at = ? WHERE job_id = ? AND item_key = ?",
    )
    .bind(item_status)
    .bind(Utc::now())
    .bind(job_id)
    .bind(key)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if skipped {
        sqlx::query(
            "UPDATE jobs SET processed_items = processed_items + 1, skipped_items = skipped_items + 1, updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now())
        .bind(job_id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    } else {
        sqlx::query(
            "UPDATE jobs SET processed_items = processed_items + 1, success_items = success_items + 1, updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now())
        .bind(job_id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    }
    transaction.commit().await.map_err(AppError::internal)?;
    let event_type = if skipped { "skipped" } else { "success" };
    let message = if skipped {
        "文件未变化"
    } else {
        "处理成功"
    };
    jobs::record_log(state, job_id, "info", event_type, Some(key), None, message).await?;
    Ok(())
}

pub(super) async fn record_item_failure(
    state: &AppState,
    job_id: Uuid,
    key: &str,
    code: &str,
    message: &str,
    retryable: bool,
) -> Result<(), AppError> {
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = 'failed', error_code = ?, message = ?, retryable = ?, updated_at = ? WHERE job_id = ? AND item_key = ?",
    )
    .bind(code)
    .bind(message)
    .bind(retryable)
    .bind(Utc::now())
    .bind(job_id)
    .bind(key)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE jobs SET processed_items = processed_items + 1, failed_items = failed_items + 1, updated_at = ? WHERE id = ?",
    )
    .bind(Utc::now())
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    jobs::record_log(state, job_id, "error", "failed", Some(key), None, message).await?;
    Ok(())
}

pub(super) async fn record_walk_error(
    state: &AppState,
    job_id: Uuid,
    message: String,
) -> Result<(), AppError> {
    let key = format!("walk-error-{}", Uuid::new_v4());
    increment_total(state, job_id).await?;
    upsert_running_item(state, job_id, &key).await?;
    record_item_failure(state, job_id, &key, "walk_failed", &message, true).await
}

pub(super) async fn finish_cancelled(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE jobs SET status = 'cancelled', current_item = NULL, finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(now)
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    jobs::record_log(
        state,
        job_id,
        "warn",
        "cancelled",
        None,
        None,
        "扫描任务已取消",
    )
    .await?;
    emit_current(state, job_id).await;
    Ok(())
}

pub(super) async fn emit_current(state: &AppState, job_id: Uuid) {
    if let Ok(job) = jobs::fetch_job(&state.pool, job_id).await {
        jobs::emit(state, job);
    }
}

pub(super) async fn fetch_library(state: &AppState, id: Uuid) -> Result<LibraryRecord, AppError> {
    sqlx::query_as::<_, LibraryRecord>(
        r#"
        SELECT id, name, path, scan_enabled, watch_enabled, writable, role,
               exclude_patterns, last_scan_at, created_at, updated_at
        FROM libraries WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("曲库不存在".to_owned()))
}
