use serde_json::{Map, Value, json};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{error::AppError, playback, state::AppState, text_normalization};

#[derive(Debug, Clone, FromRow)]
struct SongRow {
    id: Uuid,
    title: String,
    artist: String,
    artist_id: Option<Uuid>,
    album: String,
    album_id: Option<Uuid>,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    year: Option<i64>,
    genre: Option<String>,
    duration_ms: Option<i64>,
    created_at: String,
    media_id: Uuid,
    relative_path: String,
    extension: String,
    file_size: i64,
    bitrate: Option<i64>,
    starred: bool,
}

pub async fn music_folders(state: &AppState) -> Result<Map<String, Value>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,name FROM libraries WHERE scan_enabled=1 AND role='managed' ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    map(
        "musicFolders",
        json!({"musicFolder":rows.into_iter().map(|(id,name)|json!({"id":id,"name":name})).collect::<Vec<_>>() }),
    )
}

pub async fn artists(state: &AppState, user: Uuid) -> Result<Map<String, Value>, AppError> {
    let rows=sqlx::query_as::<_,(Uuid,String,i64)>("SELECT a.id,a.name,COUNT(DISTINCT t.album_id) FROM artists a JOIN track_artists ta ON ta.artist_id=a.id JOIN tracks t ON t.id=ta.track_id WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND l.role='managed') GROUP BY a.id ORDER BY a.name COLLATE NOCASE").fetch_all(&state.pool).await.map_err(AppError::internal)?;
    let mut indexes: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for (id, name, album_count) in rows {
        let letter = text_normalization::artist_initial(&name);
        indexes
            .entry(letter)
            .or_default()
            .push(json!({"id":format!("ar:{id}"),"name":name,"albumCount":album_count}));
    }
    let _ = user;
    map(
        "artists",
        json!({"ignoredArticles":"The El La Los Las Le Les","index":indexes.into_iter().map(|(name,artist)|json!({"name":name,"artist":artist})).collect::<Vec<_>>() }),
    )
}

