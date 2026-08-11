mod audio;
mod job_control;
mod runtime;
mod storage;

use std::path::{Path, PathBuf};

use chrono::Utc;
use tokio::sync::{OwnedSemaphorePermit, mpsc};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    error::AppError,
    jobs::{self, JobResponse},
    library::{LibraryRecord, is_supported_audio, path_is_excluded, preflight_path},
    state::AppState,
};

use self::{
    audio::{file_stat, inspect_audio},
    job_control::{
        emit_current, fetch_library, finish_cancelled, increment_total, record_item_failure,
        record_item_success, record_walk_error, upsert_running_item, wait_until_runnable,
    },
    storage::{media_is_unchanged, reconcile_removed_files, upsert_media},
};

pub use self::runtime::{refresh_watchers, start_background_services};

pub async fn enqueue_scan(state: AppState, library_id: Uuid) -> Result<JobResponse, AppError> {
    let job = jobs::create_scan_job(&state, library_id).await?;
    spawn_job(state, job.id);
    Ok(job)
}

pub fn spawn_job(state: AppState, job_id: Uuid) {
    tokio::spawn(async move {
        if let Err(error) = run_scan(state.clone(), job_id).await {
            tracing::error!(job_id = %job_id, %error, "扫描任务异常退出");
            let now = Utc::now();
            let _ = sqlx::query(
                "UPDATE jobs SET status = 'failed', error_message = ?, finished_at = ?, updated_at = ? WHERE id = ? AND status NOT IN ('cancelled', 'completed', 'completed_with_errors')",
            )
            .bind(error.to_string())
            .bind(now)
            .bind(now)
            .bind(job_id)
            .execute(&state.pool)
            .await;
            let _ = sqlx::query(
                "UPDATE job_items SET status = 'failed', error_code = 'job_aborted', message = '任务异常退出，可重试该项', retryable = 1, updated_at = ? WHERE job_id = ? AND status = 'running'",
            )
            .bind(Utc::now())
            .bind(job_id)
            .execute(&state.pool)
            .await;
            let _ = sqlx::query(
                "UPDATE jobs SET processed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status IN ('success', 'skipped', 'failed')), failed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'), updated_at = ? WHERE id = ?",
            )
            .bind(job_id)
            .bind(job_id)
            .bind(Utc::now())
            .bind(job_id)
            .execute(&state.pool)
            .await;
            emit_current(&state, job_id).await;
        }
    });
}

async fn run_scan(state: AppState, job_id: Uuid) -> Result<(), AppError> {
    let Some(_permit) = acquire_scan_slot(&state, job_id).await? else {
        return Ok(());
    };

    let job = jobs::fetch_job(&state.pool, job_id).await?;
    let library_id = job
        .library_id
        .ok_or_else(|| AppError::NotFound("任务关联的 Library Root 已被删除".to_owned()))?;
    let library = fetch_library(&state, library_id).await?;
    let (root, _) = preflight_path(&library.path)?;
    let excludes: Vec<String> = serde_json::from_str(&library.exclude_patterns).unwrap_or_default();
    let (sender, mut receiver) = mpsc::channel::<Result<PathBuf, String>>(64);
    let enumerate_root = root.clone();
    tokio::task::spawn_blocking(move || enumerate_files(enumerate_root, excludes, sender));

    while let Some(entry) = receiver.recv().await {
        if !wait_until_runnable(&state, job_id).await? {
            finish_cancelled(&state, job_id).await?;
            return Ok(());
        }
        match entry {
            Ok(path) => process_job_path(&state, job_id, &library, &root, &path).await?,
            Err(message) => record_walk_error(&state, job_id, message).await?,
        }
        emit_current(&state, job_id).await;
    }

    reconcile_removed_files(&state, job_id, library_id).await?;
    finish_scan(&state, job_id, library_id).await
}

