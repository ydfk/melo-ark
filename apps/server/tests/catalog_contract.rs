use std::{fs, path::Path};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::Utc;
use http_body_util::BodyExt;
use meloark_server::{
    build_app,
    config::{
        AiConfig, AnalysisConfig, AppConfig, DatabaseConfig, JwtConfig, LoggingConfig,
        PlaybackConfig, ProviderConfig, ScanConfig, ServerConfig,
    },
    db,
};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

struct CatalogContext {
    app: Router,
    pool: sqlx::SqlitePool,
    _temp: TempDir,
    track_id: Uuid,
    media_id: Uuid,
}

async fn context() -> CatalogContext {
    let temp = tempfile::tempdir().expect("测试目录");
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("来源目录");
    let audio = source.join("夜曲.wav");
    write_wave(&audio);
    let database = temp
        .path()
        .join("catalog.sqlite")
        .to_string_lossy()
        .into_owned();
    let config = AppConfig {
        app: ServerConfig {
            host: "127.0.0.1".to_owned(),
            port: 0,
            environment: "test".to_owned(),
            web_dist: None,
        },
        jwt: JwtConfig {
            secret: "catalog-test-secret-at-least-sixteen-characters".to_owned(),
            expiration: 3_600,
        },
        database: DatabaseConfig {
            path: database.clone(),
        },
        logging: LoggingConfig {
            filter: "off".to_owned(),
        },
        scan: ScanConfig::default(),
        providers: ProviderConfig::default(),
        analysis: AnalysisConfig::default(),
        ai: AiConfig::default(),
        playback: PlaybackConfig::default(),
    };
    let app = build_app(&config).await.expect("构建服务");
    let pool = db::connect(&database).await.expect("连接数据库");
    let track_id = Uuid::new_v4();
    let media_id = Uuid::new_v4();
    insert_catalog_fixture(&pool, &source, &audio, track_id, media_id).await;
    CatalogContext {
        app,
        pool,
        _temp: temp,
        track_id,
        media_id,
    }
}

