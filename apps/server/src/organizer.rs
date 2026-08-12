use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    state::AppState,
    tag_operations::{OperationItemResponse, OperationResponse},
    text_normalization::artist_initial,
};

pub const DEFAULT_TEMPLATE: &str = "{artist}/{album}/{track:02} - {title}.{ext}";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerPreviewRequest {
    pub media_ids: Vec<Uuid>,
    pub target_library_id: Uuid,
    #[serde(default = "default_template")]
    pub template: String,
    #[serde(default = "default_true")]
    pub cross_platform_safe: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerApplyRequest {
    pub operation_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerUndoRequest {
    pub operation_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct OrganizerPreflight {
    same_filesystem: bool,
    target_exists: bool,
    same_inode: bool,
    path_conflict: bool,
    can_apply: bool,
}

#[derive(Debug, FromRow)]
struct OrganizerSource {
    media_id: Uuid,
    source_library_path: String,
    relative_path: String,
    extension: String,
    title: String,
    artist: String,
    album_artist: Option<String>,
    album: String,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    year: Option<i64>,
    genre: Option<String>,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
}

#[derive(Debug, FromRow)]
struct TargetLibrary {
    path: String,
    role: String,
    writable: bool,
}

pub async fn preview(
    state: &AppState,
    user_id: Uuid,
    request: OrganizerPreviewRequest,
) -> Result<OperationResponse, AppError> {
    if request.media_ids.is_empty() {
        return Err(AppError::BadRequest("至少选择一个媒体文件".to_owned()));
    }
    validate_template(&request.template)?;
    let target_library = load_target_library(state, request.target_library_id).await?;
    if !target_library.writable || !matches!(target_library.role.as_str(), "managed" | "both") {
        return Err(AppError::BadRequest(
            "整理目标必须是允许写入的已整理曲库".to_owned(),
        ));
    }
    let target_root = Path::new(&target_library.path)
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("整理目标不可访问：{error}")))?;
    let operation_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO operations (id, kind, status, request_json, created_by, created_at) VALUES (?, 'organize', 'previewed', ?, ?, ?)",
    )
    .bind(operation_id)
    .bind(serde_json::to_string(&request).map_err(AppError::internal)?)
    .bind(user_id)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let mut items = Vec::new();
    for media_id in &request.media_ids {
        let source = load_source(state, *media_id).await?;
        let source_path = safe_source_path(&source)?;
        let relative_target =
            render_target(&source, &request.template, request.cross_platform_safe)?;
        let target_path = target_root.join(relative_target);
        ensure_lexically_inside(&target_root, &target_path)?;
        let preflight = preflight(&source_path, &target_root, &target_path)?;
        let item_id = Uuid::new_v4();
        let error_message = (!preflight.can_apply).then(|| {
            if !preflight.same_filesystem {
                "源文件与整理目录不在同一文件系统，Hardlink 禁止 Copy fallback".to_owned()
            } else {
                "目标路径已存在且不是同一 inode，拒绝覆盖".to_owned()
            }
        });
        let preflight_json = serde_json::to_string(&preflight).map_err(AppError::internal)?;
        sqlx::query(
            r#"INSERT INTO operation_items
              (id, operation_id, media_file_id, action, status, source_path, target_path,
               preflight_json, error_message, retryable, created_at, updated_at)
              VALUES (?, ?, ?, 'hardlink', 'previewed', ?, ?, ?, ?, 1, ?, ?)"#,
        )
        .bind(item_id)
        .bind(operation_id)
        .bind(source.media_id)
        .bind(source_path.to_string_lossy().into_owned())
        .bind(target_path.to_string_lossy().into_owned())
        .bind(&preflight_json)
        .bind(&error_message)
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        items.push(OperationItemResponse {
            id: item_id,
            media_file_id: Some(source.media_id),
            source_path: Some(source_path.to_string_lossy().into_owned()),
            target_path: Some(target_path.to_string_lossy().into_owned()),
            status: "previewed".to_owned(),
            diffs: Vec::new(),
            error_message,
            preflight: Some(serde_json::to_value(preflight).map_err(AppError::internal)?),
        });
    }
    Ok(OperationResponse {
        id: operation_id,
        kind: "organize".to_owned(),
        status: "previewed".to_owned(),
        items,
    })
}

