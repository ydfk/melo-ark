use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{AppError, Problem},
    state::AppState,
    text_normalization::normalize_for_match,
};

use super::auth::require_user_id;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackResponse {
    pub id: Uuid,
    pub media_id: Uuid,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<i64>,
    pub duration_ms: Option<i64>,
    pub variant_count: i64,
    pub total_bytes: i64,
    pub codec: Option<String>,
    pub extension: String,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub quality_score: Option<i64>,
    pub has_lyrics: bool,
    pub has_artwork: bool,
    pub tag_health: String,
    pub path: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackListResponse {
    pub items: Vec<TrackResponse>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetailResponse {
    pub id: Uuid,
    pub title: String,
    pub artists: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub duration_ms: Option<i64>,
    pub version_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MediaFileResponse {
    pub id: Uuid,
    pub library_id: Uuid,
    pub library_name: String,
    pub path: String,
    pub extension: String,
    pub file_size: i64,
    pub device_id: String,
    pub inode: String,
    pub hardlink_count: i64,
    pub codec: Option<String>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub has_artwork: bool,
    pub metadata_writable: bool,
    pub library_writable: bool,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackOperationHistoryResponse {
    pub id: Uuid,
    pub operation_id: Uuid,
    pub kind: String,
    pub action: String,
    pub status: String,
    pub source_path: Option<String>,
    pub target_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tracks", get(list))
        .route("/api/tracks/{id}", get(get_one))
        .route("/api/tracks/{id}/files", get(files))
        .route("/api/tracks/{id}/operations", get(operations))
}

#[utoipa::path(get, path = "/api/tracks/{id}", tag = "tracks", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = TrackDetailResponse)))]
pub async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<TrackDetailResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let track = sqlx::query_as::<_, TrackDetailResponse>(
        r#"SELECT t.id, t.title,
          COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY ta.position), '未知艺术家') AS artists,
          COALESCE(al.title, '未分类') AS album, al.album_artist, t.track_no, t.disc_no,
          t.year, t.genre, t.duration_ms, t.version_label
          FROM tracks t LEFT JOIN albums al ON al.id = t.album_id WHERE t.id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("曲目不存在".to_owned()))?;
    Ok(Json(track))
}

#[utoipa::path(get, path = "/api/tracks/{id}/files", tag = "tracks", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = Vec<MediaFileResponse>)))]
pub async fn files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MediaFileResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    let files = sqlx::query_as::<_, MediaFileResponse>(
        r#"SELECT mf.id, mf.library_id, l.name AS library_name,
          l.path || '/' || mf.relative_path AS path, mf.extension, mf.file_size,
          mf.device_id, mf.inode, mf.hardlink_count, mf.codec, mf.duration_ms,
          mf.bitrate, mf.sample_rate, mf.bit_depth, mf.has_artwork, mf.metadata_writable,
          l.writable AS library_writable
          FROM media_files mf JOIN libraries l ON l.id = mf.library_id
          WHERE mf.track_id = ? ORDER BY mf.file_size DESC, mf.relative_path"#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(Json(files))
}

#[utoipa::path(get, path = "/api/tracks/{id}/operations", tag = "tracks", params(("id" = Uuid, Path)), security(("bearerAuth" = [])), responses((status = 200, body = Vec<TrackOperationHistoryResponse>)))]
pub async fn operations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TrackOperationHistoryResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?)")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if !exists {
        return Err(AppError::NotFound("曲目不存在".to_owned()));
    }
    let items = sqlx::query_as::<_, TrackOperationHistoryResponse>(
        r#"SELECT oi.id, oi.operation_id, o.kind, oi.action, oi.status,
          oi.source_path, oi.target_path, oi.error_message, o.created_at,
          o.confirmed_at, o.finished_at, oi.updated_at
          FROM operation_items oi
          JOIN operations o ON o.id = oi.operation_id
          JOIN media_files mf ON mf.id = oi.media_file_id
          WHERE mf.track_id = ?
          ORDER BY oi.updated_at DESC LIMIT 100"#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(Json(items))
}

#[utoipa::path(
    get,
    path = "/api/tracks",
    tag = "tracks",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "分页曲目列表", body = TrackListResponse),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TrackListQuery>,
) -> Result<Json<TrackListResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * per_page;
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let filter = query
        .filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if filter.is_some_and(|value| {
        !matches!(
            value,
            "missing_lyrics" | "missing_cover" | "missing_tags" | "duplicates"
        )
    }) {
        return Err(AppError::BadRequest("未知曲库筛选条件".to_owned()));
    }

    let (total, items) = if let Some(search) = search {
        let fts_query = fts_query(search);
        let total = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM tracks t
               WHERE t.id IN (SELECT track_id FROM track_search WHERE track_search MATCH ?)
               AND (? IS NULL
                 OR (? = 'missing_lyrics' AND NOT EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id=t.id AND ly.active=1))
                 OR (? = 'missing_cover' AND NOT EXISTS (SELECT 1 FROM media_files mf WHERE mf.track_id=t.id AND mf.has_artwork=1))
                 OR (? = 'missing_tags' AND (t.title='' OR NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id=t.id) OR t.album_id IS NULL))
                 OR (? = 'duplicates' AND EXISTS (SELECT 1 FROM duplicate_group_members dgm JOIN media_files mf ON mf.id=dgm.media_file_id WHERE mf.track_id=t.id)))"#,
        )
        .bind(&fts_query)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let items = fetch_tracks(&state, Some(&fts_query), filter, per_page, offset).await?;
        (total, items)
    } else {
        let total = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM tracks t WHERE ? IS NULL
               OR (? = 'missing_lyrics' AND NOT EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id=t.id AND ly.active=1))
               OR (? = 'missing_cover' AND NOT EXISTS (SELECT 1 FROM media_files mf WHERE mf.track_id=t.id AND mf.has_artwork=1))
               OR (? = 'missing_tags' AND (t.title='' OR NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id=t.id) OR t.album_id IS NULL))
               OR (? = 'duplicates' AND EXISTS (SELECT 1 FROM duplicate_group_members dgm JOIN media_files mf ON mf.id=dgm.media_file_id WHERE mf.track_id=t.id))"#,
        )
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let items = fetch_tracks(&state, None, filter, per_page, offset).await?;
        (total, items)
    };

    Ok(Json(TrackListResponse {
        items,
        page,
        per_page,
        total,
    }))
}

