use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use axum::{
    body::Body,
    http::{HeaderMap, Response, StatusCode, header},
};
use chrono::{DateTime, Utc};
use lofty::{file::TaggedFileExt, picture::PictureType, probe::Probe};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Clone, FromRow)]
pub struct StreamTarget {
    pub id: Uuid,
    pub track_id: Uuid,
    pub library_path: String,
    pub relative_path: String,
    pub extension: String,
    pub file_size: i64,
    pub mtime_ms: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeQuery {
    pub profile: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScrobbleRequest {
    pub track_id: Uuid,
    pub media_file_id: Option<Uuid>,
    #[serde(default)]
    pub completed: bool,
    pub position_sec: Option<i64>,
    pub client: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackHistory {
    pub id: Uuid,
    pub track_id: Uuid,
    pub title: String,
    pub artist: String,
    pub client: String,
    pub played_at: DateTime<Utc>,
    pub completed: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistRequest {
    pub name: String,
    pub comment: Option<String>,
    #[serde(default)]
    pub track_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlaylistRequest {
    pub name: Option<String>,
    pub comment: Option<String>,
    pub track_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: Uuid,
    pub name: String,
    pub comment: Option<String>,
    pub song_count: i64,
    pub duration_sec: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn stream_media(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
) -> Result<Response<Body>, AppError> {
    let target = load_media(state, id).await?;
    let path = safe_path(&target)?;
    ranged_file(path, mime_for_extension(&target.extension), headers).await
}

pub async fn artwork(state: &AppState, id: Uuid) -> Result<Response<Body>, AppError> {
    let target = load_media(state, id).await?;
    let path = safe_path(&target)?;
    let tagged = Probe::open(path)
        .and_then(|probe| probe.read())
        .map_err(|_| AppError::NotFound("封面不存在".to_owned()))?;
    let picture = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .and_then(|tag| {
            tag.get_picture_type(PictureType::CoverFront)
                .or_else(|| tag.pictures().first())
        })
        .ok_or_else(|| AppError::NotFound("封面不存在".to_owned()))?;
    let mime = picture
        .mime_type()
        .map(ToString::to_string)
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "private, max-age=86400")
        .body(Body::from(picture.data().to_vec()))
        .map_err(AppError::internal)
}

pub async fn transcode_media(
    state: &AppState,
    id: Uuid,
    profile: &str,
    headers: &HeaderMap,
) -> Result<Response<Body>, AppError> {
    if profile == "original" {
        return stream_media(state, id, headers).await;
    }
    let (extension, mime, args) = match profile {
        "opus-192" => (
            "opus",
            "audio/ogg",
            vec!["-c:a", "libopus", "-b:a", "192k", "-vn"],
        ),
        "aac-256" => (
            "m4a",
            "audio/mp4",
            vec![
                "-c:a",
                "aac",
                "-b:a",
                "256k",
                "-vn",
                "-movflags",
                "+faststart",
            ],
        ),
        "mp3-320" => (
            "mp3",
            "audio/mpeg",
            vec!["-c:a", "libmp3lame", "-b:a", "320k", "-vn"],
        ),
        _ => return Err(AppError::BadRequest("未知转码 Profile".to_owned())),
    };
    let target = load_media(state, id).await?;
    let source = safe_path(&target)?;
    let metadata = tokio::fs::metadata(&source)
        .await
        .map_err(AppError::internal)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let cache_key =
        blake3::hash(format!("{}:{}:{}:{profile}", id, metadata.len(), modified).as_bytes())
            .to_hex()
            .to_string();
    let cache_root = PathBuf::from(&state.playback.cache_dir);
    tokio::fs::create_dir_all(&cache_root)
        .await
        .map_err(AppError::internal)?;
    let output = cache_root.join(format!("{cache_key}.{extension}"));
    if !output.is_file() {
        let _permit = state
            .transcode_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(AppError::internal)?;
        if !output.is_file() {
            let temporary =
                cache_root.join(format!(".{cache_key}-{}.tmp.{extension}", Uuid::new_v4()));
            let result = tokio::process::Command::new(&state.playback.ffmpeg_path)
                .arg("-hide_banner")
                .arg("-loglevel")
                .arg("error")
                .arg("-y")
                .arg("-i")
                .arg(&source)
                .args(&args)
                .arg(&temporary)
                .stdin(Stdio::null())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|error| AppError::BadRequest(format!("FFmpeg 不可用：{error}")))?;
            if !result.status.success() {
                let _ = tokio::fs::remove_file(&temporary).await;
                return Err(AppError::BadRequest(format!(
                    "FFmpeg 转码失败：{}",
                    String::from_utf8_lossy(&result.stderr)
                )));
            }
            tokio::fs::rename(&temporary, &output)
                .await
                .map_err(AppError::internal)?;
        }
    }
    let size = tokio::fs::metadata(&output)
        .await
        .map_err(AppError::internal)?
        .len() as i64;
    let now = Utc::now();
    sqlx::query("INSERT INTO transcode_cache (cache_key, media_file_id, profile, path, file_size, last_accessed_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(cache_key) DO UPDATE SET last_accessed_at = excluded.last_accessed_at")
        .bind(&cache_key).bind(id).bind(profile).bind(output.to_string_lossy().into_owned()).bind(size).bind(now).bind(now).execute(&state.pool).await.map_err(AppError::internal)?;
    evict_cache(state).await?;
    ranged_file(output, mime, headers).await
}

async fn ranged_file(
    path: PathBuf,
    mime: &str,
    headers: &HeaderMap,
) -> Result<Response<Body>, AppError> {
    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(AppError::internal)?;
    let size = file.metadata().await.map_err(AppError::internal)?.len();
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| parse_range(value, size))
        .transpose()?;
    let (status, start, end) = range.map_or(
        (StatusCode::OK, 0, size.saturating_sub(1)),
        |(start, end)| (StatusCode::PARTIAL_CONTENT, start, end),
    );
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(AppError::internal)?;
    let length = if size == 0 { 0 } else { end - start + 1 };
    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length.to_string());
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    response
        .body(Body::from_stream(stream))
        .map_err(AppError::internal)
}

pub fn parse_range(value: &str, size: u64) -> Result<(u64, u64), AppError> {
    let value = value
        .strip_prefix("bytes=")
        .ok_or_else(|| AppError::BadRequest("只支持 bytes Range".to_owned()))?;
    if value.contains(',') || size == 0 {
        return Err(AppError::BadRequest(
            "仅支持单段且非空文件 Range".to_owned(),
        ));
    }
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| AppError::BadRequest("Range 格式无效".to_owned()))?;
    let (start, end) = if start.is_empty() {
        let suffix: u64 = end
            .parse()
            .map_err(|_| AppError::BadRequest("Range 格式无效".to_owned()))?;
        (size.saturating_sub(suffix.min(size)), size - 1)
    } else {
        let start: u64 = start
            .parse()
            .map_err(|_| AppError::BadRequest("Range 格式无效".to_owned()))?;
        let end = if end.is_empty() {
            size - 1
        } else {
            end.parse()
                .map_err(|_| AppError::BadRequest("Range 格式无效".to_owned()))?
        };
        (start, end.min(size - 1))
    };
    if start > end || start >= size {
        return Err(AppError::BadRequest("Range 超出文件范围".to_owned()));
    }
    Ok((start, end))
}

async fn evict_cache(state: &AppState) -> Result<(), AppError> {
    let cache_max_bytes = state
        .runtime
        .read()
        .await
        .editable
        .transcode_cache_max_bytes;
    let mut total: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(file_size),0) FROM transcode_cache")
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    if total <= cache_max_bytes {
        return Ok(());
    }
    let entries = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT cache_key, path, file_size FROM transcode_cache ORDER BY last_accessed_at ASC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for (key, path, size) in entries {
        if total <= cache_max_bytes {
            break;
        }
        let _ = tokio::fs::remove_file(path).await;
        sqlx::query("DELETE FROM transcode_cache WHERE cache_key = ?")
            .bind(key)
            .execute(&state.pool)
            .await
            .map_err(AppError::internal)?;
        total -= size;
    }
    Ok(())
}