pub async fn apply(
    state: &AppState,
    request: OrganizerApplyRequest,
) -> Result<OperationResponse, AppError> {
    require_confirmation(&request.confirmation)?;
    let (kind, status, request_json) = sqlx::query_as::<_, (String, String, String)>(
        "SELECT kind, status, request_json FROM operations WHERE id = ?",
    )
    .bind(request.operation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("整理预览不存在".to_owned()))?;
    if kind != "organize" || status != "previewed" {
        return Err(AppError::Conflict(
            "只有未执行的整理预览可以确认执行".to_owned(),
        ));
    }
    let preview_request: OrganizerPreviewRequest =
        serde_json::from_str(&request_json).map_err(AppError::internal)?;
    sqlx::query("UPDATE operations SET status = 'running', confirmed_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    crate::jobs::start_operation_job(state, request.operation_id, "organize").await?;
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, source_path, target_path FROM operation_items WHERE operation_id = ? AND status = 'previewed'",
    )
    .bind(request.operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut failures = 0_u64;
    for (item_id, source, target) in rows {
        let result = create_hardlink(Path::new(&source), Path::new(&target)).await;
        let (item_status, error_message, after_json) = match result {
            Ok(created) => (
                "success",
                None,
                Some(serde_json::json!({ "created": created }).to_string()),
            ),
            Err(error) => {
                failures += 1;
                ("failed", Some(error.to_string()), None)
            }
        };
        sqlx::query("UPDATE operation_items SET status = ?, error_message = ?, after_json = ?, updated_at = ? WHERE id = ?")
            .bind(item_status)
            .bind(error_message.as_deref())
            .bind(after_json)
            .bind(Utc::now())
            .bind(item_id)
            .execute(&state.pool)
            .await
            .map_err(AppError::internal)?;
        crate::jobs::record_operation_item(
            state,
            request.operation_id,
            item_id,
            &source,
            item_status == "success",
            error_message.as_deref(),
        )
        .await?;
    }
    let final_status = if failures == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
    sqlx::query("UPDATE operations SET status = ?, finished_at = ? WHERE id = ?")
        .bind(final_status)
        .bind(Utc::now())
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let _ = crate::scanner::enqueue_scan(state.clone(), preview_request.target_library_id).await;
    crate::jobs::finish_operation_job(state, request.operation_id).await?;
    crate::tag_operations::get_operation(state, request.operation_id).await
}

pub async fn undo(
    state: &AppState,
    request: OrganizerUndoRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "UNDO" {
        return Err(AppError::BadRequest(
            "撤销整理必须提交 confirmation=UNDO".to_owned(),
        ));
    }
    let (kind, status, request_json) = sqlx::query_as::<_, (String, String, String)>(
        "SELECT kind, status, request_json FROM operations WHERE id = ?",
    )
    .bind(request.operation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("整理操作不存在".to_owned()))?;
    if kind != "organize" || !matches!(status.as_str(), "completed" | "completed_with_errors") {
        return Err(AppError::Conflict("该整理操作当前不能撤销".to_owned()));
    }
    let preview_request: OrganizerPreviewRequest =
        serde_json::from_str(&request_json).map_err(AppError::internal)?;
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        "SELECT id, source_path, target_path, after_json FROM operation_items WHERE operation_id = ? AND status = 'success'",
    )
    .bind(request.operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for (item_id, source, target, after_json) in rows {
        let created = after_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .and_then(|value| value.get("created").and_then(serde_json::Value::as_bool))
            .unwrap_or(false);
        if created {
            remove_created_link(Path::new(&source), Path::new(&target)).await?;
        }
        sqlx::query(
            "UPDATE operation_items SET status = 'rolled_back', updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now())
        .bind(item_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    }
    sqlx::query("UPDATE operations SET status = 'rolled_back', rolled_back_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let _ = crate::scanner::enqueue_scan(state.clone(), preview_request.target_library_id).await;
    crate::tag_operations::get_operation(state, request.operation_id).await
}

pub async fn retry_failed(
    state: &AppState,
    request: OrganizerApplyRequest,
) -> Result<OperationResponse, AppError> {
    require_confirmation(&request.confirmation)?;
    let changed = sqlx::query(
        "UPDATE operations SET status = 'previewed', finished_at = NULL WHERE id = ? AND kind = 'organize' AND status = 'completed_with_errors'",
    )
    .bind(request.operation_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "该整理操作当前没有可重试失败项".to_owned(),
        ));
    }
    sqlx::query("UPDATE operation_items SET status = 'previewed', error_message = NULL WHERE operation_id = ? AND status = 'failed'")
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    apply(state, request).await
}

async fn create_hardlink(source: &Path, target: &Path) -> Result<bool, AppError> {
    let source_meta = tokio::fs::metadata(source)
        .await
        .map_err(|error| AppError::BadRequest(format!("源文件不可访问：{error}")))?;
    if let Ok(target_meta) = tokio::fs::metadata(target).await {
        if same_physical(&source_meta, &target_meta) {
            return Ok(false);
        }
        return Err(AppError::Conflict(
            "目标路径已存在且不是同一 inode，拒绝覆盖".to_owned(),
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| AppError::BadRequest("目标路径没有父目录".to_owned()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| AppError::BadRequest(format!("无法创建目标目录：{error}")))?;
    let parent_meta = tokio::fs::metadata(parent)
        .await
        .map_err(AppError::internal)?;
    if device_id(&source_meta) != device_id(&parent_meta) {
        return Err(AppError::BadRequest(
            "源文件与目标目录跨文件系统，Hardlink 不会回退为 Copy".to_owned(),
        ));
    }
    tokio::fs::hard_link(source, target)
        .await
        .map_err(|error| AppError::BadRequest(format!("创建 Hardlink 失败：{error}")))?;
    Ok(true)
}

async fn remove_created_link(source: &Path, target: &Path) -> Result<(), AppError> {
    let source_meta = tokio::fs::metadata(source)
        .await
        .map_err(|error| AppError::BadRequest(format!("撤销前无法读取源文件：{error}")))?;
    let target_meta = tokio::fs::metadata(target)
        .await
        .map_err(|error| AppError::BadRequest(format!("撤销前无法读取目标文件：{error}")))?;
    if !same_physical(&source_meta, &target_meta) {
        return Err(AppError::Conflict(
            "目标文件 inode 已改变，拒绝撤销以避免删除其他文件".to_owned(),
        ));
    }
    tokio::fs::remove_file(target)
        .await
        .map_err(|error| AppError::BadRequest(format!("撤销 Hardlink 失败：{error}")))
}

fn preflight(
    source: &Path,
    target_root: &Path,
    target: &Path,
) -> Result<OrganizerPreflight, AppError> {
    let source_meta = source.metadata().map_err(AppError::internal)?;
    let root_meta = target_root.metadata().map_err(AppError::internal)?;
    let same_filesystem = device_id(&source_meta) == device_id(&root_meta);
    let target_meta = target.metadata().ok();
    let target_exists = target_meta.is_some();
    let same_inode = target_meta
        .as_ref()
        .is_some_and(|meta| same_physical(&source_meta, meta));
    let path_conflict = target_exists && !same_inode;
    Ok(OrganizerPreflight {
        same_filesystem,
        target_exists,
        same_inode,
        path_conflict,
        can_apply: same_filesystem && !path_conflict,
    })
}

async fn load_target_library(state: &AppState, id: Uuid) -> Result<TargetLibrary, AppError> {
    sqlx::query_as::<_, TargetLibrary>("SELECT path, role, writable FROM libraries WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::NotFound("整理目标曲库不存在".to_owned()))
}

async fn load_source(state: &AppState, id: Uuid) -> Result<OrganizerSource, AppError> {
    sqlx::query_as::<_, OrganizerSource>(
        r#"SELECT mf.id AS media_id, l.path AS source_library_path, mf.relative_path,
          mf.extension, t.title,
          COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY ta.position), '未知艺术家') AS artist,
          al.album_artist, COALESCE(al.title, '未分类') AS album, t.track_no, t.disc_no,
          t.year, t.genre, mf.sample_rate, mf.bit_depth
          FROM media_files mf JOIN libraries l ON l.id = mf.library_id
          JOIN tracks t ON t.id = mf.track_id LEFT JOIN albums al ON al.id = t.album_id
          WHERE mf.id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("整理源媒体文件不存在".to_owned()))
}

fn safe_source_path(source: &OrganizerSource) -> Result<PathBuf, AppError> {
    let root = Path::new(&source.source_library_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    let path = root
        .join(&source.relative_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    if !path.starts_with(root) {
        return Err(AppError::BadRequest("整理源路径超出曲库范围".to_owned()));
    }
    Ok(path)
}

fn render_target(
    source: &OrganizerSource,
    template: &str,
    cross_platform_safe: bool,
) -> Result<PathBuf, AppError> {
    let artist = first_artist(&source.artist);
    let values = [
        (
            "{artist}",
            sanitize(artist, cross_platform_safe, "未知艺术家"),
        ),
        (
            "{artist_initial}",
            sanitize(&artist_initial(artist), cross_platform_safe, "_"),
        ),
        (
            "{album_artist}",
            sanitize(
                source.album_artist.as_deref().unwrap_or(artist),
                cross_platform_safe,
                "未知艺术家",
            ),
        ),
        (
            "{album}",
            sanitize(&source.album, cross_platform_safe, "未分类"),
        ),
        (
            "{title}",
            sanitize(&source.title, cross_platform_safe, "未命名"),
        ),
        ("{track}", source.track_no.unwrap_or(0).to_string()),
        ("{track:02}", format!("{:02}", source.track_no.unwrap_or(0))),
        ("{disc}", source.disc_no.unwrap_or(0).to_string()),
        ("{disc:02}", format!("{:02}", source.disc_no.unwrap_or(0))),
        (
            "{year}",
            source
                .year
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知年份".to_owned()),
        ),
        (
            "{genre}",
            sanitize(
                source.genre.as_deref().unwrap_or("未分类"),
                cross_platform_safe,
                "未分类",
            ),
        ),
        ("{ext}", source.extension.clone()),
        ("{quality}", quality_label(source)),
    ];
    let mut rendered = template.to_owned();
    for (placeholder, value) in values {
        rendered = rendered.replace(placeholder, &value);
    }
    if rendered.contains('{') || rendered.contains('}') {
        return Err(AppError::BadRequest(
            "Organizer 模板包含未知变量".to_owned(),
        ));
    }
    let path = PathBuf::from(rendered);
    if path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AppError::BadRequest(
            "Organizer 模板生成了不安全路径".to_owned(),
        ));
    }
    Ok(path)
}

fn validate_template(template: &str) -> Result<(), AppError> {
    if template.trim().is_empty()
        || template.len() > 512
        || !template.contains("{title}")
        || !template.contains("{ext}")
    {
        return Err(AppError::BadRequest(
            "Organizer 模板必须包含 {title} 和 {ext}，且不超过 512 字节".to_owned(),
        ));
    }
    Ok(())
}

fn sanitize(value: &str, cross_platform_safe: bool, fallback: &str) -> String {
    let mut cleaned = value
        .chars()
        .map(|character| {
            let forbidden = character == '/'
                || character == '\\'
                || character == '\0'
                || character.is_control()
                || (cross_platform_safe
                    && matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|'));
            if forbidden { ' ' } else { character }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches([' ', '.'])
        .chars()
        .take(120)
        .collect::<String>();
    if cleaned.is_empty() || matches!(cleaned.as_str(), "." | "..") {
        cleaned = fallback.to_owned();
    }
    cleaned
}

fn first_artist(value: &str) -> &str {
    value
        .split([';', '、', '&'])
        .next()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("未知艺术家")
}

fn quality_label(source: &OrganizerSource) -> String {
    match (source.bit_depth, source.sample_rate) {
        (Some(depth), Some(rate)) => format!("{depth}bit-{}kHz", rate / 1000),
        _ => "unknown".to_owned(),
    }
}

fn ensure_lexically_inside(root: &Path, target: &Path) -> Result<(), AppError> {
    if target.starts_with(root) {
        Ok(())
    } else {
        Err(AppError::BadRequest("整理目标超出曲库范围".to_owned()))
    }
}

fn require_confirmation(value: &str) -> Result<(), AppError> {
    if value == "APPLY" {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "确认整理必须提交 confirmation=APPLY".to_owned(),
        ))
    }
}

fn default_template() -> String {
    DEFAULT_TEMPLATE.to_owned()
}

fn default_true() -> bool {
    true
}

#[cfg(unix)]
fn device_id(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.dev()
}

#[cfg(not(unix))]
fn device_id(_metadata: &std::fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn same_physical(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_physical(_left: &std::fs::Metadata, _right: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_unsafe_segments() {
        assert_eq!(sanitize("../ 周杰伦 /: *", true, "x"), "周杰伦");
        assert_eq!(sanitize("..", true, "fallback"), "fallback");
    }

    #[test]
    fn template_requires_title_and_extension() {
        assert!(validate_template(DEFAULT_TEMPLATE).is_ok());
        assert!(validate_template("{artist}/{title}").is_err());
    }
}