async fn fetch_tracks(
    state: &AppState,
    search: Option<&str>,
    filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackResponse>, AppError> {
    let rows = if let Some(search) = search {
        sqlx::query_as::<_, TrackResponse>(
            r#"
            SELECT t.id, ms.media_id, t.title,
              COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id ORDER BY ta.position), '未知艺术家') AS artist,
              COALESCE(al.title, '未分类') AS album, t.year, t.duration_ms,
              ms.variant_count,
              ms.total_bytes,
              pm.codec, pm.extension, pm.sample_rate, pm.bit_depth, pm.quality_score,
              EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id=t.id AND ly.active=1) AS has_lyrics,
              pm.has_artwork,
              CASE WHEN t.title='' OR NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id=t.id) OR t.album_id IS NULL THEN 'missing' ELSE 'complete' END AS tag_health,
              l.path || '/' || pm.relative_path AS path
            FROM tracks t
            LEFT JOIN albums al ON al.id = t.album_id
            JOIN (
              SELECT track_id, (SELECT id FROM media_files preferred WHERE preferred.track_id = media_files.track_id ORDER BY COALESCE(quality_score, 0) DESC, file_size DESC LIMIT 1) AS media_id,
                     COUNT(*) AS variant_count, SUM(file_size) AS total_bytes
              FROM media_files GROUP BY track_id
            ) ms ON ms.track_id = t.id
            JOIN media_files pm ON pm.id=ms.media_id
            JOIN libraries l ON l.id=pm.library_id
            WHERE t.id IN (SELECT track_id FROM track_search WHERE track_search MATCH ?)
              AND (? IS NULL
                OR (? = 'missing_lyrics' AND NOT EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id=t.id AND ly.active=1))
                OR (? = 'missing_cover' AND NOT EXISTS (SELECT 1 FROM media_files mf WHERE mf.track_id=t.id AND mf.has_artwork=1))
                OR (? = 'missing_tags' AND (t.title='' OR NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id=t.id) OR t.album_id IS NULL))
                OR (? = 'duplicates' AND EXISTS (SELECT 1 FROM duplicate_group_members dgm JOIN media_files mf ON mf.id=dgm.media_file_id WHERE mf.track_id=t.id)))
            ORDER BY t.updated_at DESC, t.title
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(search)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, TrackResponse>(
            r#"
            SELECT t.id, ms.media_id, t.title,
              COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id ORDER BY ta.position), '未知艺术家') AS artist,
              COALESCE(al.title, '未分类') AS album, t.year, t.duration_ms,
              ms.variant_count,
              ms.total_bytes,
              pm.codec, pm.extension, pm.sample_rate, pm.bit_depth, pm.quality_score,
              EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id=t.id AND ly.active=1) AS has_lyrics,
              pm.has_artwork,
              CASE WHEN t.title='' OR NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id=t.id) OR t.album_id IS NULL THEN 'missing' ELSE 'complete' END AS tag_health,
              l.path || '/' || pm.relative_path AS path
            FROM tracks t
            LEFT JOIN albums al ON al.id = t.album_id
            JOIN (
              SELECT track_id, (SELECT id FROM media_files preferred WHERE preferred.track_id = media_files.track_id ORDER BY COALESCE(quality_score, 0) DESC, file_size DESC LIMIT 1) AS media_id,
                     COUNT(*) AS variant_count, SUM(file_size) AS total_bytes
              FROM media_files GROUP BY track_id
            ) ms ON ms.track_id = t.id
            JOIN media_files pm ON pm.id=ms.media_id
            JOIN libraries l ON l.id=pm.library_id
            WHERE ? IS NULL
              OR (? = 'missing_lyrics' AND NOT EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id=t.id AND ly.active=1))
              OR (? = 'missing_cover' AND NOT EXISTS (SELECT 1 FROM media_files mf WHERE mf.track_id=t.id AND mf.has_artwork=1))
              OR (? = 'missing_tags' AND (t.title='' OR NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id=t.id) OR t.album_id IS NULL))
              OR (? = 'duplicates' AND EXISTS (SELECT 1 FROM duplicate_group_members dgm JOIN media_files mf ON mf.id=dgm.media_file_id WHERE mf.track_id=t.id))
            ORDER BY t.updated_at DESC, t.title
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
    };
    rows.map_err(AppError::internal)
}

fn fts_query(search: &str) -> String {
    normalize_for_match(search)
        .split_whitespace()
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}
