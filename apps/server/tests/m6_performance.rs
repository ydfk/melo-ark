use std::time::{Duration, Instant};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use meloark_server::{
    build_app,
    config::{
        AiConfig, AnalysisConfig, AppConfig, DatabaseConfig, JwtConfig, LoggingConfig,
        PlaybackConfig, ProviderConfig, ScanConfig, ServerConfig,
    },
    db,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

const FAKE_TRACK_COUNT: i64 = 50_001;
const QUERY_BUDGET: Duration = Duration::from_secs(5);

#[tokio::test]
async fn fifty_thousand_track_catalog_remains_paginated_and_searchable() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let (app, _temp, database) = test_app().await;
    let token = setup(&app).await;
    seed_fake_catalog(&database).await;

    let started = Instant::now();
    let (status, list) = request(&app, "/api/tracks?page=1&perPage=50", &token).await;
    let list_elapsed = started.elapsed();
    assert_eq!(status, StatusCode::OK, "list response: {list}");
    assert_eq!(list["total"], FAKE_TRACK_COUNT);
    assert_eq!(list["items"].as_array().expect("分页结果").len(), 50);
    assert!(
        list_elapsed < QUERY_BUDGET,
        "50k 首屏分页耗时 {list_elapsed:?}，超过 {QUERY_BUDGET:?}"
    );

    let started = Instant::now();
    let (status, result) =
        request(&app, "/api/tracks?page=1&perPage=50&search=42424", &token).await;
    let search_elapsed = started.elapsed();
    assert_eq!(status, StatusCode::OK, "search response: {result}");
    assert_eq!(result["total"], 1);
    assert_eq!(result["items"][0]["title"], "Fake Track 42424");
    assert!(
        search_elapsed < QUERY_BUDGET,
        "50k FTS 查询耗时 {search_elapsed:?}，超过 {QUERY_BUDGET:?}"
    );

    eprintln!(
        "50k catalog profile: list={list_elapsed:?}, search={search_elapsed:?}, rows={FAKE_TRACK_COUNT}"
    );
}

async fn test_app() -> (Router, TempDir, String) {
    let temp = tempfile::tempdir().expect("测试目录");
    let database = temp
        .path()
        .join("performance.sqlite")
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
            secret: "test-secret-at-least-sixteen-characters".to_owned(),
            expiration: 3600,
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
    (build_app(&config).await.expect("服务"), temp, database)
}

async fn setup(app: &Router) -> String {
    let credentials = json!({"username":"admin","password":"pass123"});
    json_call(
        app,
        "POST",
        "/api/auth/setup",
        Some(credentials.clone()),
        None,
    )
    .await;
    let (_, login) = json_call(app, "POST", "/api/auth/login", Some(credentials), None).await;
    login["token"].as_str().expect("JWT").to_owned()
}

async fn seed_fake_catalog(database: &str) {
    let pool = db::connect(database).await.expect("数据库");
    let library_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("曲库 ID");
    sqlx::query("INSERT INTO libraries (id,name,path,scan_enabled,watch_enabled,writable,role,exclude_patterns,created_at,updated_at) VALUES (?, '50k Fixture', '/virtual/music', 1, 0, 0, 'source', '[]', datetime('now'), datetime('now'))")
        .bind(library_id)
        .execute(&pool)
        .await
        .expect("曲库 fixture");
    sqlx::query(
        r#"
        WITH RECURSIVE counter(value) AS (
          SELECT 1 UNION ALL SELECT value + 1 FROM counter WHERE value < ?
        )
        INSERT INTO tracks (id,title,normalized_title,duration_ms,created_at,updated_at)
        SELECT unhex(printf('00000000000000010000%012d', value)),
               'Fake Track ' || value,
               'fake track ' || value,
               180000,
               '2026-08-10T00:00:00Z',
               printf('2026-08-10T00:00:%02dZ', value % 60)
        FROM counter
        "#,
    )
    .bind(FAKE_TRACK_COUNT)
    .execute(&pool)
    .await
    .expect("曲目 fixture");
    sqlx::query(
        r#"
        INSERT INTO media_files (
          id,track_id,library_id,relative_path,extension,file_size,mtime_ms,
          device_id,inode,hardlink_count,codec,duration_ms,metadata_readable,
          metadata_writable,created_at,updated_at
        )
        SELECT unhex('10000000000000010000' || substr(hex(id), 21, 12)),
               id, ?, 'Fake/' || title || '.flac', 'flac', 25000000, 1,
               'fixture', substr(hex(id), 21, 12), 1, 'flac', duration_ms, 1, 0,
               created_at, updated_at
        FROM tracks
        "#,
    )
    .bind(library_id)
    .execute(&pool)
    .await
    .expect("媒体 fixture");
    sqlx::query(
        r#"
        INSERT INTO track_search (track_id,media_id,title,artist,album,path,normalized_text)
        SELECT t.id,mf.id,t.title,'Fixture Artist','Fixture Album',mf.relative_path,
               t.normalized_title || ' fixture artist fixture album'
        FROM tracks t JOIN media_files mf ON mf.track_id=t.id
        "#,
    )
    .execute(&pool)
    .await
    .expect("FTS fixture");
}

async fn request(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    json_call(app, "GET", uri, None, Some(token)).await
}

async fn json_call(
    app: &Router,
    method: &str,
    uri: &str,
    payload: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = if let Some(payload) = payload {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(payload.to_string())
    } else {
        Body::empty()
    };
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("请求"))
        .await
        .expect("响应");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON")
    };
    (status, value)
}