pub async fn artist(
    state: &AppState,
    id: Uuid,
    user: Uuid,
) -> Result<Map<String, Value>, AppError> {
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM artists WHERE id=?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let name = name.ok_or_else(|| AppError::NotFound("艺术家不存在".to_owned()))?;
    let albums=sqlx::query_as::<_,(Uuid,String,Option<i64>,i64,Option<i64>)>(r#"SELECT al.id,al.title,al.year,COUNT(t.id),SUM(t.duration_ms)/1000 FROM albums al JOIN tracks t ON t.album_id=al.id JOIN track_artists ta ON ta.track_id=t.id WHERE ta.artist_id=? AND EXISTS(SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND l.role='managed') GROUP BY al.id ORDER BY al.year DESC,al.title"#).bind(id).fetch_all(&state.pool).await.map_err(AppError::internal)?;
    let album=albums.into_iter().map(|(album_id,title,year,count,duration)|json!({"id":format!("al:{album_id}"),"name":title,"title":title,"artist":name,"artistId":format!("ar:{id}"),"songCount":count,"duration":duration.unwrap_or(0),"year":year,"coverArt":format!("al:{album_id}")})).collect::<Vec<_>>();
    let _ = user;
    map(
        "artist",
        json!({"id":format!("ar:{id}"),"name":name,"albumCount":album.len(),"album":album}),
    )
}

pub async fn album(state: &AppState, id: Uuid, user: Uuid) -> Result<Map<String, Value>, AppError> {
    let row = sqlx::query_as::<_, (String, Option<String>, Option<i64>)>(
        "SELECT title,album_artist,year FROM albums WHERE id=? AND EXISTS (SELECT 1 FROM tracks t JOIN media_files mf ON mf.track_id=t.id JOIN libraries l ON l.id=mf.library_id WHERE t.album_id=albums.id AND mf.available=1 AND l.role='managed')",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("专辑不存在".to_owned()))?;
    let songs = album_songs(state, user, id).await?;
    let duration: i64 = songs
        .iter()
        .filter_map(|item| item["duration"].as_i64())
        .sum();
    map(
        "album",
        json!({"id":format!("al:{id}"),"name":row.0,"title":row.0,"artist":row.1.clone().unwrap_or_else(||"未知艺术家".to_owned()),"year":row.2,"songCount":songs.len(),"duration":duration,"coverArt":format!("al:{id}"),"song":songs}),
    )
}

pub async fn song(state: &AppState, id: Uuid, user: Uuid) -> Result<Map<String, Value>, AppError> {
    let row = load_song(state, user, id).await?;
    map("song", song_value(&row))
}

pub async fn album_list(
    state: &AppState,
    user: Uuid,
    kind: &str,
    size: i64,
    offset: i64,
) -> Result<Map<String, Value>, AppError> {
    let query = match kind {
        "random" => sqlx::query_scalar::<_, Uuid>(
            r#"SELECT al.id FROM albums al JOIN tracks t ON t.album_id=al.id LEFT JOIN play_history ph ON ph.track_id=t.id AND ph.user_id=? LEFT JOIN favorites f ON f.track_id=t.id AND f.user_id=? WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') GROUP BY al.id ORDER BY RANDOM() LIMIT ? OFFSET ?"#,
        ),
        "alphabeticalByName" => sqlx::query_scalar::<_, Uuid>(
            r#"SELECT al.id FROM albums al JOIN tracks t ON t.album_id=al.id LEFT JOIN play_history ph ON ph.track_id=t.id AND ph.user_id=? LEFT JOIN favorites f ON f.track_id=t.id AND f.user_id=? WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') GROUP BY al.id ORDER BY al.title COLLATE NOCASE LIMIT ? OFFSET ?"#,
        ),
        "frequent" => sqlx::query_scalar::<_, Uuid>(
            r#"SELECT al.id FROM albums al JOIN tracks t ON t.album_id=al.id LEFT JOIN play_history ph ON ph.track_id=t.id AND ph.user_id=? LEFT JOIN favorites f ON f.track_id=t.id AND f.user_id=? WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') GROUP BY al.id ORDER BY COUNT(ph.id) DESC LIMIT ? OFFSET ?"#,
        ),
        "recent" => sqlx::query_scalar::<_, Uuid>(
            r#"SELECT al.id FROM albums al JOIN tracks t ON t.album_id=al.id LEFT JOIN play_history ph ON ph.track_id=t.id AND ph.user_id=? LEFT JOIN favorites f ON f.track_id=t.id AND f.user_id=? WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') GROUP BY al.id ORDER BY MAX(ph.played_at) DESC LIMIT ? OFFSET ?"#,
        ),
        "starred" => sqlx::query_scalar::<_, Uuid>(
            r#"SELECT al.id FROM albums al JOIN tracks t ON t.album_id=al.id LEFT JOIN play_history ph ON ph.track_id=t.id AND ph.user_id=? LEFT JOIN favorites f ON f.track_id=t.id AND f.user_id=? WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') GROUP BY al.id ORDER BY MAX(f.created_at) DESC LIMIT ? OFFSET ?"#,
        ),
        _ => sqlx::query_scalar::<_, Uuid>(
            r#"SELECT al.id FROM albums al JOIN tracks t ON t.album_id=al.id LEFT JOIN play_history ph ON ph.track_id=t.id AND ph.user_id=? LEFT JOIN favorites f ON f.track_id=t.id AND f.user_id=? WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed') GROUP BY al.id ORDER BY MAX(t.created_at) DESC LIMIT ? OFFSET ?"#,
        ),
    };
    let ids = query
        .bind(user)
        .bind(user)
        .bind(size.clamp(1, 500))
        .bind(offset.max(0))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let mut albums = Vec::new();
    for id in ids {
        let payload = album(state, id, user).await?;
        if let Some(value) = payload.get("album") {
            let mut summary = value.clone();
            if let Some(object) = summary.as_object_mut() {
                object.remove("song");
            }
            albums.push(summary);
        }
    }
    map("albumList2", json!({"album":albums}))
}

pub async fn random_songs(
    state: &AppState,
    user: Uuid,
    size: i64,
) -> Result<Map<String, Value>, AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM tracks WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=tracks.id AND mf.available=1 AND l.role='managed') ORDER BY RANDOM() LIMIT ?")
        .bind(size.clamp(1, 500))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let mut songs = Vec::new();
    for id in ids {
        songs.push(song_value(&load_song(state, user, id).await?));
    }
    map("randomSongs", json!({"song":songs}))
}