#[tokio::test]
async fn anonymous_catalog_only_exposes_playback_fields() {
    let context = context().await;
    let (status, page) = json_request(
        &context.app,
        "GET",
        "/api/catalog/tracks?page=1&perPage=10&search=%E5%A4%9C%E6%9B%B2",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["id"], context.track_id.to_string());
    assert_eq!(page["items"][0]["mediaId"], context.media_id.to_string());
    assert_eq!(page["items"][0]["title"], "夜曲");
    assert_eq!(page["items"][0]["artist"], "周杰伦");
    assert_eq!(page["items"][0]["album"], "十一月的萧邦");
    assert_eq!(page["items"][0]["hasLyrics"], true);
    for internal in [
        "path",
        "libraryPath",
        "libraryId",
        "qualityScore",
        "codec",
        "fileSize",
        "metadataWritable",
        "missingSince",
    ] {
        assert!(
            page["items"][0].get(internal).is_none(),
            "公开目录泄露字段 {internal}"
        );
    }

    let (status, albums) = json_request(&context.app, "GET", "/api/catalog/albums?limit=8").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(albums.as_array().map(Vec::len), Some(1));
    assert_eq!(albums[0]["title"], "十一月的萧邦");
    assert_eq!(albums[0]["trackCount"], 1);

    let (status, lyrics) = json_request(
        &context.app,
        "GET",
        &format!("/api/catalog/tracks/{}/lyrics", context.track_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lyrics["content"], "[00:01.00]有效歌词");
    assert!(lyrics.get("qualityScore").is_none());
    assert!(lyrics.get("providerId").is_none());
    assert!(lyrics.get("storage").is_none());
}

#[tokio::test]
async fn anonymous_token_streams_without_history_but_management_stays_protected() {
    let context = context().await;
    let (status, ticket) = json_request(
        &context.app,
        "POST",
        &format!("/api/catalog/media/{}/play-token", context.media_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ticket["expiresIn"], 600);
    let token = ticket["token"].as_str().expect("媒体令牌");

    let response = raw(
        &context.app,
        "GET",
        &format!("/api/media/{}/stream?token={token}", context.media_id),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .expect("音频")
            .to_bytes()
            .len(),
        204
    );

    let artwork = raw(
        &context.app,
        "GET",
        &format!("/api/catalog/artwork/{}", context.media_id),
    )
    .await;
    // fixture 没有内嵌封面；404 而非 401 证明公开接口已完成媒体读取。
    assert_eq!(artwork.status(), StatusCode::NOT_FOUND);

    let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM play_history")
        .fetch_one(&context.pool)
        .await
        .expect("历史计数");
    assert_eq!(history_count, 0);
    for (method, uri) in [
        ("GET", "/api/tracks"),
        ("GET", "/api/favorites"),
        ("GET", "/api/playlists"),
    ] {
        assert_eq!(
            raw(&context.app, method, uri).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        raw_json(
            &context.app,
            "POST",
            "/api/playback/scrobble",
            &format!(r#"{{"trackId":"{}","completed":true}}"#, context.track_id),
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn unavailable_media_is_hidden_and_cannot_receive_a_token() {
    let context = context().await;
    sqlx::query("UPDATE media_files SET available = 0, missing_since = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(context.media_id)
        .execute(&context.pool)
        .await
        .expect("标记不可用");

    let (status, page) = json_request(&context.app, "GET", "/api/catalog/tracks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 0);
    let (status, albums) = json_request(&context.app, "GET", "/api/catalog/albums").await;
    assert_eq!(status, StatusCode::OK);
    assert!(albums.as_array().is_some_and(Vec::is_empty));
    assert_eq!(
        raw(
            &context.app,
            "POST",
            &format!("/api/catalog/media/{}/play-token", context.media_id),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        raw(
            &context.app,
            "GET",
            &format!("/api/catalog/artwork/{}", context.media_id),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        raw(
            &context.app,
            "GET",
            &format!("/api/catalog/tracks/{}/lyrics", context.track_id),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn source_media_is_never_exposed_as_playable_catalog_content() {
    let context = context().await;
    sqlx::query("UPDATE libraries SET role = 'source', writable = 0")
        .execute(&context.pool)
        .await
        .expect("切换为来源目录");
    let (status, page) = json_request(&context.app, "GET", "/api/catalog/tracks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 0);
    assert_eq!(
        raw(
            &context.app,
            "POST",
            &format!("/api/catalog/media/{}/play-token", context.media_id),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

async fn insert_catalog_fixture(
    pool: &sqlx::SqlitePool,
    source: &Path,
    audio: &Path,
    track_id: Uuid,
    media_id: Uuid,
) {
    let library_id = Uuid::new_v4();
    let artist_id = Uuid::new_v4();
    let album_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query("INSERT INTO libraries (id, name, path, writable, role, created_at, updated_at) VALUES (?, ?, ?, 1, 'managed', ?, ?)")
        .bind(library_id)
        .bind(source.to_string_lossy().as_ref())
        .bind(source.to_string_lossy().as_ref())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("插入曲库");
    sqlx::query("INSERT INTO artists (id, name, normalized_name) VALUES (?, '周杰伦', '周杰伦')")
        .bind(artist_id)
        .execute(pool)
        .await
        .expect("插入艺术家");
    sqlx::query("INSERT INTO albums (id, title, album_artist, normalized_title, year) VALUES (?, '十一月的萧邦', '周杰伦', '十一月的萧邦', 2005)")
        .bind(album_id)
        .execute(pool)
        .await
        .expect("插入专辑");
    sqlx::query("INSERT INTO tracks (id, title, normalized_title, album_id, year, duration_ms, created_at, updated_at) VALUES (?, '夜曲', '夜曲', ?, 2005, 240000, ?, ?)")
        .bind(track_id)
        .bind(album_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("插入曲目");
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, position) VALUES (?, ?, 0)")
        .bind(track_id)
        .bind(artist_id)
        .execute(pool)
        .await
        .expect("关联艺术家");
    sqlx::query(
        r#"INSERT INTO media_files (
          id, track_id, library_id, relative_path, extension, file_size, mtime_ms,
          device_id, inode, hardlink_count, metadata_readable, metadata_writable,
          fingerprint_status, hash_status, created_at, updated_at
        ) VALUES (?, ?, ?, '夜曲.wav', 'wav', ?, 1, 'fixture', '1', 1, 1, 0,
          'pending', 'pending', ?, ?)"#,
    )
    .bind(media_id)
    .bind(track_id)
    .bind(library_id)
    .bind(fs::metadata(audio).expect("媒体信息").len() as i64)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("插入媒体");
    sqlx::query("INSERT INTO track_search (track_id, media_id, title, artist, album, path, normalized_text) VALUES (?, ?, '夜曲', '周杰伦', '十一月的萧邦', '夜曲.wav', '夜曲 周杰伦 十一月的萧邦')")
        .bind(track_id)
        .bind(media_id)
        .execute(pool)
        .await
        .expect("插入搜索索引");
    for (active, content, score) in [
        (false, "[00:01.00]候选歌词", 99_i64),
        (true, "[00:01.00]有效歌词", 80_i64),
    ] {
        sqlx::query("INSERT INTO lyrics (id, track_id, format, language, content, synced, quality_score, storage, active, created_at, updated_at) VALUES (?, ?, 'lrc', 'zh', ?, 1, ?, 'candidate', ?, ?, ?)")
            .bind(Uuid::new_v4())
            .bind(track_id)
            .bind(content)
            .bind(score)
            .bind(active)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .expect("插入歌词");
    }
}

async fn json_request(app: &Router, method: &str, uri: &str) -> (StatusCode, Value) {
    let response = raw(app, method, uri).await;
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("读取响应")
        .to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON 响应")
    };
    (status, body)
}

async fn raw(app: &Router, method: &str, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .expect("构建请求"),
        )
        .await
        .expect("发送请求")
}

async fn raw_json(app: &Router, method: &str, uri: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .expect("构建请求"),
        )
        .await
        .expect("发送请求")
}

fn write_wave(path: &Path) {
    let samples = [0_u8; 160];
    let mut bytes = Vec::with_capacity(44 + samples.len());
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&samples);
    fs::write(path, bytes).expect("写入 WAV");
}
