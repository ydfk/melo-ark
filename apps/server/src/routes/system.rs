use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{AppError, Problem},
    state::AppState,
};

use super::auth::require_user_id;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStatsResponse {
    pub library_count: i64,
    pub artist_count: i64,
    pub album_count: i64,
    pub track_count: i64,
    pub media_file_count: i64,
    pub available_managed_file_count: i64,
    pub pending_review_count: i64,
    pub total_bytes: i64,
    pub missing_tag_count: i64,
    pub missing_lyrics_count: i64,
    pub missing_cover_count: i64,
    pub possible_duplicate_count: i64,
    pub exact_duplicate_count: i64,
    pub running_job_count: i64,
    pub recent_scan_at: Option<DateTime<Utc>>,
    pub format_distribution: Vec<FormatDistribution>,
    pub recent_added: Vec<DashboardRecentTrack>,
    pub recent_played: Vec<DashboardRecentPlay>,
}

#[derive(Debug, FromRow)]
struct DashboardCounts {
    library_count: i64,
    artist_count: i64,
    album_count: i64,
    track_count: i64,
    media_file_count: i64,
    total_bytes: i64,
    missing_tag_count: i64,
    running_job_count: i64,
    missing_lyrics_count: i64,
    missing_cover_count: i64,
    possible_duplicate_count: i64,
    exact_duplicate_count: i64,
    available_managed_file_count: i64,
    pending_review_count: i64,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatDistribution {
    pub extension: String,
    pub count: i64,
    pub total_bytes: i64,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardRecentTrack {
    pub id: Uuid,
    pub media_id: Uuid,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub has_artwork: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardRecentPlay {
    pub track_id: Uuid,
    pub title: String,
    pub artist: String,
    pub client: String,
    pub played_at: DateTime<Utc>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/dashboard/stats", get(dashboard_stats))
}

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "system",
    responses((status = 200, description = "服务正常", body = HealthResponse))
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "meloark",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[utoipa::path(
    get,
    path = "/api/dashboard/stats",
    tag = "system",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "曲库健康统计", body = DashboardStatsResponse),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn dashboard_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DashboardStatsResponse>, AppError> {
    let user_id = require_user_id(&headers, &state)?;
    let counts = sqlx::query_as::<_, DashboardCounts>(
        r#"
        SELECT
          (SELECT COUNT(*) FROM libraries WHERE role='managed') AS library_count,
          (SELECT COUNT(*) FROM artists a WHERE EXISTS (SELECT 1 FROM track_artists ta JOIN media_files mf ON mf.track_id=ta.track_id JOIN libraries l ON l.id=mf.library_id WHERE ta.artist_id=a.id AND mf.available=1 AND l.role='managed')) AS artist_count,
          (SELECT COUNT(*) FROM albums a WHERE EXISTS (SELECT 1 FROM tracks t JOIN media_files mf ON mf.track_id=t.id JOIN libraries l ON l.id=mf.library_id WHERE t.album_id=a.id AND mf.available=1 AND l.role='managed')) AS album_count,
          (SELECT COUNT(*) FROM tracks t WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND l.role='managed')) AS track_count,
          (SELECT COUNT(*) FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE l.role='managed') AS media_file_count,
          COALESCE((SELECT SUM(mf.file_size) FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE l.role='managed'), 0) AS total_bytes,
          (SELECT COUNT(*) FROM tracks t WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND l.role='managed') AND (t.title = '' OR t.title IS NULL OR t.album_id IS NULL OR NOT EXISTS
            (SELECT 1 FROM track_artists ta WHERE ta.track_id = t.id))) AS missing_tag_count,
          (SELECT COUNT(*) FROM jobs WHERE internal = 0 AND status IN ('queued', 'running', 'paused', 'cancel_requested')) AS running_job_count,
          (SELECT COUNT(*) FROM tracks t WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') AND NOT EXISTS
            (SELECT 1 FROM lyrics l WHERE l.track_id = t.id AND l.active = 1)) AS missing_lyrics_count,
          (SELECT COUNT(*) FROM tracks t WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') AND NOT EXISTS
            (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id = t.id AND mf.has_artwork = 1 AND ml.role='managed')) AS missing_cover_count,
          (SELECT COUNT(*) FROM duplicate_groups WHERE kind = 'possible_duplicate') AS possible_duplicate_count,
          (SELECT COUNT(*) FROM duplicate_groups WHERE kind = 'binary_exact') AS exact_duplicate_count,
          (SELECT COUNT(*) FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE l.role='managed' AND mf.available=1) AS available_managed_file_count,
          (SELECT COUNT(*) FROM review_items WHERE status='pending') AS pending_review_count
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let recent_scan_at =
        sqlx::query_scalar::<_, Option<DateTime<Utc>>>("SELECT MAX(last_scan_at) FROM libraries")
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    let format_distribution = sqlx::query_as::<_, FormatDistribution>(
        r#"SELECT UPPER(extension) AS extension, COUNT(*) AS count,
           COALESCE(SUM(file_size), 0) AS total_bytes
           FROM media_files mf JOIN libraries l ON l.id=mf.library_id
           WHERE l.role='managed' GROUP BY UPPER(extension) ORDER BY total_bytes DESC, extension"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let (recent_added, recent_played) = tokio::try_join!(
        sqlx::query_as::<_, DashboardRecentTrack>(
            r#"SELECT t.id,
               (SELECT mf.id FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed' ORDER BY COALESCE(mf.quality_score,0) DESC, mf.file_size DESC LIMIT 1) AS media_id,
               t.title,
               COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id ORDER BY ta.position), '未知艺术家') AS artist,
               COALESCE(al.title, '未分类') AS album,
               EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.has_artwork=1 AND ml.role='managed') AS has_artwork,
               t.created_at
               FROM tracks t LEFT JOIN albums al ON al.id=t.album_id
               WHERE EXISTS (SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed')
               ORDER BY t.created_at DESC LIMIT 6"#,
        )
        .fetch_all(&state.pool),
        sqlx::query_as::<_, DashboardRecentPlay>(
            r#"SELECT ph.track_id, t.title,
               COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id ORDER BY ta.position), '未知艺术家') AS artist,
               ph.client, ph.played_at
               FROM play_history ph JOIN tracks t ON t.id=ph.track_id
               WHERE ph.user_id=? ORDER BY ph.played_at DESC LIMIT 6"#,
        )
        .bind(user_id)
        .fetch_all(&state.pool)
    )
    .map_err(AppError::internal)?;

    Ok(Json(DashboardStatsResponse {
        library_count: counts.library_count,
        artist_count: counts.artist_count,
        album_count: counts.album_count,
        track_count: counts.track_count,
        media_file_count: counts.media_file_count,
        available_managed_file_count: counts.available_managed_file_count,
        pending_review_count: counts.pending_review_count,
        total_bytes: counts.total_bytes,
        missing_tag_count: counts.missing_tag_count,
        running_job_count: counts.running_job_count,
        missing_lyrics_count: counts.missing_lyrics_count,
        missing_cover_count: counts.missing_cover_count,
        possible_duplicate_count: counts.possible_duplicate_count,
        exact_duplicate_count: counts.exact_duplicate_count,
        recent_scan_at,
        format_distribution,
        recent_added,
        recent_played,
    }))
}