pub async fn starred(state: &AppState, user: Uuid) -> Result<Map<String, Value>, AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT f.track_id FROM favorites f WHERE f.user_id=? AND EXISTS(SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=f.track_id AND mf.available=1 AND l.role='managed') ORDER BY f.created_at DESC",
    )
    .bind(user)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut songs = Vec::new();
    for id in ids {
        songs.push(song_value(&load_song(state, user, id).await?));
    }
    map("starred2", json!({"song":songs,"album":[],"artist":[]}))
}

pub async fn search(
    state: &AppState,
    user: Uuid,
    query: &str,
    artist_count: i64,
    album_count: i64,
    song_count: i64,
) -> Result<Map<String, Value>, AppError> {
    let trimmed = query.trim();
    let (artist_rows, album_rows, ids) = if trimmed.is_empty() {
        let artist_rows = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id,name FROM artists a WHERE EXISTS (SELECT 1 FROM track_artists ta JOIN media_files mf ON mf.track_id=ta.track_id JOIN libraries l ON l.id=mf.library_id WHERE ta.artist_id=a.id AND mf.available=1 AND l.role='managed') ORDER BY name LIMIT ?",
        )
        .bind(artist_count.clamp(0, 500))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let album_rows = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id,title FROM albums a WHERE EXISTS (SELECT 1 FROM tracks t JOIN media_files mf ON mf.track_id=t.id JOIN libraries l ON l.id=mf.library_id WHERE t.album_id=a.id AND mf.available=1 AND l.role='managed') ORDER BY title LIMIT ?",
        )
        .bind(album_count.clamp(0, 500))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM tracks WHERE EXISTS(SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=tracks.id AND mf.available=1 AND l.role='managed') LIMIT ?")
            .bind(song_count.clamp(0, 500))
            .fetch_all(&state.pool)
            .await
            .map_err(AppError::internal)?;
        (artist_rows, album_rows, ids)
    } else {
        let fts_query = build_fts_query(trimmed);
        let artist_rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT DISTINCT a.id,a.name FROM artists a
               JOIN track_artists ta ON ta.artist_id=a.id
               WHERE ta.track_id IN (SELECT track_id FROM track_search WHERE track_search MATCH ?)
                 AND EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=ta.track_id AND mf.available=1 AND ml.role='managed')
               ORDER BY a.name LIMIT ?"#,
        )
        .bind(&fts_query)
        .bind(artist_count.clamp(0, 500))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let album_rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT DISTINCT a.id,a.title FROM albums a JOIN tracks t ON t.album_id=a.id
               WHERE t.id IN (SELECT track_id FROM track_search WHERE track_search MATCH ?)
                 AND EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=t.id AND mf.available=1 AND ml.role='managed')
               ORDER BY a.title LIMIT ?"#,
        )
        .bind(&fts_query)
        .bind(album_count.clamp(0, 500))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT track_id FROM track_search WHERE track_search MATCH ? AND EXISTS(SELECT 1 FROM media_files mf JOIN libraries ml ON ml.id=mf.library_id WHERE mf.track_id=track_search.track_id AND mf.available=1 AND ml.role='managed') LIMIT ?",
        )
        .bind(&fts_query)
        .bind(song_count.clamp(0, 500))
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
        (artist_rows, album_rows, ids)
    };
    let mut songs = Vec::new();
    for id in ids {
        songs.push(song_value(&load_song(state, user, id).await?));
    }
    map(
        "searchResult3",
        json!({"artist":artist_rows.into_iter().map(|(id,name)|json!({"id":format!("ar:{id}"),"name":name})).collect::<Vec<_>>(),"album":album_rows.into_iter().map(|(id,name)|json!({"id":format!("al:{id}"),"name":name,"title":name,"coverArt":format!("al:{id}")})).collect::<Vec<_>>(),"song":songs}),
    )
}