pub async fn scrobble(
    state: &AppState,
    user_id: Uuid,
    request: ScrobbleRequest,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?)")
        .bind(request.track_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if !exists {
        return Err(AppError::NotFound("曲目不存在".to_owned()));
    }
    let client = request.client.unwrap_or_else(|| "meloark-web".to_owned());
    let now = Utc::now();
    sqlx::query("INSERT INTO play_history (id, user_id, track_id, media_file_id, client, played_at, completed) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4()).bind(user_id).bind(request.track_id).bind(request.media_file_id).bind(&client).bind(now).bind(request.completed).execute(&state.pool).await.map_err(AppError::internal)?;
    sqlx::query("INSERT INTO now_playing (user_id, track_id, media_file_id, client, position_sec, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(user_id) DO UPDATE SET track_id=excluded.track_id, media_file_id=excluded.media_file_id, client=excluded.client, position_sec=excluded.position_sec, updated_at=excluded.updated_at")
        .bind(user_id).bind(request.track_id).bind(request.media_file_id).bind(client).bind(request.position_sec.unwrap_or(0)).bind(now).execute(&state.pool).await.map_err(AppError::internal)?;
    Ok(())
}

pub async fn history(state: &AppState, user_id: Uuid) -> Result<Vec<PlaybackHistory>, AppError> {
    sqlx::query_as::<_, PlaybackHistory>(r#"SELECT ph.id, ph.track_id, t.title, COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id),'未知艺术家') AS artist, ph.client, ph.played_at, ph.completed FROM play_history ph JOIN tracks t ON t.id=ph.track_id WHERE ph.user_id=? ORDER BY ph.played_at DESC LIMIT 100"#).bind(user_id).fetch_all(&state.pool).await.map_err(AppError::internal)
}

pub async fn set_favorite(
    state: &AppState,
    user_id: Uuid,
    track_id: Uuid,
    favorite: bool,
) -> Result<(), AppError> {
    if favorite {
        sqlx::query("INSERT INTO favorites (user_id, track_id, created_at) VALUES (?, ?, ?) ON CONFLICT DO NOTHING").bind(user_id).bind(track_id).bind(Utc::now()).execute(&state.pool).await.map_err(AppError::internal)?;
    } else {
        sqlx::query("DELETE FROM favorites WHERE user_id=? AND track_id=?")
            .bind(user_id)
            .bind(track_id)
            .execute(&state.pool)
            .await
            .map_err(AppError::internal)?;
    }
    Ok(())
}
pub async fn favorite_ids(state: &AppState, user_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    sqlx::query_scalar("SELECT track_id FROM favorites WHERE user_id=? ORDER BY created_at DESC")
        .bind(user_id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)
}

pub async fn create_playlist(
    state: &AppState,
    user_id: Uuid,
    request: CreatePlaylistRequest,
) -> Result<Playlist, AppError> {
    if request.name.trim().is_empty() {
        return Err(AppError::BadRequest("Playlist 名称不能为空".to_owned()));
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("INSERT INTO playlists (id,user_id,name,comment,created_at,updated_at) VALUES (?,?,?,?,?,?)").bind(id).bind(user_id).bind(request.name.trim()).bind(request.comment).bind(now).bind(now).execute(&mut*tx).await.map_err(AppError::internal)?;
    replace_playlist_tracks(&mut tx, id, &request.track_ids).await?;
    tx.commit().await.map_err(AppError::internal)?;
    get_playlist(state, user_id, id).await
}
pub async fn update_playlist(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
    request: UpdatePlaylistRequest,
) -> Result<Playlist, AppError> {
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    let changed=sqlx::query("UPDATE playlists SET name=COALESCE(?,name), comment=COALESCE(?,comment), updated_at=? WHERE id=? AND user_id=?").bind(request.name).bind(request.comment).bind(Utc::now()).bind(id).bind(user_id).execute(&mut*tx).await.map_err(AppError::internal)?.rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound("Playlist 不存在".to_owned()));
    }
    if let Some(ids) = request.track_ids {
        replace_playlist_tracks(&mut tx, id, &ids).await?;
    }
    tx.commit().await.map_err(AppError::internal)?;
    get_playlist(state, user_id, id).await
}
async fn replace_playlist_tracks(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: Uuid,
    tracks: &[Uuid],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id=?")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(AppError::internal)?;
    for (position, track_id) in tracks.iter().enumerate() {
        sqlx::query("INSERT INTO playlist_tracks (playlist_id,track_id,position) VALUES (?,?,?)")
            .bind(id)
            .bind(track_id)
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .execute(&mut **tx)
            .await
            .map_err(AppError::internal)?;
    }
    Ok(())
}
pub async fn list_playlists(state: &AppState, user_id: Uuid) -> Result<Vec<Playlist>, AppError> {
    sqlx::query_as::<_,Playlist>(r#"SELECT p.id,p.name,p.comment,COUNT(pt.track_id) AS song_count,COALESCE(SUM(t.duration_ms)/1000,0) AS duration_sec,p.created_at,p.updated_at FROM playlists p LEFT JOIN playlist_tracks pt ON pt.playlist_id=p.id LEFT JOIN tracks t ON t.id=pt.track_id WHERE p.user_id=? GROUP BY p.id ORDER BY p.updated_at DESC"#).bind(user_id).fetch_all(&state.pool).await.map_err(AppError::internal)
}
pub async fn get_playlist(state: &AppState, user_id: Uuid, id: Uuid) -> Result<Playlist, AppError> {
    sqlx::query_as::<_,Playlist>(r#"SELECT p.id,p.name,p.comment,COUNT(pt.track_id) AS song_count,COALESCE(SUM(t.duration_ms)/1000,0) AS duration_sec,p.created_at,p.updated_at FROM playlists p LEFT JOIN playlist_tracks pt ON pt.playlist_id=p.id LEFT JOIN tracks t ON t.id=pt.track_id WHERE p.id=? AND p.user_id=? GROUP BY p.id"#).bind(id).bind(user_id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(||AppError::NotFound("Playlist 不存在".to_owned()))
}
pub async fn playlist_tracks(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let _ = get_playlist(state, user_id, id).await?;
    sqlx::query_scalar("SELECT track_id FROM playlist_tracks WHERE playlist_id=? ORDER BY position")
        .bind(id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)
}
pub async fn delete_playlist(state: &AppState, user_id: Uuid, id: Uuid) -> Result<(), AppError> {
    let changed = sqlx::query("DELETE FROM playlists WHERE id=? AND user_id=?")
        .bind(id)
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?
        .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound("Playlist 不存在".to_owned()));
    }
    Ok(())
}

pub async fn best_media_id(state: &AppState, track_id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT id FROM media_files WHERE track_id=? ORDER BY COALESCE(quality_score,0) DESC,file_size DESC LIMIT 1").bind(track_id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(||AppError::NotFound("曲目没有可播放文件".to_owned()))
}
pub async fn load_media(state: &AppState, id: Uuid) -> Result<StreamTarget, AppError> {
    sqlx::query_as::<_,StreamTarget>("SELECT mf.id,mf.track_id,l.path AS library_path,mf.relative_path,mf.extension,mf.file_size,mf.mtime_ms FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.id=?").bind(id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(||AppError::NotFound("媒体文件不存在".to_owned()))
}
pub fn safe_path(target: &StreamTarget) -> Result<PathBuf, AppError> {
    let root = Path::new(&target.library_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    let path = root
        .join(&target.relative_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    if !path.starts_with(root) {
        return Err(AppError::BadRequest("媒体路径超出曲库范围".to_owned()));
    }
    Ok(path)
}
pub fn mime_for_extension(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "m4a" | "mp4" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "aiff" | "aif" => "audio/aiff",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_parser_handles_open_and_suffix_ranges() {
        assert_eq!(parse_range("bytes=2-5", 10).expect("range"), (2, 5));
        assert_eq!(parse_range("bytes=7-", 10).expect("range"), (7, 9));
        assert_eq!(parse_range("bytes=-3", 10).expect("range"), (7, 9));
    }
    #[test]
    fn mime_mapping_is_explicit() {
        assert_eq!(mime_for_extension("flac"), "audio/flac");
        assert_eq!(mime_for_extension("unknown"), "application/octet-stream");
    }
}
