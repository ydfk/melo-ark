use std::{
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    state::AppState,
    tag_operations::{OperationItemResponse, OperationResponse},
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashPreviewRequest {
    pub media_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashApplyRequest {
    pub operation_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashPurgePreviewRequest {
    pub trash_operation_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashPurgeApplyRequest {
    pub purge_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashEntryResponse {
    pub operation_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub item_count: i64,
    pub total_bytes: i64,
    pub purge_id: Option<Uuid>,
    pub purge_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashPurgeItemResponse {
    pub id: Uuid,
    pub path: String,
    pub expected_size: i64,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashPurgeResponse {
    pub id: Uuid,
    pub trash_operation_id: Uuid,
    pub status: String,
    pub total_items: i64,
    pub total_bytes: i64,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub items: Vec<TrashPurgeItemResponse>,
}

#[derive(Debug, FromRow)]
struct TrashOperationRow {
    id: Uuid,
    status: String,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct PurgeRow {
    id: Uuid,
    trash_operation_id: Uuid,
    status: String,
    total_items: i64,
    total_bytes: i64,
    created_at: DateTime<Utc>,
    confirmed_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct PurgeItemRow {
    id: Uuid,
    library_id: Option<Uuid>,
    path: String,
    expected_size: i64,
    expected_device_id: String,
    expected_inode: String,
    status: String,
    error_message: Option<String>,
}

#[derive(Debug, FromRow)]
struct TrashTarget {
    media_id: Uuid,
    library_id: Uuid,
    library_path: String,
    relative_path: String,
    writable: bool,
    file_size: i64,
    device_id: String,
    inode: String,
    blake3_hash: Option<String>,
}

pub async fn preview(
    state: &AppState,
    user_id: Uuid,
    request: TrashPreviewRequest,
) -> Result<OperationResponse, AppError> {
    if request.media_ids.is_empty() {
        return Err(AppError::BadRequest("至少选择一个媒体文件".to_owned()));
    }
    let operation_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO operations (id, kind, status, request_json, created_by, created_at) VALUES (?, 'trash', 'previewed', ?, ?, ?)",
    )
    .bind(operation_id)
    .bind(serde_json::to_string(&request).map_err(AppError::internal)?)
    .bind(user_id)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut items = Vec::new();
    for media_id in request.media_ids {
        let target = load_target(state, media_id).await?;
        if !target.writable {
            return Err(AppError::BadRequest(
                "回收站操作要求 Library Root 允许写入".to_owned(),
            ));
        }
        let root = Path::new(&target.library_path)
            .canonicalize()
            .map_err(AppError::internal)?;
        let source = root
            .join(&target.relative_path)
            .canonicalize()
            .map_err(AppError::internal)?;
        if !source.starts_with(&root) {
            return Err(AppError::BadRequest(
                "回收站源路径逃逸 Library Root".to_owned(),
            ));
        }
        let trash_root = root.join(".meloark-trash").join(operation_id.to_string());
        let destination = trash_root.join(&target.relative_path);
        if !destination.starts_with(&trash_root) {
            return Err(AppError::BadRequest("回收站目标路径不安全".to_owned()));
        }
        let conflict = destination.exists();
        let preflight = serde_json::json!({
            "sameFilesystem": true,
            "targetExists": conflict,
            "sameInode": false,
            "pathConflict": conflict,
            "canApply": !conflict,
            "libraryId": target.library_id,
            "fileSize": target.file_size,
            "deviceId": target.device_id,
            "inode": target.inode,
            "blake3Hash": target.blake3_hash,
        });
        let item_id = Uuid::new_v4();
        let error_message = conflict.then(|| "回收站目标路径已存在，拒绝覆盖".to_owned());
        sqlx::query(
            r#"INSERT INTO operation_items
              (id, operation_id, media_file_id, action, status, source_path, target_path,
               preflight_json, error_message, retryable, created_at, updated_at)
              VALUES (?, ?, ?, 'trash', 'previewed', ?, ?, ?, ?, 1, ?, ?)"#,
        )
        .bind(item_id)
        .bind(operation_id)
        .bind(target.media_id)
        .bind(source.to_string_lossy().into_owned())
        .bind(destination.to_string_lossy().into_owned())
        .bind(preflight.to_string())
        .bind(&error_message)
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        items.push(OperationItemResponse {
            id: item_id,
            media_file_id: Some(target.media_id),
            source_path: Some(source.to_string_lossy().into_owned()),
            target_path: Some(destination.to_string_lossy().into_owned()),
            status: "previewed".to_owned(),
            diffs: Vec::new(),
            error_message,
            preflight: Some(preflight),
        });
    }
    Ok(OperationResponse {
        id: operation_id,
        kind: "trash".to_owned(),
        status: "previewed".to_owned(),
        items,
    })
}

pub async fn apply(
    state: &AppState,
    request: TrashApplyRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "TRASH" {
        return Err(AppError::BadRequest(
            "移入回收站必须提交 confirmation=TRASH".to_owned(),
        ));
    }
    transition_and_move(state, request.operation_id, "previewed", "completed", false).await
}

pub async fn restore(
    state: &AppState,
    request: TrashApplyRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "RESTORE" {
        return Err(AppError::BadRequest(
            "恢复文件必须提交 confirmation=RESTORE".to_owned(),
        ));
    }
    if let Some((purge_id, purge_status)) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, status FROM trash_purges WHERE trash_operation_id = ?",
    )
    .bind(request.operation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    {
        if purge_status != "previewed" {
            return Err(AppError::Conflict(
                "永久清理已经开始，无法再恢复该回收站操作".to_owned(),
            ));
        }
        sqlx::query("DELETE FROM trash_purges WHERE id = ?")
            .bind(purge_id)
            .execute(&state.pool)
            .await
            .map_err(AppError::internal)?;
    }
    transition_and_move(
        state,
        request.operation_id,
        "completed",
        "rolled_back",
        true,
    )
    .await
}

pub async fn list(state: &AppState) -> Result<Vec<TrashEntryResponse>, AppError> {
    let operations = sqlx::query_as::<_, TrashOperationRow>(
        r#"SELECT id, status, created_at, finished_at FROM operations
           WHERE kind = 'trash' AND status IN ('completed', 'completed_with_errors')
           ORDER BY created_at DESC"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut entries = Vec::with_capacity(operations.len());
    for operation in operations {
        let (item_count, total_bytes) = sqlx::query_as::<_, (i64, i64)>(
            r#"SELECT COUNT(*), COALESCE(SUM(CAST(json_extract(preflight_json, '$.fileSize') AS INTEGER)), 0)
               FROM operation_items WHERE operation_id = ? AND status = 'success'"#,
        )
        .bind(operation.id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let purge = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, status FROM trash_purges WHERE trash_operation_id = ?",
        )
        .bind(operation.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?;
        entries.push(TrashEntryResponse {
            operation_id: operation.id,
            status: operation.status,
            created_at: operation.created_at,
            finished_at: operation.finished_at,
            item_count,
            total_bytes,
            purge_id: purge.as_ref().map(|value| value.0),
            purge_status: purge.map(|value| value.1),
        });
    }
    Ok(entries)
}

pub async fn preview_purge(
    state: &AppState,
    user_id: Uuid,
    request: TrashPurgePreviewRequest,
) -> Result<TrashPurgeResponse, AppError> {
    if let Some(existing) = load_purge_by_operation(state, request.trash_operation_id).await? {
        return purge_response(state, existing).await;
    }
    let operation_status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM operations WHERE id = ? AND kind = 'trash'",
    )
    .bind(request.trash_operation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("回收站操作不存在".to_owned()))?;
    if !matches!(
        operation_status.as_str(),
        "completed" | "completed_with_errors"
    ) {
        return Err(AppError::Conflict(
            "只有已经移入回收站的文件可以预览永久清理".to_owned(),
        ));
    }
    let source_items = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT id, target_path, preflight_json FROM operation_items
           WHERE operation_id = ? AND status = 'success' ORDER BY created_at"#,
    )
    .bind(request.trash_operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if source_items.is_empty() {
        return Err(AppError::Conflict("该回收站操作没有可清理文件".to_owned()));
    }
    let purge_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        r#"INSERT INTO trash_purges
           (id, trash_operation_id, status, created_by, total_items, total_bytes, created_at)
           VALUES (?, ?, 'previewed', ?, ?, 0, ?)"#,
    )
    .bind(purge_id)
    .bind(request.trash_operation_id)
    .bind(user_id)
    .bind(source_items.len() as i64)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut total_bytes = 0_i64;
    for (source_item_id, path, preflight_json) in source_items {
        let library_id = serde_json::from_str::<serde_json::Value>(&preflight_json)
            .ok()
            .and_then(|value| value.get("libraryId")?.as_str()?.parse::<Uuid>().ok());
        let (library_id, metadata, error_message) = match library_id {
            Some(library_id) => match validate_purge_path(
                state,
                library_id,
                request.trash_operation_id,
                Path::new(&path),
            )
            .await
            {
                Ok(metadata) => (Some(library_id), Some(metadata), None),
                Err(error) => (Some(library_id), None, Some(error.to_string())),
            },
            None => (
                None,
                None,
                Some("回收站记录缺少 Library Root 标识".to_owned()),
            ),
        };
        let (size, device_id, inode, status) = metadata
            .map(|metadata| {
                (
                    metadata.len() as i64,
                    metadata.dev().to_string(),
                    metadata.ino().to_string(),
                    "previewed",
                )
            })
            .unwrap_or_else(|| (0, String::new(), String::new(), "failed"));
        total_bytes += size;
        sqlx::query(
            r#"INSERT INTO trash_purge_items
               (id, purge_id, source_operation_item_id, library_id, path, expected_size,
                expected_device_id, expected_inode, status, error_message, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::new_v4())
        .bind(purge_id)
        .bind(source_item_id)
        .bind(library_id)
        .bind(path)
        .bind(size)
        .bind(device_id)
        .bind(inode)
        .bind(status)
        .bind(error_message)
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    }
    sqlx::query("UPDATE trash_purges SET total_bytes = ? WHERE id = ?")
        .bind(total_bytes)
        .bind(purge_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let purge = load_purge(state, purge_id).await?;
    purge_response(state, purge).await
}

pub async fn apply_purge(
    state: &AppState,
    request: TrashPurgeApplyRequest,
) -> Result<TrashPurgeResponse, AppError> {
    if request.confirmation != "PURGE_PERMANENTLY" {
        return Err(AppError::BadRequest(
            "永久清理必须提交 confirmation=PURGE_PERMANENTLY".to_owned(),
        ));
    }
    let purge = load_purge(state, request.purge_id).await?;
    if purge.status != "previewed" {
        return Err(AppError::Conflict("永久清理状态不允许执行".to_owned()));
    }
    let now = Utc::now();
    let changed = sqlx::query(
        "UPDATE trash_purges SET status = 'running', confirmed_at = ? WHERE id = ? AND status = 'previewed'",
    )
        .bind(now)
        .bind(request.purge_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(AppError::Conflict("永久清理已经被其他请求执行".to_owned()));
    }
    let items = load_purge_items(state, request.purge_id).await?;
    for item in items.into_iter().filter(|item| item.status == "previewed") {
        let result = revalidate_and_remove(state, purge.trash_operation_id, &item).await;
        let (status, error_message) = match result {
            Ok(()) => ("success", None),
            Err(error) => ("failed", Some(error.to_string())),
        };
        sqlx::query(
            "UPDATE trash_purge_items SET status = ?, error_message = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(error_message)
        .bind(Utc::now())
        .bind(item.id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    }
    let failures = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM trash_purge_items WHERE purge_id = ? AND status = 'failed'",
    )
    .bind(request.purge_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let final_status = if failures == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
    sqlx::query("UPDATE trash_purges SET status = ?, finished_at = ? WHERE id = ?")
        .bind(final_status)
        .bind(Utc::now())
        .bind(request.purge_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    purge_response(state, load_purge(state, request.purge_id).await?).await
}

pub async fn retry_failed(
    state: &AppState,
    request: TrashApplyRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "TRASH" {
        return Err(AppError::BadRequest(
            "重试回收站操作必须提交 confirmation=TRASH".to_owned(),
        ));
    }
    let changed = sqlx::query(
        "UPDATE operations SET status = 'previewed', finished_at = NULL WHERE id = ? AND kind = 'trash' AND status = 'completed_with_errors'",
    )
    .bind(request.operation_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "该回收站操作当前没有可重试失败项".to_owned(),
        ));
    }
    sqlx::query("UPDATE operation_items SET status = 'previewed', error_message = NULL WHERE operation_id = ? AND status = 'failed'")
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    apply(state, request).await
}

async fn transition_and_move(
    state: &AppState,
    operation_id: Uuid,
    expected_status: &str,
    success_status: &str,
    reverse: bool,
) -> Result<OperationResponse, AppError> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM operations WHERE id = ? AND kind = 'trash'",
    )
    .bind(operation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("回收站操作不存在".to_owned()))?;
    if status != expected_status {
        return Err(AppError::Conflict(
            "回收站操作状态不允许当前动作".to_owned(),
        ));
    }
    sqlx::query("UPDATE operations SET status = 'running', confirmed_at = COALESCE(confirmed_at, ?) WHERE id = ?")
        .bind(Utc::now())
        .bind(operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if !reverse {
        crate::jobs::start_operation_job(state, operation_id, "trash").await?;
    }
    let item_status = if reverse { "success" } else { "previewed" };
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, source_path, target_path FROM operation_items WHERE operation_id = ? AND status = ?",
    )
    .bind(operation_id)
    .bind(item_status)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut failures = 0_u64;
    for (item_id, source, trash) in rows {
        let (from, to) = if reverse {
            (trash, source)
        } else {
            (source, trash)
        };
        let result = move_without_overwrite(Path::new(&from), Path::new(&to)).await;
        let (next_status, error_message) = match result {
            Ok(()) => (if reverse { "rolled_back" } else { "success" }, None),
            Err(error) => {
                failures += 1;
                ("failed", Some(error.to_string()))
            }
        };
        sqlx::query(
            "UPDATE operation_items SET status = ?, error_message = ?, updated_at = ? WHERE id = ?",
        )
        .bind(next_status)
        .bind(error_message.as_deref())
        .bind(Utc::now())
        .bind(item_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if !reverse {
            crate::jobs::record_operation_item(
                state,
                operation_id,
                item_id,
                &from,
                next_status == "success",
                error_message.as_deref(),
            )
            .await?;
        }
    }
    let final_status = if failures == 0 {
        success_status
    } else {
        "completed_with_errors"
    };
    sqlx::query(
        "UPDATE operations SET status = ?, finished_at = ?, rolled_back_at = CASE WHEN ? = 'rolled_back' THEN ? ELSE rolled_back_at END WHERE id = ?",
    )
    .bind(final_status)
    .bind(Utc::now())
    .bind(final_status)
    .bind(Utc::now())
    .bind(operation_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let libraries = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT DISTINCT mf.library_id FROM operation_items oi
           JOIN media_files mf ON mf.id = oi.media_file_id WHERE oi.operation_id = ?"#,
    )
    .bind(operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for library_id in libraries {
        let _ = crate::scanner::enqueue_scan(state.clone(), library_id).await;
    }
    if !reverse {
        crate::jobs::finish_operation_job(state, operation_id).await?;
    }
    crate::tag_operations::get_operation(state, operation_id).await
}

async fn move_without_overwrite(from: &Path, to: &Path) -> Result<(), AppError> {
    if to.exists() {
        return Err(AppError::Conflict("目标路径已存在，拒绝覆盖".to_owned()));
    }
    let parent = to
        .parent()
        .ok_or_else(|| AppError::BadRequest("目标路径没有父目录".to_owned()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| AppError::BadRequest(format!("无法创建目录：{error}")))?;
    tokio::fs::rename(from, to)
        .await
        .map_err(|error| AppError::BadRequest(format!("移动文件失败：{error}")))
}

async fn load_target(state: &AppState, id: Uuid) -> Result<TrashTarget, AppError> {
    sqlx::query_as::<_, TrashTarget>(
        r#"SELECT mf.id AS media_id, mf.library_id, l.path AS library_path,
          mf.relative_path, l.writable, mf.file_size, mf.device_id, mf.inode,
          mf.full_hash AS blake3_hash
          FROM media_files mf
          JOIN libraries l ON l.id = mf.library_id WHERE mf.id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("媒体文件不存在".to_owned()))
}

async fn load_purge(state: &AppState, id: Uuid) -> Result<PurgeRow, AppError> {
    sqlx::query_as::<_, PurgeRow>(
        r#"SELECT id, trash_operation_id, status, total_items, total_bytes,
           created_at, confirmed_at, finished_at FROM trash_purges WHERE id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("永久清理预览不存在".to_owned()))
}

async fn load_purge_by_operation(
    state: &AppState,
    operation_id: Uuid,
) -> Result<Option<PurgeRow>, AppError> {
    sqlx::query_as::<_, PurgeRow>(
        r#"SELECT id, trash_operation_id, status, total_items, total_bytes,
           created_at, confirmed_at, finished_at FROM trash_purges
           WHERE trash_operation_id = ?"#,
    )
    .bind(operation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)
}

async fn load_purge_items(state: &AppState, purge_id: Uuid) -> Result<Vec<PurgeItemRow>, AppError> {
    sqlx::query_as::<_, PurgeItemRow>(
        r#"SELECT id, library_id, path, expected_size, expected_device_id,
           expected_inode, status, error_message FROM trash_purge_items
           WHERE purge_id = ? ORDER BY created_at"#,
    )
    .bind(purge_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)
}

async fn purge_response(state: &AppState, purge: PurgeRow) -> Result<TrashPurgeResponse, AppError> {
    let items = load_purge_items(state, purge.id)
        .await?
        .into_iter()
        .map(|item| TrashPurgeItemResponse {
            id: item.id,
            path: item.path,
            expected_size: item.expected_size,
            status: item.status,
            error_message: item.error_message,
        })
        .collect();
    Ok(TrashPurgeResponse {
        id: purge.id,
        trash_operation_id: purge.trash_operation_id,
        status: purge.status,
        total_items: purge.total_items,
        total_bytes: purge.total_bytes,
        created_at: purge.created_at,
        confirmed_at: purge.confirmed_at,
        finished_at: purge.finished_at,
        items,
    })
}

async fn validate_purge_path(
    state: &AppState,
    library_id: Uuid,
    operation_id: Uuid,
    path: &Path,
) -> Result<std::fs::Metadata, AppError> {
    let library_path = sqlx::query_scalar::<_, String>("SELECT path FROM libraries WHERE id = ?")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::NotFound("Library Root 不存在".to_owned()))?;
    let root = Path::new(&library_path)
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("无法访问 Library Root：{error}")))?;
    let trash_root = root.join(".meloark-trash").join(operation_id.to_string());
    if !path.is_absolute() || !path.starts_with(&trash_root) {
        return Err(AppError::BadRequest(
            "永久清理路径逃逸回收站目录".to_owned(),
        ));
    }
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|error| AppError::BadRequest(format!("无法读取待清理文件：{error}")))?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::BadRequest("永久清理拒绝符号链接".to_owned()));
    }
    if !metadata.is_file() {
        return Err(AppError::BadRequest("永久清理目标不是普通文件".to_owned()));
    }
    let canonical_trash_root = trash_root
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("无法访问回收站目录：{error}")))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("无法解析待清理文件：{error}")))?;
    if !canonical_path.starts_with(&canonical_trash_root) {
        return Err(AppError::BadRequest(
            "永久清理路径逃逸回收站目录".to_owned(),
        ));
    }
    Ok(metadata)
}

async fn revalidate_and_remove(
    state: &AppState,
    operation_id: Uuid,
    item: &PurgeItemRow,
) -> Result<(), AppError> {
    let library_id = item
        .library_id
        .ok_or_else(|| AppError::BadRequest("回收站记录缺少 Library Root 标识".to_owned()))?;
    let metadata =
        validate_purge_path(state, library_id, operation_id, Path::new(&item.path)).await?;
    if metadata.len() as i64 != item.expected_size
        || metadata.dev().to_string() != item.expected_device_id
        || metadata.ino().to_string() != item.expected_inode
    {
        return Err(AppError::Conflict(
            "待清理文件在预览后已变化，拒绝永久删除".to_owned(),
        ));
    }
    tokio::fs::remove_file(PathBuf::from(&item.path))
        .await
        .map_err(|error| AppError::BadRequest(format!("永久删除文件失败：{error}")))
}
