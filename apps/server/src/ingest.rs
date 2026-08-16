use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::AppError,
    review::{ReviewIssue, upsert_issue},
    state::AppState,
};

mod queue;
mod reviews;

use reviews::{create_static_reviews, generate_candidates_and_reviews};

pub use queue::{
    cancel_queued_batch_for_scan, enqueue_new_media, resume_job, resume_pending, retry_job,
    start_queued_for_scan,
};

fn spawn_batch(state: AppState, job_id: Uuid) {
    tokio::spawn(async move {
        if let Err(error) = run_batch(&state, job_id).await {
            let message = error.to_string();
            let _ = fail_batch(&state, job_id, &message).await;
        }
    });
}

async fn run_batch(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    if !claim_batch(state, job_id).await? {
        return Ok(());
    }
    let items = load_pending_items(state, job_id).await?;
    if items.is_empty() {
        return finish_batch(state, job_id).await;
    }
    sync_linking_progress(state, job_id, None).await?;
    let target_library_id = items[0].target_library_id;
    let mut linked = Vec::new();
    for item in items {
        if !wait_until_runnable(state, job_id).await? {
            return finish_cancelled_batch(state, job_id).await;
        }
        claim_item(state, job_id, &item).await?;
        match link_item(state, job_id, &item).await {
            Ok(item) => linked.push(item),
            Err(error) => {
                mark_item_failed(state, job_id, &item, &error.to_string()).await?;
            }
        }
        sync_linking_progress(state, job_id, Some(&item.item_key)).await?;
    }
    if linked.is_empty() {
        return finish_batch(state, job_id).await;
    }
    crate::jobs::update_phase(state, job_id, "indexing", 0, None, None).await?;
    crate::jobs::record_log(
        state,
        job_id,
        "info",
        "index_started",
        None,
        None,
        "开始更新整理目录索引",
    )
    .await?;
    let scan_job =
        crate::scanner::enqueue_internal_scan(state.clone(), target_library_id, job_id).await?;
    match wait_for_scan(state, scan_job.id).await {
        Ok(true) => {}
        Ok(false) => return finish_cancelled_batch(state, job_id).await,
        Err(error) => {
            let message = error.to_string();
            for item in &linked {
                mark_item_failed(state, job_id, item, &message).await?;
            }
            return finish_batch(state, job_id).await;
        }
    }
    sync_processing_progress(state, job_id, None).await?;
    for item in linked {
        if !wait_until_runnable(state, job_id).await? {
            return finish_cancelled_batch(state, job_id).await;
        }
        sync_processing_progress(state, job_id, Some(&item.item_key)).await?;
        if let Err(error) = process_linked_item(state, job_id, &item).await {
            mark_item_failed(state, job_id, &item, &error.to_string()).await?;
            sync_processing_progress(state, job_id, Some(&item.item_key)).await?;
        }
    }
    finish_batch(state, job_id).await
}

#[derive(Clone, Debug)]
struct BatchItem {
    ingest_id: Uuid,
    source_media_id: Uuid,
    target_library_id: Uuid,
    track_id: Uuid,
    item_key: String,
    target_relative_path: Option<String>,
}

async fn load_pending_items(state: &AppState, job_id: Uuid) -> Result<Vec<BatchItem>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, String, Option<String>)>(
        r#"SELECT ir.id, ir.source_media_file_id, ir.target_library_id, mf.track_id,
                  mf.relative_path, ir.target_relative_path
           FROM ingest_records ir
           JOIN media_files mf ON mf.id = ir.source_media_file_id
           WHERE ir.job_id = ? AND ir.stage NOT IN ('completed', 'failed')
           ORDER BY ir.created_at, ir.id"#,
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(rows
        .into_iter()
        .map(
            |(
                ingest_id,
                source_media_id,
                target_library_id,
                track_id,
                item_key,
                target_relative_path,
            )| BatchItem {
                ingest_id,
                source_media_id,
                target_library_id,
                track_id,
                item_key,
                target_relative_path,
            },
        )
        .collect())
}