async fn acquire_scan_slot(
    state: &AppState,
    job_id: Uuid,
) -> Result<Option<OwnedSemaphorePermit>, AppError> {
    loop {
        let permit = state
            .scan_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(AppError::internal)?;
        let now = Utc::now();
        let claimed = sqlx::query(
            r#"UPDATE jobs
               SET status = 'running', started_at = COALESCE(started_at, ?), updated_at = ?
               WHERE id = ? AND status IN ('queued', 'interrupted')
                 AND NOT EXISTS (
                   SELECT 1 FROM jobs AS active
                   WHERE active.kind = 'scan'
                     AND active.library_id = (SELECT library_id FROM jobs WHERE id = ?)
                     AND active.id != ?
                     AND active.status IN ('running', 'paused', 'cancel_requested')
                 )"#,
        )
        .bind(now)
        .bind(now)
        .bind(job_id)
        .bind(job_id)
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if claimed.rows_affected() == 1 {
            return Ok(Some(permit));
        }
        drop(permit);

        let job = jobs::fetch_job(&state.pool, job_id).await?;
        match job.status.as_str() {
            "queued" | "interrupted" => {
                // 同一曲库已有扫描时保留后续任务，避免文件变更后的重扫请求被吞掉。
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            "cancel_requested" => {
                finish_cancelled(state, job_id).await?;
                return Ok(None);
            }
            _ => return Ok(None),
        }
    }
}

fn enumerate_files(
    root: PathBuf,
    excludes: Vec<String>,
    sender: mpsc::Sender<Result<PathBuf, String>>,
) {
    for entry in WalkDir::new(&root).follow_links(false) {
        let result = match entry {
            Ok(entry) if entry.file_type().is_file() && is_supported_audio(entry.path()) => {
                if path_is_excluded(entry.path(), &root, &excludes) {
                    continue;
                }
                Ok(entry.into_path())
            }
            Ok(_) => continue,
            Err(error) => Err(error.to_string()),
        };
        if sender.blocking_send(result).is_err() {
            break;
        }
    }
}

async fn process_job_path(
    state: &AppState,
    job_id: Uuid,
    library: &LibraryRecord,
    root: &Path,
    path: &Path,
) -> Result<(), AppError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| AppError::BadRequest("扫描路径逃逸出 Library Root".to_owned()))?
        .to_string_lossy()
        .into_owned();
    let prior_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM job_items WHERE job_id = ? AND item_key = ?")
            .bind(job_id)
            .bind(&relative)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?;
    if matches!(prior_status.as_deref(), Some("success" | "skipped")) {
        return Ok(());
    }
    if prior_status.is_none() {
        increment_total(state, job_id).await?;
    }
    upsert_running_item(state, job_id, &relative).await?;
    sqlx::query("UPDATE jobs SET current_item = ?, updated_at = ? WHERE id = ?")
        .bind(&relative)
        .bind(Utc::now())
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;

    let canonical = match path.canonicalize() {
        Ok(value) if value.starts_with(root) => value,
        Ok(_) => {
            return record_item_failure(
                state,
                job_id,
                &relative,
                "path_outside_library",
                "文件解析后的真实路径位于 Library Root 外部",
                false,
            )
            .await;
        }
        Err(error) => {
            return record_item_failure(
                state,
                job_id,
                &relative,
                "path_unavailable",
                &format!("文件当前不可访问：{error}"),
                true,
            )
            .await;
        }
    };
    let metadata = match canonical.metadata() {
        Ok(value) => value,
        Err(error) => {
            return record_item_failure(
                state,
                job_id,
                &relative,
                "metadata_failed",
                &format!("读取文件属性失败：{error}"),
                true,
            )
            .await;
        }
    };
    let stat = file_stat(&relative, &canonical, &metadata)?;
    if media_is_unchanged(state, library.id, &stat).await? {
        sqlx::query(
            "UPDATE media_files SET last_seen_scan_id = ?, updated_at = ? WHERE library_id = ? AND relative_path = ?",
        )
        .bind(job_id)
        .bind(Utc::now())
        .bind(library.id)
        .bind(&relative)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        return record_item_success(state, job_id, &relative, true).await;
    }

    let fallback_title = canonical
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名曲目")
        .to_owned();
    let inspect_path = canonical;
    let info = tokio::task::spawn_blocking(move || inspect_audio(&inspect_path, fallback_title))
        .await
        .map_err(AppError::internal)?;
    upsert_media(state, job_id, library, &stat, info).await?;
    record_item_success(state, job_id, &relative, false).await
}

async fn finish_scan(state: &AppState, job_id: Uuid, library_id: Uuid) -> Result<(), AppError> {
    let failed: i64 = sqlx::query_scalar("SELECT failed_items FROM jobs WHERE id = ?")
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
        "UPDATE jobs SET status = ?, current_item = NULL, finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(now)
    .bind(now)
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    sqlx::query("UPDATE libraries SET last_scan_at = ?, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(now)
        .bind(library_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    emit_current(state, job_id).await;
    Ok(())
}
