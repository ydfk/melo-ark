use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    auth::create_media_token,
    error::{AppError, Problem},
    playback,
    state::AppState,
    text_normalization::normalize_for_match,
};

use super::playback::PlayTokenResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTrackQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogAlbumQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTrack {
    pub id: Uuid,
    pub media_id: Uuid,
    pub title: String,
    pub artist: String,
    pub album_id: Option<Uuid>,
    pub album: String,
    pub year: Option<i64>,
    pub duration_ms: Option<i64>,
    pub has_lyrics: bool,
    pub has_artwork: bool,
    pub artwork_media_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTrackPage {
    pub items: Vec<CatalogTrack>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAlbum {
    pub id: Uuid,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub track_count: i64,
    pub duration_ms: i64,
    pub cover_media_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogLyrics {
    pub track_id: Uuid,
    pub format: String,
    pub language: Option<String>,
    pub content: String,
    pub translated_content: Option<String>,
    pub synced: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/catalog/tracks", get(tracks))
        .route("/api/catalog/albums", get(albums))
        .route("/api/catalog/artwork/{id}", get(artwork))
        .route("/api/catalog/tracks/{id}/lyrics", get(lyrics))
        .route("/api/catalog/media/{id}/play-token", post(play_token))
}

#[utoipa::path(
    get,
    path = "/api/catalog/tracks",
    tag = "catalog",
    params(
        ("page" = Option<i64>, Query, description = "页码"),
        ("perPage" = Option<i64>, Query, description = "每页数量"),
        ("search" = Option<String>, Query, description = "标题、艺术家或专辑")
    ),
    responses(
        (status = 200, description = "公开曲目目录", body = CatalogTrackPage),
        (status = 500, body = Problem)
    )
)]
pub async fn tracks(
    State(state): State<AppState>,
    Query(query): Query<CatalogTrackQuery>,
) -> Result<Json<CatalogTrackPage>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * per_page;
    let search = catalog_search(query.search.as_deref());

    let total = count_tracks(&state, search.as_deref()).await?;
    let items = fetch_tracks(&state, search.as_deref(), per_page, offset).await?;
    Ok(Json(CatalogTrackPage {
        items,
        page,
        per_page,
        total,
    }))
}

#[utoipa::path(
    get,
    path = "/api/catalog/albums",
    tag = "catalog",
    params(("limit" = Option<i64>, Query, description = "返回数量")),
    responses(
        (status = 200, description = "公开专辑目录", body = Vec<CatalogAlbum>),
        (status = 500, body = Problem)
    )
)]
pub async fn albums(
    State(state): State<AppState>,
    Query(query): Query<CatalogAlbumQuery>,
) -> Result<Json<Vec<CatalogAlbum>>, AppError> {
    let limit = query.limit.unwrap_or(24).clamp(1, 100);
    let items = sqlx::query_as::<_, CatalogAlbum>(
        r#"SELECT al.id, al.title,
          COALESCE(NULLIF(al.album_artist, ''),
            (SELECT REPLACE(GROUP_CONCAT(DISTINCT a.name), ',', '; ')
             FROM tracks at
             JOIN track_artists ta ON ta.track_id = at.id
             JOIN artists a ON a.id = ta.artist_id
             WHERE at.album_id = al.id),
            '未知艺术家') AS artist,
          al.year,
          (SELECT COUNT(*) FROM tracks ct WHERE ct.album_id = al.id
            AND EXISTS (SELECT 1 FROM media_files cmf JOIN libraries cl ON cl.id = cmf.library_id
              WHERE cmf.track_id = ct.id AND cmf.available = 1 AND cl.role = 'managed')) AS track_count,
          COALESCE((SELECT SUM(COALESCE(dt.duration_ms, 0)) FROM tracks dt
            WHERE dt.album_id = al.id
              AND EXISTS (SELECT 1 FROM media_files dmf JOIN libraries dl ON dl.id = dmf.library_id
                WHERE dmf.track_id = dt.id AND dmf.available = 1 AND dl.role = 'managed')), 0) AS duration_ms,
          (SELECT amf.id FROM media_files amf JOIN libraries aml ON aml.id = amf.library_id
             JOIN tracks atr ON atr.id = amf.track_id
             WHERE atr.album_id = al.id AND amf.available = 1 AND amf.has_artwork = 1
               AND aml.role = 'managed'
             ORDER BY COALESCE(amf.quality_score, 0) DESC, amf.file_size DESC LIMIT 1) AS cover_media_id
          FROM albums al
          WHERE EXISTS (
            SELECT 1 FROM tracks et
            JOIN media_files emf ON emf.track_id = et.id
            JOIN libraries el ON el.id = emf.library_id
            WHERE et.album_id = al.id AND emf.available = 1 AND el.role = 'managed'
          )
          ORDER BY (SELECT MAX(ut.updated_at) FROM tracks ut WHERE ut.album_id = al.id) DESC,
            al.title
          LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(Json(items))
}

#[utoipa::path(
    get,
    path = "/api/catalog/artwork/{id}",
    tag = "catalog",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, description = "公开封面图片"),
        (status = 404, description = "媒体不可用或没有封面", body = Problem)
    )
)]
pub async fn artwork(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<axum::response::Response, AppError> {
    playback::artwork(&state, id).await
}

#[utoipa::path(
    get,
    path = "/api/catalog/tracks/{id}/lyrics",
    tag = "catalog",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, description = "当前生效歌词", body = CatalogLyrics),
        (status = 404, description = "没有生效歌词", body = Problem)
    )
)]
pub async fn lyrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CatalogLyrics>, AppError> {
    let item = sqlx::query_as::<_, CatalogLyrics>(
        r#"SELECT ly.track_id, ly.format, ly.language, ly.content, ly.translated_content, ly.synced
          FROM lyrics ly
          WHERE ly.track_id = ? AND ly.active = 1
            AND EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id = mf.library_id
              WHERE mf.track_id = ly.track_id AND mf.available = 1 AND ml.role = 'managed')
          ORDER BY ly.quality_score DESC, ly.updated_at DESC LIMIT 1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("当前曲目没有可用歌词".to_owned()))?;
    Ok(Json(item))
}