async fn link_item(
    state: &AppState,
    job_id: Uuid,
    item: &BatchItem,
) -> Result<BatchItem, AppError> {
    set_stage(state, item.ingest_id, "linking", None).await?;
    let link = crate::organizer::create_ingest_hardlink(
        state,
        item.source_media_id,
        item.target_library_id,
    )
    .await?;
    sqlx::query("UPDATE ingest_records SET target_relative_path = ?, updated_at = ? WHERE id = ?")
        .bind(&link.target_relative_path)
        .bind(Utc::now())
        .bind(item.ingest_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        job_id,
        "info",
        if link.created {
            "hardlink_created"
        } else {
            "hardlink_exists"
        },
        Some(&item.item_key),
        None,
        if link.created {
            "已创建整理硬链接"
        } else {
            "整理硬链接已存在，继续处理"
        },
    )
    .await?;
    let mut linked = item.clone();
    linked.target_relative_path = Some(link.target_relative_path);
    Ok(linked)
}

async fn process_linked_item(
    state: &AppState,
    job_id: Uuid,
    item: &BatchItem,
) -> Result<(), AppError> {
    let target_relative_path = item
        .target_relative_path
        .as_deref()
        .ok_or_else(|| AppError::NotFound("整理目标路径不存在".to_owned()))?;
    let (target_media_id, scanned_target_track_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, track_id FROM media_files WHERE library_id = ? AND relative_path = ? AND available = 1",
    )
    .bind(item.target_library_id)
    .bind(target_relative_path)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("整理文件扫描后仍未建立索引".to_owned()))?;
    let target_track_id = item.track_id;
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("UPDATE media_files SET track_id = ?, updated_at = ? WHERE id = ?")
        .bind(target_track_id)
        .bind(Utc::now())
        .bind(target_media_id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE ingest_records SET stage = 'indexed', target_media_file_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(target_media_id)
    .bind(Utc::now())
    .bind(item.ingest_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if scanned_target_track_id != target_track_id {
        sqlx::query("DELETE FROM tracks WHERE id = ? AND NOT EXISTS (SELECT 1 FROM media_files WHERE track_id = ?)")
            .bind(scanned_target_track_id)
            .bind(scanned_target_track_id)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
    }
    transaction.commit().await.map_err(AppError::internal)?;

    set_stage(state, item.ingest_id, "matching", None).await?;
    generate_candidates_and_reviews(
        state,
        job_id,
        target_track_id,
        target_media_id,
        item.target_library_id,
    )
    .await;
    set_stage(state, item.ingest_id, "analyzing", None).await?;
    if let Err(error) = crate::duplicates::analyze_media_for_ingest(state, target_media_id).await {
        upsert_issue(
            state,
            ReviewIssue {
                kind: "job_failed",
                subject_key: format!("ingest-analysis:{}", item.ingest_id),
                title: "音频分析失败",
                detail: error.to_string(),
                track_id: Some(target_track_id),
                media_file_id: Some(target_media_id),
                library_id: Some(item.target_library_id),
                confidence: None,
                payload: serde_json::json!({ "ingestId": item.ingest_id }),
            },
        )
        .await?;
    }
    set_stage(state, item.ingest_id, "reviewing", None).await?;
    create_static_reviews(
        state,
        target_track_id,
        target_media_id,
        item.target_library_id,
        target_relative_path,
    )
    .await?;
    finish_item(state, item.ingest_id, job_id, &item.item_key).await
}

async fn claim_batch(state: &AppState, job_id: Uuid) -> Result<bool, AppError> {
    loop {
        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE jobs SET status = 'running', started_at = COALESCE(started_at, ?), updated_at = ? WHERE id = ? AND status IN ('queued', 'interrupted')",
        )
        .bind(now)
        .bind(now)
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if result.rows_affected() == 1 {
            break;
        }
        let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
        match status.as_str() {
            "paused" => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            "running" => break,
            _ => return Ok(false),
        }
    }
    crate::jobs::record_log(
        state,
        job_id,
        "info",
        "started",
        None,
        None,
        "新增音乐接入批次开始执行",
    )
    .await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(true)
}

