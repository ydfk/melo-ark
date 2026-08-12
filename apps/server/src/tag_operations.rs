use std::collections::HashSet;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Utc;
use ferrous_opencc::{OpenCC, config::BuiltinConfig};
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    picture::{Picture, PictureType},
    prelude::Accessor,
    probe::Probe,
    tag::{ItemKey, Tag},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TagValues {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub cover: Option<CoverData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CoverData {
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagSet {
    pub title: Option<String>,
    pub artists: Option<Vec<String>>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub cover_data_base64: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TagField {
    Title,
    Artists,
    Album,
    AlbumArtist,
    TrackNo,
    DiscNo,
    Year,
    Genre,
    Cover,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TagTransform {
    Trim {
        fields: Vec<TagField>,
    },
    FindReplace {
        fields: Vec<TagField>,
        find: String,
        replacement: String,
    },
    RegexReplace {
        fields: Vec<TagField>,
        pattern: String,
        replacement: String,
    },
    TraditionalToSimplified {
        fields: Vec<TagField>,
    },
    NormalizePunctuation {
        fields: Vec<TagField>,
    },
    FilenameToTags,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagPreviewRequest {
    pub media_ids: Vec<Uuid>,
    #[serde(default)]
    pub set: TagSet,
    #[serde(default)]
    pub clear: Vec<TagField>,
    #[serde(default)]
    pub transforms: Vec<TagTransform>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOperationRequest {
    pub operation_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UndoOperationRequest {
    pub operation_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagDiff {
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationItemResponse {
    pub id: Uuid,
    pub media_file_id: Option<Uuid>,
    pub source_path: Option<String>,
    pub target_path: Option<String>,
    pub status: String,
    pub diffs: Vec<TagDiff>,
    pub error_message: Option<String>,
    pub preflight: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationResponse {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub items: Vec<OperationItemResponse>,
}

#[derive(Debug, FromRow)]
struct MediaTarget {
    id: Uuid,
    library_path: String,
    relative_path: String,
    writable: bool,
    file_size: i64,
    mtime_ms: i64,
    device_id: String,
    inode: String,
}

pub async fn preview(
    state: &AppState,
    user_id: Uuid,
    request: TagPreviewRequest,
) -> Result<OperationResponse, AppError> {
    if request.media_ids.is_empty() {
        return Err(AppError::BadRequest("至少选择一个媒体文件".to_owned()));
    }
    validate_transforms(&request.transforms)?;
    let operation_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO operations (id, kind, status, request_json, created_by, created_at) VALUES (?, 'tag_edit', 'previewed', ?, ?, ?)",
    )
    .bind(operation_id)
    .bind(serde_json::to_string(&request).map_err(AppError::internal)?)
    .bind(user_id)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let mut items = Vec::new();
    let mut physical_files = HashSet::new();
    for media_id in &request.media_ids {
        let target = load_target(state, *media_id).await?;
        if !physical_files.insert((target.device_id.clone(), target.inode.clone())) {
            continue;
        }
        let path = safe_media_path(&target)?;
        let item_id = Uuid::new_v4();
        let result = read_tag_values(&path).and_then(|before| {
            let after = apply_request(before.clone(), &path, &request)?;
            Ok((before, after))
        });
        let (status, before_json, after_json, diffs, error_message) = match result {
            Ok((before, after)) => (
                "previewed",
                Some(serde_json::to_string(&before).map_err(AppError::internal)?),
                Some(serde_json::to_string(&after).map_err(AppError::internal)?),
                tag_diffs(&before, &after),
                None,
            ),
            Err(error) => ("failed", None, None, Vec::new(), Some(error.to_string())),
        };
        sqlx::query(
            r#"INSERT INTO operation_items
              (id, operation_id, media_file_id, action, status, before_json, after_json,
               source_path, error_message, retryable, created_at, updated_at)
              VALUES (?, ?, ?, 'tag_edit', ?, ?, ?, ?, ?, 1, ?, ?)"#,
        )
        .bind(item_id)
        .bind(operation_id)
        .bind(target.id)
        .bind(status)
        .bind(&before_json)
        .bind(&after_json)
        .bind(path.to_string_lossy().into_owned())
        .bind(&error_message)
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        items.push(OperationItemResponse {
            id: item_id,
            media_file_id: Some(target.id),
            source_path: Some(path.to_string_lossy().into_owned()),
            target_path: None,
            status: status.to_owned(),
            diffs,
            error_message,
            preflight: None,
        });
    }
    Ok(OperationResponse {
        id: operation_id,
        kind: "tag_edit".to_owned(),
        status: "previewed".to_owned(),
        items,
    })
}

pub async fn apply(
    state: &AppState,
    request: ApplyOperationRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "APPLY" {
        return Err(AppError::BadRequest(
            "确认写入必须提交 confirmation=APPLY".to_owned(),
        ));
    }
    let operation =
        sqlx::query_as::<_, (String, String)>("SELECT kind, status FROM operations WHERE id = ?")
            .bind(request.operation_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::NotFound("操作预览不存在".to_owned()))?;
    if operation.0 != "tag_edit" || operation.1 != "previewed" {
        return Err(AppError::Conflict(
            "只有未执行的 Tag 预览可以确认写入".to_owned(),
        ));
    }
    let now = Utc::now();
    sqlx::query("UPDATE operations SET status = 'running', confirmed_at = ? WHERE id = ?")
        .bind(now)
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    crate::jobs::start_operation_job(state, request.operation_id, "tag_edit").await?;

    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String)>(
        "SELECT id, media_file_id, before_json, after_json FROM operation_items WHERE operation_id = ? AND status = 'previewed'",
    )
    .bind(request.operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut failures = 0_u64;
    for (item_id, media_id, before_json, after_json) in rows {
        let target = load_target(state, media_id).await?;
        let path = safe_media_path(&target)?;
        let before: TagValues = serde_json::from_str(&before_json).map_err(AppError::internal)?;
        let after: TagValues = serde_json::from_str(&after_json).map_err(AppError::internal)?;
        let result = apply_one(state, request.operation_id, &target, &path, &before, &after).await;
        let (status, error) = match result {
            Ok(()) => ("success", None),
            Err(error) => {
                failures += 1;
                ("failed", Some(error.to_string()))
            }
        };
        sqlx::query(
            "UPDATE operation_items SET status = ?, error_message = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(error.as_deref())
        .bind(Utc::now())
        .bind(item_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        crate::jobs::record_operation_item(
            state,
            request.operation_id,
            item_id,
            &path.to_string_lossy(),
            status == "success",
            error.as_deref(),
        )
        .await?;
    }
    let status = if failures == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
    sqlx::query("UPDATE operations SET status = ?, finished_at = ? WHERE id = ?")
        .bind(status)
        .bind(Utc::now())
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let library_ids = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT DISTINCT mf.library_id FROM operation_items oi
           JOIN media_files mf ON mf.id = oi.media_file_id
           WHERE oi.operation_id = ? AND oi.status = 'success'"#,
    )
    .bind(request.operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for library_id in library_ids {
        let _ = crate::scanner::enqueue_scan(state.clone(), library_id).await;
    }
    crate::jobs::finish_operation_job(state, request.operation_id).await?;
    get_operation(state, request.operation_id).await
}

pub async fn retry_failed(
    state: &AppState,
    request: ApplyOperationRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "APPLY" {
        return Err(AppError::BadRequest(
            "重试写入必须提交 confirmation=APPLY".to_owned(),
        ));
    }
    let changed = sqlx::query(
        "UPDATE operations SET status = 'previewed', finished_at = NULL WHERE id = ? AND kind = 'tag_edit' AND status = 'completed_with_errors'",
    )
    .bind(request.operation_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "该 Tag 操作当前没有可重试失败项".to_owned(),
        ));
    }
    sqlx::query("UPDATE operation_items SET status = 'previewed', error_message = NULL WHERE operation_id = ? AND status = 'failed'")
        .bind(request.operation_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    apply(state, request).await
}

pub async fn undo(
    state: &AppState,
    request: UndoOperationRequest,
) -> Result<OperationResponse, AppError> {
    if request.confirmation != "UNDO" {
        return Err(AppError::BadRequest(
            "撤销 Tag 修改必须提交 confirmation=UNDO".to_owned(),
        ));
    }
    let (kind, status) =
        sqlx::query_as::<_, (String, String)>("SELECT kind, status FROM operations WHERE id = ?")
            .bind(request.operation_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::NotFound("Tag 操作不存在".to_owned()))?;
    if kind != "tag_edit" || !matches!(status.as_str(), "completed" | "completed_with_errors") {
        return Err(AppError::Conflict("该 Tag 操作当前不能撤销".to_owned()));
    }
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "SELECT id, media_file_id, before_json FROM operation_items WHERE operation_id = ? AND status = 'success'",
    )
    .bind(request.operation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut library_ids = Vec::new();
    for (item_id, media_id, before_json) in rows {
        let target = load_target(state, media_id).await?;
        let path = safe_media_path(&target)?;
        let metadata = path.metadata().map_err(AppError::internal)?;
        if physical_identity(&metadata) != (target.device_id.clone(), target.inode.clone()) {
            return Err(AppError::Conflict(
                "媒体文件物理身份已变化，拒绝覆盖式撤销".to_owned(),
            ));
        }
        let before: TagValues = serde_json::from_str(&before_json).map_err(AppError::internal)?;
        write_tag_values(&path, &before)?;
        invalidate_physical_index(state, &target).await?;
        let library_id =
            sqlx::query_scalar::<_, Uuid>("SELECT library_id FROM media_files WHERE id = ?")
                .bind(media_id)
                .fetch_one(&state.pool)
                .await
                .map_err(AppError::internal)?;
        library_ids.push(library_id);
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
    library_ids.sort_unstable();
    library_ids.dedup();
    for library_id in library_ids {
        let _ = crate::scanner::enqueue_scan(state.clone(), library_id).await;
    }
    get_operation(state, request.operation_id).await
}

async fn apply_one(
    state: &AppState,
    operation_id: Uuid,
    target: &MediaTarget,
    path: &Path,
    before: &TagValues,
    after: &TagValues,
) -> Result<(), AppError> {
    if !target.writable || !metadata_extension_writable(path) {
        return Err(AppError::BadRequest(
            "曲库或该格式未允许写入，拒绝修改标签".to_owned(),
        ));
    }
    sqlx::query(
        r#"INSERT INTO embedded_metadata_snapshots
          (id, media_file_id, operation_id, metadata_json, file_size, mtime_ms, device_id, inode, created_at)
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(Uuid::new_v4())
    .bind(target.id)
    .bind(operation_id)
    .bind(serde_json::to_string(before).map_err(AppError::internal)?)
    .bind(target.file_size)
    .bind(target.mtime_ms)
    .bind(&target.device_id)
    .bind(&target.inode)
    .bind(Utc::now())
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    write_tag_values(path, after)?;
    invalidate_physical_index(state, target).await?;
    Ok(())
}

async fn invalidate_physical_index(state: &AppState, target: &MediaTarget) -> Result<(), AppError> {
    sqlx::query("UPDATE media_files SET mtime_ms = -1 WHERE device_id = ? AND inode = ?")
        .bind(&target.device_id)
        .bind(&target.inode)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    Ok(())
}

pub async fn get_operation(state: &AppState, id: Uuid) -> Result<OperationResponse, AppError> {
    let (kind, status) =
        sqlx::query_as::<_, (String, String)>("SELECT kind, status FROM operations WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::NotFound("操作不存在".to_owned()))?;
    let rows = sqlx::query_as::<_, (Uuid, Option<Uuid>, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, media_file_id, source_path, target_path, status, before_json, after_json, error_message, preflight_json FROM operation_items WHERE operation_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let items = rows
        .into_iter()
        .map(|row| {
            let diffs = row
                .5
                .as_deref()
                .zip(row.6.as_deref())
                .and_then(|(before, after)| {
                    Some(tag_diffs(
                        &serde_json::from_str(before).ok()?,
                        &serde_json::from_str(after).ok()?,
                    ))
                })
                .unwrap_or_default();
            OperationItemResponse {
                id: row.0,
                media_file_id: row.1,
                source_path: row.2,
                target_path: row.3,
                status: row.4,
                diffs,
                error_message: row.7,
                preflight: row
                    .8
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok()),
            }
        })
        .collect();
    Ok(OperationResponse {
        id,
        kind,
        status,
        items,
    })
}

async fn load_target(state: &AppState, media_id: Uuid) -> Result<MediaTarget, AppError> {
    sqlx::query_as::<_, MediaTarget>(
        r#"SELECT mf.id, l.path AS library_path, mf.relative_path, l.writable,
          mf.file_size, mf.mtime_ms, mf.device_id, mf.inode
          FROM media_files mf JOIN libraries l ON l.id = mf.library_id WHERE mf.id = ?"#,
    )
    .bind(media_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("媒体文件不存在".to_owned()))
}

fn metadata_extension_writable(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| {
            matches!(
                extension.as_str(),
                "mp3" | "flac" | "m4a" | "mp4" | "ogg" | "opus" | "wav" | "aiff" | "aif"
            )
        })
}

#[cfg(unix)]
fn physical_identity(metadata: &std::fs::Metadata) -> (String, String) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev().to_string(), metadata.ino().to_string())
}

#[cfg(not(unix))]
fn physical_identity(_metadata: &std::fs::Metadata) -> (String, String) {
    ("unsupported".to_owned(), "unsupported".to_owned())
}

fn safe_media_path(target: &MediaTarget) -> Result<PathBuf, AppError> {
    let root = Path::new(&target.library_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    let path = root.join(&target.relative_path);
    let canonical = path
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("媒体文件不可访问：{error}")))?;
    if !canonical.starts_with(&root) {
        return Err(AppError::BadRequest(
            "媒体文件路径超出曲库范围，拒绝操作".to_owned(),
        ));
    }
    Ok(canonical)
}

fn read_tag_values(path: &Path) -> Result<TagValues, AppError> {
    let tagged = Probe::open(path)
        .and_then(|probe| probe.read())
        .map_err(|error| AppError::BadRequest(format!("Tag 读取失败：{error}")))?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    Ok(tag.map_or_else(TagValues::default, values_from_tag))
}

fn values_from_tag(tag: &Tag) -> TagValues {
    TagValues {
        title: tag
            .title()
            .map(|value| value.into_owned())
            .unwrap_or_default(),
        artists: tag
            .artist()
            .map(|value| {
                value
                    .split([';', '、', '&'])
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        album: tag
            .album()
            .map(|value| value.into_owned())
            .unwrap_or_default(),
        album_artist: tag.get_string(ItemKey::AlbumArtist).map(ToOwned::to_owned),
        track_no: tag.track(),
        disc_no: tag.disk(),
        year: tag.date().map(|value| u32::from(value.year)),
        genre: tag.genre().map(|value| value.into_owned()),
        cover: tag
            .get_picture_type(PictureType::CoverFront)
            .or_else(|| tag.pictures().first())
            .map(|picture| CoverData {
                mime_type: picture
                    .mime_type()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "application/octet-stream".to_owned()),
                data_base64: STANDARD.encode(picture.data()),
            }),
    }
}

fn write_tag_values(path: &Path, values: &TagValues) -> Result<(), AppError> {
    let mut tagged = Probe::open(path)
        .and_then(|probe| probe.read())
        .map_err(|error| AppError::BadRequest(format!("Tag 读取失败：{error}")))?;
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(Tag::new(tagged.primary_tag_type()));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| AppError::BadRequest("该音频格式不能创建主 Tag".to_owned()))?;
    set_or_remove_text(
        tag,
        TagField::Title,
        (!values.title.is_empty()).then_some(&values.title),
    );
    let artists = values.artists.join("; ");
    set_or_remove_text(
        tag,
        TagField::Artists,
        (!artists.is_empty()).then_some(&artists),
    );
    set_or_remove_text(
        tag,
        TagField::Album,
        (!values.album.is_empty()).then_some(&values.album),
    );
    set_or_remove_text(tag, TagField::AlbumArtist, values.album_artist.as_ref());
    set_or_remove_text(tag, TagField::Genre, values.genre.as_ref());
    match values.track_no {
        Some(value) => tag.set_track(value),
        None => tag.remove_track(),
    }
    match values.disc_no {
        Some(value) => tag.set_disk(value),
        None => tag.remove_disk(),
    }
    match values.year {
        Some(value) => {
            tag.insert_text(ItemKey::RecordingDate, value.to_string());
        }
        None => tag.remove_date(),
    }
    tag.remove_picture_type(PictureType::CoverFront);
    if let Some(cover) = &values.cover {
        let data = STANDARD
            .decode(&cover.data_base64)
            .map_err(|error| AppError::BadRequest(format!("封面 Base64 无效：{error}")))?;
        let mut picture = Picture::from_reader(&mut std::io::Cursor::new(data))
            .map_err(|error| AppError::BadRequest(format!("封面格式无效：{error}")))?;
        picture.set_pic_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }
    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|error| AppError::BadRequest(format!("Tag 写入失败：{error}")))
}

fn set_or_remove_text(tag: &mut Tag, field: TagField, value: Option<&String>) {
    let Some(key) = field_key(field) else {
        return;
    };
    if let Some(value) = value {
        tag.insert_text(key, value.clone());
    } else {
        tag.remove_key(key);
    }
}

fn field_key(field: TagField) -> Option<ItemKey> {
    Some(match field {
        TagField::Title => ItemKey::TrackTitle,
        TagField::Artists => ItemKey::TrackArtist,
        TagField::Album => ItemKey::AlbumTitle,
        TagField::AlbumArtist => ItemKey::AlbumArtist,
        TagField::Genre => ItemKey::Genre,
        TagField::TrackNo => ItemKey::TrackNumber,
        TagField::DiscNo => ItemKey::DiscNumber,
        TagField::Year => ItemKey::RecordingDate,
        TagField::Cover => return None,
    })
}

fn apply_request(
    mut values: TagValues,
    path: &Path,
    request: &TagPreviewRequest,
) -> Result<TagValues, AppError> {
    if let Some(value) = &request.set.title {
        values.title.clone_from(value);
    }
    if let Some(value) = &request.set.artists {
        values.artists.clone_from(value);
    }
    if let Some(value) = &request.set.album {
        values.album.clone_from(value);
    }
    if let Some(value) = &request.set.album_artist {
        values.album_artist = Some(value.clone());
    }
    if let Some(value) = request.set.track_no {
        values.track_no = Some(value);
    }
    if let Some(value) = request.set.disc_no {
        values.disc_no = Some(value);
    }
    if let Some(value) = request.set.year {
        values.year = Some(value);
    }
    if let Some(value) = &request.set.genre {
        values.genre = Some(value.clone());
    }
    if let Some(value) = &request.set.cover_data_base64 {
        values.cover = Some(validate_cover(value)?);
    }
    for field in &request.clear {
        clear_field(&mut values, *field);
    }
    for transform in &request.transforms {
        apply_transform(&mut values, path, transform)?;
    }
    Ok(values)
}

fn validate_transforms(transforms: &[TagTransform]) -> Result<(), AppError> {
    for transform in transforms {
        if let TagTransform::RegexReplace { pattern, .. } = transform {
            Regex::new(pattern)
                .map_err(|error| AppError::BadRequest(format!("正则表达式无效：{error}")))?;
        }
    }
    Ok(())
}

fn apply_transform(
    values: &mut TagValues,
    path: &Path,
    transform: &TagTransform,
) -> Result<(), AppError> {
    match transform {
        TagTransform::Trim { fields } => {
            mutate_text_fields(values, fields, |value| value.trim().to_owned())
        }
        TagTransform::FindReplace {
            fields,
            find,
            replacement,
        } => {
            mutate_text_fields(values, fields, |value| value.replace(find, replacement));
        }
        TagTransform::RegexReplace {
            fields,
            pattern,
            replacement,
        } => {
            let regex = Regex::new(pattern)
                .map_err(|error| AppError::BadRequest(format!("正则表达式无效：{error}")))?;
            mutate_text_fields(values, fields, |value| {
                regex.replace_all(value, replacement).into_owned()
            });
        }
        TagTransform::TraditionalToSimplified { fields } => {
            let converter = OpenCC::from_config(BuiltinConfig::T2s)
                .map_err(|error| AppError::internal(format!("简繁转换初始化失败：{error}")))?;
            mutate_text_fields(values, fields, |value| converter.convert(value));
        }
        TagTransform::NormalizePunctuation { fields } => {
            mutate_text_fields(values, fields, normalize_punctuation);
        }
        TagTransform::FilenameToTags => parse_filename(values, path),
    }
    Ok(())
}

fn normalize_punctuation(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '，' => ',',
            '。' => '.',
            '：' => ':',
            '；' => ';',
            '！' => '!',
            '？' => '?',
            '（' => '(',
            '）' => ')',
            '【' | '〔' => '[',
            '】' | '〕' => ']',
            '“' | '”' | '「' | '」' | '『' | '』' => '"',
            '‘' | '’' => '\'',
            '—' | '–' | '－' => '-',
            '＆' => '&',
            '　' => ' ',
            other => other,
        })
        .collect()
}

fn mutate_text_fields(
    values: &mut TagValues,
    fields: &[TagField],
    mutate: impl Fn(&str) -> String,
) {
    for field in fields {
        match field {
            TagField::Title => values.title = mutate(&values.title),
            TagField::Artists => {
                values.artists = values.artists.iter().map(|value| mutate(value)).collect();
            }
            TagField::Album => values.album = mutate(&values.album),
            TagField::AlbumArtist => {
                values.album_artist = values.album_artist.as_deref().map(&mutate);
            }
            TagField::Genre => values.genre = values.genre.as_deref().map(&mutate),
            TagField::TrackNo | TagField::DiscNo | TagField::Year | TagField::Cover => {}
        }
    }
}

fn parse_filename(values: &mut TagValues, path: &Path) {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut parts = stem.splitn(2, " - ");
    let first = parts.next().unwrap_or_default().trim();
    if let Some(title) = parts.next() {
        values.title = title.trim().to_owned();
        if let Ok(track) = first.parse::<u32>() {
            values.track_no = Some(track);
        }
    } else if !first.is_empty() {
        values.title = first.to_owned();
    }
}

fn clear_field(values: &mut TagValues, field: TagField) {
    match field {
        TagField::Title => values.title.clear(),
        TagField::Artists => values.artists.clear(),
        TagField::Album => values.album.clear(),
        TagField::AlbumArtist => values.album_artist = None,
        TagField::TrackNo => values.track_no = None,
        TagField::DiscNo => values.disc_no = None,
        TagField::Year => values.year = None,
        TagField::Genre => values.genre = None,
        TagField::Cover => values.cover = None,
    }
}

fn tag_diffs(before: &TagValues, after: &TagValues) -> Vec<TagDiff> {
    let fields = [
        (
            "title",
            Some(before.title.clone()),
            Some(after.title.clone()),
        ),
        (
            "artists",
            Some(before.artists.join(" / ")),
            Some(after.artists.join(" / ")),
        ),
        (
            "album",
            Some(before.album.clone()),
            Some(after.album.clone()),
        ),
        (
            "albumArtist",
            before.album_artist.clone(),
            after.album_artist.clone(),
        ),
        (
            "trackNo",
            before.track_no.map(|v| v.to_string()),
            after.track_no.map(|v| v.to_string()),
        ),
        (
            "discNo",
            before.disc_no.map(|v| v.to_string()),
            after.disc_no.map(|v| v.to_string()),
        ),
        (
            "year",
            before.year.map(|v| v.to_string()),
            after.year.map(|v| v.to_string()),
        ),
        ("genre", before.genre.clone(), after.genre.clone()),
        (
            "cover",
            before.cover.as_ref().map(cover_summary),
            after.cover.as_ref().map(cover_summary),
        ),
    ];
    fields
        .into_iter()
        .filter(|(_, before, after)| before != after)
        .map(|(field, before, after)| TagDiff {
            field: field.to_owned(),
            before,
            after,
        })
        .collect()
}

fn validate_cover(value: &str) -> Result<CoverData, AppError> {
    let data = STANDARD
        .decode(value)
        .map_err(|error| AppError::BadRequest(format!("封面 Base64 无效：{error}")))?;
    if data.len() > 10 * 1024 * 1024 {
        return Err(AppError::BadRequest("封面不能超过 10 MiB".to_owned()));
    }
    let picture = Picture::from_reader(&mut std::io::Cursor::new(&data))
        .map_err(|error| AppError::BadRequest(format!("封面只支持可识别的图片格式：{error}")))?;
    let mime_type = picture
        .mime_type()
        .map(ToString::to_string)
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    if !matches!(
        mime_type.as_str(),
        "image/jpeg" | "image/png" | "image/webp"
    ) {
        return Err(AppError::BadRequest(
            "封面首版仅支持 JPEG、PNG、WebP".to_owned(),
        ));
    }
    Ok(CoverData {
        mime_type,
        data_base64: STANDARD.encode(data),
    })
}

fn cover_summary(cover: &CoverData) -> String {
    let bytes = cover.data_base64.len() * 3 / 4;
    format!("{} · {} KiB", cover.mime_type, bytes / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_parser_extracts_track_and_title() {
        let mut values = TagValues::default();
        parse_filename(&mut values, Path::new("03 - 晴天.flac"));
        assert_eq!(values.track_no, Some(3));
        assert_eq!(values.title, "晴天");
    }

    #[test]
    fn traditional_conversion_is_explicit_and_non_destructive() {
        let mut values = TagValues {
            title: "晴天".to_owned(),
            artists: vec!["周杰倫".to_owned()],
            ..TagValues::default()
        };
        apply_transform(
            &mut values,
            Path::new("x.flac"),
            &TagTransform::TraditionalToSimplified {
                fields: vec![TagField::Artists],
            },
        )
        .unwrap();
        assert_eq!(values.artists, ["周杰伦"]);
        assert_eq!(values.title, "晴天");
    }

    #[test]
    fn punctuation_normalization_preserves_text_content() {
        assert_eq!(
            normalize_punctuation("晴天（Live）—「周杰倫」！"),
            "晴天(Live)-\"周杰倫\"!"
        );
    }
}