#[utoipa::path(
    post,
    path = "/api/catalog/media/{id}/play-token",
    tag = "catalog",
    params(("id" = Uuid, Path)),
    responses(
        (status = 200, description = "十分钟有效的媒体凭据", body = PlayTokenResponse),
        (status = 404, description = "媒体文件不可播放", body = Problem)
    )
)]
pub async fn play_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PlayTokenResponse>, AppError> {
    playback::load_media(&state, id).await?;
    Ok(Json(PlayTokenResponse {
        token: create_media_token(id, &state.jwt)?,
        expires_in: 600,
    }))
}

async fn count_tracks(state: &AppState, search: Option<&str>) -> Result<i64, AppError> {
    let count = if let Some(search) = search {
        sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM tracks t
              WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id = mf.library_id
                WHERE mf.track_id = t.id AND mf.available = 1 AND ml.role = 'managed')
                AND t.id IN (SELECT track_id FROM track_search WHERE track_search MATCH ?)"#,
        )
        .bind(search)
        .fetch_one(&state.pool)
        .await
    } else {
        sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM tracks t
              WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id = mf.library_id
                WHERE mf.track_id = t.id AND mf.available = 1 AND ml.role = 'managed')"#,
        )
        .fetch_one(&state.pool)
        .await
    };
    count.map_err(AppError::internal)
}

async fn fetch_tracks(
    state: &AppState,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<CatalogTrack>, AppError> {
    sqlx::query_as::<_, CatalogTrack>(
        r#"SELECT t.id,
          (SELECT pm.id FROM media_files pm JOIN libraries pl ON pl.id = pm.library_id
            WHERE pm.track_id = t.id AND pm.available = 1 AND pl.role = 'managed'
            ORDER BY COALESCE(pm.quality_score, 0) DESC, pm.file_size DESC LIMIT 1) AS media_id,
          t.title,
          COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta
            JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY ta.position),
            '未知艺术家') AS artist,
          t.album_id, COALESCE(al.title, '未分类') AS album, t.year, t.duration_ms,
          EXISTS (SELECT 1 FROM lyrics ly WHERE ly.track_id = t.id AND ly.active = 1) AS has_lyrics,
          EXISTS (SELECT 1 FROM media_files cmf JOIN libraries cl ON cl.id = cmf.library_id
            WHERE cmf.track_id = t.id AND cmf.available = 1 AND cmf.has_artwork = 1
              AND cl.role = 'managed') AS has_artwork,
          (SELECT amf.id FROM media_files amf JOIN libraries alib ON alib.id = amf.library_id
            WHERE amf.track_id = t.id AND amf.available = 1 AND amf.has_artwork = 1
              AND alib.role = 'managed'
            ORDER BY COALESCE(amf.quality_score, 0) DESC, amf.file_size DESC LIMIT 1) AS artwork_media_id
          FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
          WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id = mf.library_id
            WHERE mf.track_id = t.id AND mf.available = 1 AND ml.role = 'managed')
            AND (? IS NULL OR t.id IN (
              SELECT track_id FROM track_search WHERE track_search MATCH ?
            ))
          ORDER BY t.updated_at DESC, t.title LIMIT ? OFFSET ?"#,
    )
    .bind(search)
    .bind(search)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)
}

fn catalog_search(search: Option<&str>) -> Option<String> {
    let terms = normalize_for_match(search?.trim())
        .split_whitespace()
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");
    (!terms.is_empty()).then(|| format!("{{title artist album normalized_text}} : ({terms})"))
}

#[cfg(test)]
mod tests {
    use super::catalog_search;

    #[test]
    fn catalog_search_escapes_fts_tokens() {
        assert_eq!(
            catalog_search(Some("  夜曲 live  ")).as_deref(),
            Some("{title artist album normalized_text} : (\"夜曲\"* AND \"live\"*)")
        );
        assert_eq!(catalog_search(Some("  ")), None);
    }
}