async fn claim_item(state: &AppState, job_id: Uuid, item: &BatchItem) -> Result<(), AppError> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE ingest_records SET attempt_count = attempt_count + 1, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(item.ingest_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    sqlx::query("UPDATE jobs SET current_item = ?, updated_at = ? WHERE id = ?")
        .bind(&item.item_key)
        .bind(now)
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = 'running', attempt_count = attempt_count + 1, updated_at = ? WHERE job_id = ? AND item_key = ?",
    )
    .bind(now)
    .bind(job_id)
    .bind(&item.item_key)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let attempt: i64 =
        sqlx::query_scalar("SELECT attempt_count FROM job_items WHERE job_id = ? AND item_key = ?")
            .bind(job_id)
            .bind(&item.item_key)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        job_id,
        "info",
        "item_started",
        Some(&item.item_key),
        Some(attempt),
        "开始处理新增音乐",
    )
    .await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

async fn wait_until_runnable(state: &AppState, job_id: Uuid) -> Result<bool, AppError> {
    loop {
        let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
        match status.as_str() {
            "running" => return Ok(true),
            "paused" => tokio::time::sleep(std::time::Duration::from_millis(400)).await,
            "cancel_requested" | "cancelled" => return Ok(false),
            _ => return Ok(false),
        }
    }
}

async fn set_stage(
    state: &AppState,
    ingest_id: Uuid,
    stage: &str,
    error: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE ingest_records SET stage = ?, last_error = ?, updated_at = ? WHERE id = ?")
        .bind(stage)
        .bind(error)
        .bind(Utc::now())
        .bind(ingest_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    Ok(())
}

async fn finish_item(
    state: &AppState,
    ingest_id: Uuid,
    job_id: Uuid,
    item_key: &str,
) -> Result<(), AppError> {
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE ingest_records SET stage = 'completed', last_error = NULL, completed_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(now)
    .bind(ingest_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = 'success', retryable = 0, message = NULL, updated_at = ? WHERE job_id = ? AND item_key = ?",
    )
    .bind(now)
    .bind(job_id)
    .bind(item_key)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE jobs SET processed_items = processed_items + 1, success_items = success_items + 1, current_item = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        job_id,
        "info",
        "success",
        Some(item_key),
        None,
        "新增音乐接入完成",
    )
    .await?;
    sync_processing_progress(state, job_id, None).await?;
    Ok(())
}

async fn sync_linking_progress(
    state: &AppState,
    job_id: Uuid,
    current_item: Option<&str>,
) -> Result<(), AppError> {
    let processed: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM ingest_records
           WHERE job_id = ? AND (target_relative_path IS NOT NULL OR stage = 'failed')"#,
    )
    .bind(job_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let total: i64 = sqlx::query_scalar("SELECT total_items FROM jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    crate::jobs::update_phase(
        state,
        job_id,
        "linking",
        processed,
        Some(total),
        current_item,
    )
    .await?;
    Ok(())
}

async fn sync_processing_progress(
    state: &AppState,
    job_id: Uuid,
    current_item: Option<&str>,
) -> Result<(), AppError> {
    let (processed, total): (i64, i64) =
        sqlx::query_as("SELECT processed_items, total_items FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    crate::jobs::update_phase(
        state,
        job_id,
        "processing",
        processed,
        Some(total),
        current_item,
    )
    .await?;
    Ok(())
}

async fn mark_item_failed(
    state: &AppState,
    job_id: Uuid,
    item: &BatchItem,
    message: &str,
) -> Result<(), AppError> {
    let now = Utc::now();
    set_stage(state, item.ingest_id, "failed", Some(message)).await?;
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE job_items SET status = 'failed', message = ?, retryable = 1, updated_at = ? WHERE job_id = ? AND item_key = ?",
    )
    .bind(message)
    .bind(now)
    .bind(job_id)
    .bind(&item.item_key)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query(
        "UPDATE jobs SET processed_items = processed_items + 1, failed_items = failed_items + 1, current_item = NULL, error_message = ?, updated_at = ? WHERE id = ?",
    )
    .bind(message)
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    upsert_issue(
        state,
        ReviewIssue {
            kind: if message.contains("目标路径") || message.contains("硬链接") {
                "hardlink_conflict"
            } else {
                "job_failed"
            },
            subject_key: item.ingest_id.to_string(),
            title: "自动接入失败",
            detail: message.to_owned(),
            track_id: Some(item.track_id),
            media_file_id: Some(item.source_media_id),
            library_id: Some(item.target_library_id),
            confidence: None,
            payload: serde_json::json!({ "ingestId": item.ingest_id, "jobId": job_id }),
        },
    )
    .await?;
    crate::jobs::record_log(
        state,
        job_id,
        "error",
        "failed",
        Some(&item.item_key),
        None,
        message,
    )
    .await?;
    Ok(())
}

