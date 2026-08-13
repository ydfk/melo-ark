use chrono::Utc;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

use super::spawn_batch;

pub async fn enqueue_new_media(
    state: AppState,
    scan_job_id: Uuid,
    source_media_id: Uuid,
    source_library_id: Uuid,
    item_key: &str,
) -> Result<(), AppError> {
    let target_library_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT target_library_id FROM libraries
           WHERE id = ? AND auto_ingest_enabled = 1 AND target_library_id IS NOT NULL"#,
    )
    .bind(source_library_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .flatten();
    let Some(target_library_id) = target_library_id else {
        return Ok(());
    };
    let ingest_id = Uuid::new_v4();
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    let job_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM jobs WHERE kind = 'ingest' AND parent_job_id = ?",
    )
    .bind(scan_job_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(AppError::internal)?
    {
        Some(job_id) => job_id,
        None => {
            let job_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO jobs
                     (id, kind, status, library_id, parent_job_id, source_type, source_id,
                      total_items, created_at, updated_at)
                   VALUES (?, 'ingest', 'queued', ?, ?, 'library', ?, 0, ?, ?)"#,
            )
            .bind(job_id)
            .bind(source_library_id)
            .bind(scan_job_id)
            .bind(source_library_id.to_string())
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
            job_id
        }
    };
    let inserted = sqlx::query(
        r#"INSERT OR IGNORE INTO ingest_records
             (id, source_media_file_id, target_library_id, job_id, stage, created_at, updated_at)
           VALUES (?, ?, ?, ?, 'pending', ?, ?)"#,
    )
    .bind(ingest_id)
    .bind(source_media_id)
    .bind(target_library_id)
    .bind(job_id)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if inserted.rows_affected() == 0 {
        transaction.rollback().await.map_err(AppError::internal)?;
        return Ok(());
    }
    sqlx::query(
        r#"INSERT INTO job_items
             (id, job_id, item_key, status, retryable, updated_at)
           VALUES (?, ?, ?, 'pending', 1, ?)"#,
    )
    .bind(Uuid::new_v4())
    .bind(job_id)
    .bind(item_key)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query("UPDATE jobs SET total_items = total_items + 1, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(job_id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    crate::jobs::record_log(
        &state,
        job_id,
        "info",
        "queued",
        Some(item_key),
        None,
        "新增音乐已进入自动接入流程",
    )
    .await?;
    Ok(())
}

pub async fn start_queued_for_scan(state: AppState, scan_job_id: Uuid) -> Result<(), AppError> {
    let job_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM jobs WHERE kind = 'ingest' AND parent_job_id = ? AND status = 'queued'",
    )
    .bind(scan_job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if let Some(job_id) = job_id {
        spawn_batch(state, job_id);
    }
    Ok(())
}

pub async fn cancel_queued_batch_for_scan(
    state: &AppState,
    scan_job_id: Uuid,
) -> Result<(), AppError> {
    let job_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM jobs WHERE kind = 'ingest' AND parent_job_id = ? AND status IN ('queued', 'paused', 'interrupted')",
    )
    .bind(scan_job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let Some(job_id) = job_id else {
        return Ok(());
    };
    let now = Utc::now();
    sqlx::query(
        "UPDATE jobs SET status = 'cancelled', finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(now)
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        job_id,
        "warn",
        "cancelled",
        None,
        None,
        "来源扫描已取消，接入批次未执行",
    )
    .await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

pub async fn resume_pending(state: AppState) -> Result<(), AppError> {
    let job_ids = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT DISTINCT j.id FROM jobs j
           JOIN ingest_records ir ON ir.job_id = j.id
           WHERE ir.stage NOT IN ('completed', 'failed')
             AND j.status IN ('queued', 'interrupted')"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for job_id in job_ids {
        spawn_batch(state.clone(), job_id);
    }
    Ok(())
}

pub async fn resume_job(state: AppState, job_id: Uuid) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ingest_records WHERE job_id = ?)")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    if !exists {
        return Err(AppError::NotFound("接入任务不存在".to_owned()));
    }
    spawn_batch(state, job_id);
    Ok(())
}

pub async fn retry_job(state: AppState, job_id: Uuid) -> Result<(), AppError> {
    let retryable: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ingest_records WHERE job_id = ? AND stage = 'failed'",
    )
    .bind(job_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let job = crate::jobs::fetch_job(&state.pool, job_id).await?;
    if retryable == 0 && job.status != "failed" {
        return Err(AppError::Conflict(
            "接入任务当前没有可重试失败项".to_owned(),
        ));
    }
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE ingest_records SET stage = 'pending', last_error = NULL, completed_at = NULL, updated_at = ? WHERE job_id = ? AND stage = 'failed'",
    )
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = 'pending', message = NULL, retryable = 1, updated_at = ? WHERE job_id = ? AND status = 'failed'",
    )
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE jobs SET status = 'queued', processed_items = success_items + skipped_items, failed_items = 0, current_item = NULL, error_message = NULL, finished_at = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    resume_job(state, job_id).await
}