fn build_fts_query(query: &str) -> String {
    text_normalization::normalize_for_match(query)
        .split_whitespace()
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub async fn music_directory(
    state: &AppState,
    user: Uuid,
    id: &str,
) -> Result<Map<String, Value>, AppError> {
    if let Some(id) = parse_prefixed(id, "ar:") {
        let payload = artist(state, id, user).await?;
        let artist = &payload["artist"];
        map(
            "directory",
            json!({"id":format!("ar:{id}"),"name":artist["name"],"child":artist["album"]}),
        )
    } else if let Some(id) = parse_prefixed(id, "al:") {
        let payload = album(state, id, user).await?;
        let album = &payload["album"];
        map(
            "directory",
            json!({"id":format!("al:{id}"),"name":album["name"],"child":album["song"]}),
        )
    } else {
        Err(AppError::NotFound("目录不存在".to_owned()))
    }
}

pub async fn indexes(state: &AppState, user: Uuid) -> Result<Map<String, Value>, AppError> {
    let payload = artists(state, user).await?;
    let artists = &payload["artists"];
    map(
        "indexes",
        json!({"lastModified":UtcMillis::now(),"ignoredArticles":artists["ignoredArticles"],"index":artists["index"]}),
    )
}

async fn album_songs(state: &AppState, user: Uuid, album_id: Uuid) -> Result<Vec<Value>, AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM tracks WHERE album_id=? AND EXISTS(SELECT 1 FROM media_files mf JOIN libraries l ON l.id=mf.library_id WHERE mf.track_id=tracks.id AND mf.available=1 AND l.role='managed') ORDER BY disc_no,track_no,title",
    )
    .bind(album_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut songs = Vec::new();
    for id in ids {
        songs.push(song_value(&load_song(state, user, id).await?));
    }
    Ok(songs)
}

async fn load_song(state: &AppState, user: Uuid, id: Uuid) -> Result<SongRow, AppError> {
    sqlx::query_as::<_,SongRow>(r#"SELECT t.id,t.title,COALESCE((SELECT GROUP_CONCAT(a.name,'; ') FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id),'未知艺术家') AS artist,(SELECT a.id FROM track_artists ta JOIN artists a ON a.id=ta.artist_id WHERE ta.track_id=t.id ORDER BY ta.position LIMIT 1) AS artist_id,COALESCE(al.title,'未分类') AS album,t.album_id,t.track_no,t.disc_no,t.year,t.genre,t.duration_ms,t.created_at,mf.id AS media_id,mf.relative_path,mf.extension,mf.file_size,mf.bitrate,EXISTS(SELECT 1 FROM favorites f WHERE f.user_id=? AND f.track_id=t.id) AS starred FROM tracks t LEFT JOIN albums al ON al.id=t.album_id JOIN media_files mf ON mf.id=(SELECT smf.id FROM media_files smf JOIN libraries sl ON sl.id=smf.library_id WHERE smf.track_id=t.id AND smf.available=1 AND sl.role='managed' ORDER BY COALESCE(smf.quality_score,0) DESC,smf.file_size DESC LIMIT 1) WHERE t.id=?"#).bind(user).bind(id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(||AppError::NotFound("曲目不存在".to_owned()))
}

fn song_value(row: &SongRow) -> Value {
    json!({"id":row.id,"parent":row.album_id.map(|id|format!("al:{id}")),"isDir":false,"title":row.title,"album":row.album,"albumId":row.album_id.map(|id|format!("al:{id}")),"artist":row.artist,"artistId":row.artist_id.map(|id|format!("ar:{id}")),"track":row.track_no,"discNumber":row.disc_no,"year":row.year,"genre":row.genre,"coverArt":format!("mf:{}",row.media_id),"size":row.file_size,"contentType":playback::mime_for_extension(&row.extension),"suffix":row.extension,"duration":row.duration_ms.unwrap_or(0)/1000,"bitRate":row.bitrate.map(|value|value/1000),"path":row.relative_path,"created":row.created_at,"starred":row.starred.then_some(row.created_at.clone()),"type":"music","isVideo":false,"mediaFileId":row.media_id})
}
pub fn parse_track_id(value: &str) -> Option<Uuid> {
    value.strip_prefix("tr:").unwrap_or(value).parse().ok()
}
pub fn parse_media_id(value: &str) -> Option<Uuid> {
    value.strip_prefix("mf:").unwrap_or(value).parse().ok()
}
pub fn parse_prefixed(value: &str, prefix: &str) -> Option<Uuid> {
    value.strip_prefix(prefix)?.parse().ok()
}
fn map(key: &str, value: Value) -> Result<Map<String, Value>, AppError> {
    let mut map = Map::new();
    map.insert(key.to_owned(), value);
    Ok(map)
}
struct UtcMillis;
impl UtcMillis {
    fn now() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}
