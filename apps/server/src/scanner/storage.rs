use chrono::Utc;
use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError, library::LibraryRecord, state::AppState, text_normalization::search_aliases,
};

use super::audio::{AudioInfo, FileStat, normalize_text};

pub(super) async fn media_is_unchanged(
    state: &AppState,
    library_id: Uuid,
    stat: &FileStat,
) -> Result<bool, AppError> {
    let existing = sqlx::query_as::<_, (i64, i64, String, String)>(
        "SELECT file_size, mtime_ms, device_id, inode FROM media_files WHERE library_id = ? AND relative_path = ?",
    )
    .bind(library_id)
    .bind(&stat.relative_path)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(existing.is_some_and(|value| {
        value.0 == stat.file_size
            && value.1 == stat.mtime_ms
            && value.2 == stat.device_id
            && value.3 == stat.inode
    }))
}

pub(super) async fn upsert_media(
    state: &AppState,
    job_id: Uuid,
    library: &LibraryRecord,
    stat: &FileStat,
    info: AudioInfo,
) -> Result<(), AppError> {
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    let artist_ids = upsert_artists(&mut transaction, &info.artists).await?;
    let album_id = upsert_album(&mut transaction, &info).await?;
    let track_id = upsert_track(&mut transaction, album_id, &info).await?;
    for (position, artist_id) in artist_ids.into_iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO track_artists (track_id, artist_id, position) VALUES (?, ?, ?)",
        )
        .bind(track_id)
        .bind(artist_id)
        .bind(position as i64)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    }
    let media_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM media_files WHERE library_id = ? AND relative_path = ?",
    )
    .bind(library.id)
    .bind(&stat.relative_path)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(AppError::internal)?
    .unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO media_files
          (id, track_id, library_id, relative_path, extension, file_size, mtime_ms,
           device_id, inode, hardlink_count, codec, container, duration_ms, bitrate,
           sample_rate, bit_depth, channels, metadata_readable, metadata_writable,
           has_artwork, scan_error, last_seen_scan_id, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(library_id, relative_path) DO UPDATE SET
          track_id = excluded.track_id, extension = excluded.extension,
          file_size = excluded.file_size, mtime_ms = excluded.mtime_ms,
          device_id = excluded.device_id, inode = excluded.inode,
          hardlink_count = excluded.hardlink_count, codec = excluded.codec,
          container = excluded.container, duration_ms = excluded.duration_ms,
          bitrate = excluded.bitrate, sample_rate = excluded.sample_rate,
          bit_depth = excluded.bit_depth, channels = excluded.channels,
          metadata_readable = excluded.metadata_readable,
          metadata_writable = excluded.metadata_writable,
          has_artwork = excluded.has_artwork, scan_error = excluded.scan_error,
          last_seen_scan_id = excluded.last_seen_scan_id, updated_at = excluded.updated_at
        "#,
    )
    .bind(media_id)
    .bind(track_id)
    .bind(library.id)
    .bind(&stat.relative_path)
    .bind(&stat.extension)
    .bind(stat.file_size)
    .bind(stat.mtime_ms)
    .bind(&stat.device_id)
    .bind(&stat.inode)
    .bind(stat.hardlink_count)
    .bind(&info.codec)
    .bind(&info.container)
    .bind(info.duration_ms)
    .bind(info.bitrate)
    .bind(info.sample_rate)
    .bind(info.bit_depth)
    .bind(info.channels)
    .bind(info.metadata_readable)
    .bind(info.metadata_writable)
    .bind(info.has_artwork)
    .bind(&info.scan_error)
    .bind(job_id)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if info.has_artwork {
        sqlx::query(
            r#"INSERT INTO artworks (id, media_file_id, kind, source, created_at)
              VALUES (?, ?, 'front', 'embedded', ?)
              ON CONFLICT(media_file_id, kind) DO UPDATE SET
                source = excluded.source, created_at = excluded.created_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(media_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    } else {
        sqlx::query("DELETE FROM artworks WHERE media_file_id = ? AND source = 'embedded'")
            .bind(media_id)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
    }
    refresh_search_row(
        &mut transaction,
        track_id,
        media_id,
        &info,
        &stat.relative_path,
    )
    .await?;
    transaction.commit().await.map_err(AppError::internal)
}

async fn upsert_artists(
    transaction: &mut Transaction<'_, Sqlite>,
    artists: &[String],
) -> Result<Vec<Uuid>, AppError> {
    let mut ids = Vec::with_capacity(artists.len());
    for artist in artists {
        let normalized = normalize_text(artist);
        let existing =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM artists WHERE normalized_name = ?")
                .bind(&normalized)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(AppError::internal)?;
        let id = existing.unwrap_or_else(Uuid::new_v4);
        sqlx::query("INSERT OR IGNORE INTO artists (id, name, normalized_name) VALUES (?, ?, ?)")
            .bind(id)
            .bind(artist)
            .bind(normalized)
            .execute(&mut **transaction)
            .await
            .map_err(AppError::internal)?;
        ids.push(id);
    }
    Ok(ids)
}

async fn upsert_album(
    transaction: &mut Transaction<'_, Sqlite>,
    info: &AudioInfo,
) -> Result<Option<Uuid>, AppError> {
    let normalized = normalize_text(&info.album);
    let album_artist = info
        .album_artist
        .clone()
        .or_else(|| info.artists.first().cloned())
        .unwrap_or_default();
    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM albums WHERE normalized_title = ? AND COALESCE(album_artist, '') = ? AND COALESCE(year, 0) = COALESCE(?, 0) LIMIT 1",
    )
    .bind(&normalized)
    .bind(&album_artist)
    .bind(info.year)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(AppError::internal)?;
    let id = existing.unwrap_or_else(Uuid::new_v4);
    sqlx::query(
        "INSERT OR IGNORE INTO albums (id, title, album_artist, normalized_title, year) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(&info.album)
    .bind(album_artist)
    .bind(normalized)
    .bind(info.year)
    .execute(&mut **transaction)
    .await
    .map_err(AppError::internal)?;
    Ok(Some(id))
}

async fn upsert_track(
    transaction: &mut Transaction<'_, Sqlite>,
    album_id: Option<Uuid>,
    info: &AudioInfo,
) -> Result<Uuid, AppError> {
    let normalized = normalize_text(&info.title);
    let existing = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM tracks
        WHERE normalized_title = ?
          AND COALESCE(album_id, '') = COALESCE(?, '')
          AND COALESCE(track_no, 0) = COALESCE(?, 0)
          AND ABS(COALESCE(duration_ms, 0) - COALESCE(?, 0)) <= 2000
        LIMIT 1
        "#,
    )
    .bind(&normalized)
    .bind(album_id)
    .bind(info.track_no)
    .bind(info.duration_ms)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(AppError::internal)?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO tracks
          (id, title, normalized_title, album_id, track_no, disc_no, year, genre, duration_ms, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(&info.title)
    .bind(normalized)
    .bind(album_id)
    .bind(info.track_no)
    .bind(info.disc_no)
    .bind(info.year)
    .bind(&info.genre)
    .bind(info.duration_ms)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(AppError::internal)?;
    Ok(id)
}

async fn refresh_search_row(
    transaction: &mut Transaction<'_, Sqlite>,
    track_id: Uuid,
    media_id: Uuid,
    info: &AudioInfo,
    path: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM track_search WHERE media_id = ?")
        .bind(media_id)
        .execute(&mut **transaction)
        .await
        .map_err(AppError::internal)?;
    let artist = info.artists.join(" / ");
    let normalized = search_aliases(&format!("{} {} {}", info.title, artist, info.album));
    sqlx::query(
        "INSERT INTO track_search (track_id, media_id, title, artist, album, path, normalized_text) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(track_id)
    .bind(media_id)
    .bind(&info.title)
    .bind(artist)
    .bind(&info.album)
    .bind(path)
    .bind(normalized)
    .execute(&mut **transaction)
    .await
    .map_err(AppError::internal)?;
    Ok(())
}

pub(super) async fn reconcile_removed_files(
    state: &AppState,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), AppError> {
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(
        "DELETE FROM media_files WHERE library_id = ? AND COALESCE(last_seen_scan_id, '') != ?",
    )
    .bind(library_id)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM track_search WHERE media_id NOT IN (SELECT id FROM media_files)")
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM tracks WHERE id NOT IN (SELECT DISTINCT track_id FROM media_files)")
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query(
        "DELETE FROM artists WHERE id NOT IN (SELECT DISTINCT artist_id FROM track_artists)",
    )
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)")
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)
}
