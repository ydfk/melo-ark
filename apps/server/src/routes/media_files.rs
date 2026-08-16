use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite};
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
pub struct ManagedMediaFileQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedMediaFileResponse {
    pub media_id: Uuid,
    pub track_id: Uuid,
    pub organized_library_id: Uuid,
    pub organized_path: String,
    pub relative_path: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub extension: String,
    pub file_size: i64,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub quality_score: Option<i64>,
    pub has_lyrics: bool,
    pub has_artwork: bool,
    pub tag_health: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedMediaFilePage {
    pub items: Vec<ManagedMediaFileResponse>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/media-files", get(list))
}

#[utoipa::path(
    get,
    path = "/api/media-files",
    tag = "tracks",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "分页整理文件列表", body = ManagedMediaFilePage),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManagedMediaFileQuery>,
) -> Result<Json<ManagedMediaFilePage>, AppError> {
    require_user_id(&headers, &state)?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * per_page;
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(fts_query);
    let filter = query
        .filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    validate_filter(filter)?;

    let mut count = QueryBuilder::<Sqlite>::new(
        r#"SELECT COUNT(*)
           FROM media_files mf
           JOIN libraries l ON l.id = mf.library_id
           JOIN tracks t ON t.id = mf.track_id
           WHERE mf.available = 1 AND l.role = 'managed'"#,
    );
    push_conditions(&mut count, search.as_deref(), filter);
    let total = count
        .build_query_scalar::<i64>()
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;

    let mut items = QueryBuilder::<Sqlite>::new(
        r#"SELECT mf.id AS media_id, mf.track_id,
             l.id AS organized_library_id, l.path AS organized_path,
             mf.relative_path, l.path || '/' || mf.relative_path AS path,
             t.title,
             COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta
                       JOIN artists a ON a.id = ta.artist_id
                       WHERE ta.track_id = t.id ORDER BY ta.position), '未知艺术家') AS artist,
             COALESCE(al.title, '未分类') AS album, t.year,
             COALESCE(mf.duration_ms, t.duration_ms) AS duration_ms,
             mf.codec, mf.extension, mf.file_size, mf.sample_rate, mf.bit_depth,
             mf.quality_score,
             EXISTS(SELECT 1 FROM lyrics ly WHERE ly.track_id = t.id AND ly.active = 1) AS has_lyrics,
             mf.has_artwork,
             CASE WHEN t.title = ''
                    OR NOT EXISTS(SELECT 1 FROM track_artists ta WHERE ta.track_id = t.id)
                    OR t.album_id IS NULL
                  THEN 'missing' ELSE 'complete' END AS tag_health
           FROM media_files mf
           JOIN libraries l ON l.id = mf.library_id
           JOIN tracks t ON t.id = mf.track_id
           LEFT JOIN albums al ON al.id = t.album_id
           WHERE mf.available = 1 AND l.role = 'managed'"#,
    );
    push_conditions(&mut items, search.as_deref(), filter);
    items.push(" ORDER BY mf.updated_at DESC, l.path, mf.relative_path LIMIT ");
    items.push_bind(per_page);
    items.push(" OFFSET ");
    items.push_bind(offset);
    let rows = items
        .build_query_as::<ManagedMediaFileResponse>()
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;

    Ok(Json(ManagedMediaFilePage {
        items: rows,
        page,
        per_page,
        total,
    }))
}

fn validate_filter(filter: Option<&str>) -> Result<(), AppError> {
    if filter.is_some_and(|value| {
        !matches!(
            value,
            "missing_lyrics" | "missing_cover" | "missing_tags" | "duplicates"
        )
    }) {
        return Err(AppError::BadRequest("未知整理文件筛选条件".to_owned()));
    }
    Ok(())
}

fn push_conditions(query: &mut QueryBuilder<Sqlite>, search: Option<&str>, filter: Option<&str>) {
    if let Some(search) = search {
        query.push(" AND mf.id IN (SELECT media_id FROM track_search WHERE track_search MATCH ");
        query.push_bind(search.to_owned());
        query.push(")");
    }
    match filter {
        Some("missing_lyrics") => query.push(
            " AND NOT EXISTS(SELECT 1 FROM lyrics ly WHERE ly.track_id = t.id AND ly.active = 1)",
        ),
        Some("missing_cover") => query.push(" AND mf.has_artwork = 0"),
        Some("missing_tags") => query.push(
            " AND (t.title = '' OR NOT EXISTS(SELECT 1 FROM track_artists ta WHERE ta.track_id = t.id) OR t.album_id IS NULL)",
        ),
        Some("duplicates") => query.push(
            " AND EXISTS(SELECT 1 FROM duplicate_group_members dgm WHERE dgm.media_file_id = mf.id)",
        ),
        _ => query,
    };
}

fn fts_query(search: &str) -> String {
    normalize_for_match(search)
        .split_whitespace()
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}