async fn finish_batch(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    let (success, skipped, failed): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END),
             SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END),
             SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END)
           FROM job_items WHERE job_id = ?"#,
    )
    .bind(job_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let status = if failed > 0 {
        "completed_with_errors"
    } else {
        "completed"
    };
    let now = Utc::now();
    sqlx::query(
        "UPDATE jobs SET status = ?, processed_items = ?, success_items = ?, skipped_items = ?, failed_items = ?, phase = 'processing', phase_processed_items = ?, phase_total_items = total_items, current_item = NULL, finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(success + skipped + failed)
    .bind(success)
    .bind(skipped)
    .bind(failed)
    .bind(success + skipped + failed)
    .bind(now)
    .bind(now)
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        job_id,
        if failed > 0 { "warn" } else { "info" },
        status,
        None,
        None,
        if failed > 0 {
            "新增音乐接入批次完成，部分项目需要处理"
        } else {
            "新增音乐接入批次完成"
        },
    )
    .await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

async fn finish_cancelled_batch(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
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
    crate::jobs::record_log(
        state,
        job_id,
        "warn",
        "cancelled",
        None,
        None,
        "新增音乐接入批次已取消",
    )
    .await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

async fn fail_batch(state: &AppState, job_id: Uuid, message: &str) -> Result<(), AppError> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE jobs SET status = 'failed', current_item = NULL, error_message = ?, finished_at = ?, updated_at = ? WHERE id = ? AND status NOT IN ('cancelled', 'completed', 'completed_with_errors')",
    )
    .bind(message)
    .bind(now)
    .bind(now)
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    crate::jobs::record_log(state, job_id, "error", "failed", None, None, message).await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

async fn wait_for_scan(state: &AppState, job_id: Uuid) -> Result<bool, AppError> {
    let parent_job_id: Option<Uuid> =
        sqlx::query_scalar("SELECT parent_job_id FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    for _ in 0..2_400 {
        let job = crate::jobs::fetch_job(&state.pool, job_id).await?;
        if let Some(parent_job_id) = parent_job_id {
            let parent_status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
                .bind(parent_job_id)
                .fetch_one(&state.pool)
                .await
                .map_err(AppError::internal)?;
            match parent_status.as_str() {
                "paused" if matches!(job.status.as_str(), "queued" | "running" | "interrupted") => {
                    crate::jobs::set_status(
                        state,
                        job_id,
                        &["queued", "running", "interrupted"],
                        "paused",
                    )
                    .await?;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                "running" if job.status == "paused" => {
                    crate::jobs::set_status(state, job_id, &["paused"], "running").await?;
                }
                "cancel_requested" | "cancelled" => {
                    if job.status == "running" {
                        crate::jobs::set_status(state, job_id, &["running"], "cancel_requested")
                            .await?;
                    } else if matches!(job.status.as_str(), "queued" | "paused" | "interrupted") {
                        crate::jobs::set_status(
                            state,
                            job_id,
                            &["queued", "paused", "interrupted"],
                            "cancelled",
                        )
                        .await?;
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }
        match job.status.as_str() {
            "completed" => {
                if let Some(parent_job_id) = parent_job_id {
                    crate::jobs::record_log(
                        state,
                        parent_job_id,
                        "info",
                        "index_completed",
                        None,
                        None,
                        &format!("整理目录索引已更新，共处理 {} 个文件", job.processed_items),
                    )
                    .await?;
                }
                return Ok(true);
            }
            "completed_with_errors" | "failed" | "cancelled" => {
                return Err(AppError::Conflict(format!(
                    "整理目标扫描未成功完成：{}",
                    job.status
                )));
            }
            _ => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
    Err(AppError::Conflict("整理目标扫描超时".to_owned()))
}
