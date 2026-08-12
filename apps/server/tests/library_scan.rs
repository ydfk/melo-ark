use std::{fs, path::Path, time::Duration};

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
    db, jobs,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct TestContext {
    app: Router,
    _temp_dir: TempDir,
    database_path: String,
    library_path: String,
}

async fn test_context() -> TestContext {
    let temp_dir = tempfile::tempdir().expect("创建测试目录");
    let library = temp_dir.path().join("library");
    fs::create_dir(&library).expect("创建曲库目录");
    write_silent_wave(&library.join("sample.wav"));
    fs::hard_link(library.join("sample.wav"), library.join("alias.wav"))
        .expect("创建 Hardlink fixture");
    let database_path = temp_dir
        .path()
        .join("test.sqlite")
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
            path: database_path.clone(),
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
    TestContext {
        app: build_app(&config).await.expect("构建测试服务"),
        _temp_dir: temp_dir,
        database_path,
        library_path: library.to_string_lossy().into_owned(),
    }
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let request_body = if let Some(payload) = body {
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
        .oneshot(builder.body(request_body).expect("构建请求"))
        .await
        .expect("发送请求");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("读取响应")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("解析 JSON")
    };
    (status, value)
}

async fn authenticate(app: &Router) -> String {
    let credentials = json!({"username": "admin", "password": "pass123"});
    let (status, _) = request(
        app,
        "POST",
        "/api/auth/setup",
        Some(credentials.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = request(app, "POST", "/api/auth/login", Some(credentials), None).await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().expect("JWT").to_owned()
}

async fn wait_for_job(app: &Router, token: &str, id: &str) -> Value {
    for _ in 0..200 {
        let (status, body) =
            request(app, "GET", &format!("/api/jobs/{id}"), None, Some(token)).await;
        assert_eq!(status, StatusCode::OK);
        if matches!(
            body["status"].as_str(),
            Some("completed" | "completed_with_errors" | "failed" | "cancelled")
        ) {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("扫描任务未在测试时限内完成")
}

#[tokio::test]
async fn scan_is_incremental_and_recognizes_hardlink_identity() {
    let context = test_context().await;
    let token = authenticate(&context.app).await;
    let (status, library) = request(
        &context.app,
        "POST",
        "/api/libraries",
        Some(json!({
            "path": context.library_path,
            "role": "source",
            "scanEnabled": true,
            "watchEnabled": false,
            "writable": false
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let library_id = library["id"].as_str().expect("Library ID");
    assert!(library.get("name").is_none());
    assert_eq!(library["path"], context.library_path);

    let first = start_scan(&context.app, &token, library_id).await;
    assert_eq!(first["status"], "completed");
    assert_eq!(first["successItems"], 2);

    let pool = db::connect(&context.database_path)
        .await
        .expect("连接测试数据库");
    let identities: Vec<(String, String)> =
        sqlx::query_as("SELECT device_id, inode FROM media_files ORDER BY relative_path")
            .fetch_all(&pool)
            .await
            .expect("读取媒体物理标识");
    assert_eq!(identities.len(), 2);
    assert_eq!(identities[0], identities[1]);

    let second = start_scan(&context.app, &token, library_id).await;
    assert_eq!(second["status"], "completed");
    assert_eq!(second["skippedItems"], 2);
    assert_eq!(second["successItems"], 0);

    let (status, tracks) = request(
        &context.app,
        "GET",
        "/api/tracks?page=1&perPage=20",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tracks["total"], 2);
    for field in [
        "mediaId",
        "extension",
        "qualityScore",
        "hasLyrics",
        "hasArtwork",
        "tagHealth",
        "path",
    ] {
        assert!(
            tracks["items"][0].get(field).is_some(),
            "缺少曲目字段 {field}"
        );
    }
    let (status, missing_lyrics) = request(
        &context.app,
        "GET",
        "/api/tracks?filter=missing_lyrics",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(missing_lyrics["total"], 2);
    let (status, dashboard) = request(
        &context.app,
        "GET",
        "/api/dashboard/stats",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dashboard["trackCount"], 2);
    assert_eq!(dashboard["mediaFileCount"], 2);
    assert_eq!(dashboard["missingLyricsCount"], 2);
    assert_eq!(dashboard["missingCoverCount"], 2);
    assert!(dashboard["recentScanAt"].is_string());
    assert_eq!(dashboard["formatDistribution"][0]["extension"], "WAV");
    assert_eq!(dashboard["formatDistribution"][0]["count"], 2);
    assert_eq!(dashboard["recentAdded"].as_array().map(Vec::len), Some(2));
    assert!(
        dashboard["recentPlayed"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(first.get("itemsPerSecond").is_some());
    assert!(first.get("etaSeconds").is_some());
    let first_id = first["id"].as_str().expect("任务 ID");
    let (status, logs) = request(
        &context.app,
        "GET",
        &format!("/api/jobs/{first_id}/logs?limit=200"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        logs["items"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    let (status, first_log_page) = request(
        &context.app,
        "GET",
        &format!("/api/jobs/{first_id}/logs?limit=1"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_log_page["items"].as_array().map(Vec::len), Some(1));
    assert!(first_log_page["nextBefore"].is_number());
    let (status, error_logs) = request(
        &context.app,
        "GET",
        &format!("/api/jobs/{first_id}/logs?level=error"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(error_logs["items"].as_array().is_some_and(Vec::is_empty));

    let (status, _) = request(
        &context.app,
        "DELETE",
        &format!("/api/libraries/{library_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        Path::new(&context.library_path)
            .join("sample.wav")
            .is_file()
    );
    assert!(Path::new(&context.library_path).join("alias.wav").is_file());
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM media_files), (SELECT COUNT(*) FROM tracks), (SELECT COUNT(*) FROM track_search)",
    )
    .fetch_one(&pool)
    .await
    .expect("读取删除曲库后的索引计数");
    assert_eq!(counts, (0, 0, 0));
    let task_counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM jobs WHERE kind = 'scan'), (SELECT COUNT(*) FROM job_items), (SELECT COUNT(*) FROM job_logs)",
    )
    .fetch_one(&pool)
    .await
    .expect("读取删除曲库后的任务计数");
    assert_eq!(task_counts, (0, 0, 0));
}

#[tokio::test]
async fn expired_logs_are_cleaned_without_deleting_job_summary() {
    let context = test_context().await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接测试数据库");
    let job_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO jobs (id, kind, status, created_at, finished_at, updated_at) VALUES (?, 'analyze', 'completed', ?, ?, ?)",
    )
    .bind(job_id)
    .bind(chrono::Utc::now() - chrono::Duration::days(40))
    .bind(chrono::Utc::now() - chrono::Duration::days(40))
    .bind(chrono::Utc::now() - chrono::Duration::days(40))
    .execute(&pool)
    .await
    .expect("写入历史任务");
    sqlx::query(
        "INSERT INTO job_logs (job_id, level, event_type, message, created_at) VALUES (?, 'info', 'completed', '历史日志', ?)",
    )
    .bind(job_id)
    .bind(chrono::Utc::now() - chrono::Duration::days(40))
    .execute(&pool)
    .await
    .expect("写入历史日志");

    assert_eq!(
        jobs::cleanup_expired_logs(&pool).await.expect("清理日志"),
        1
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM jobs WHERE id = ?), (SELECT COUNT(*) FROM job_logs WHERE job_id = ?)",
    )
    .bind(job_id)
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .expect("读取日志清理结果");
    assert_eq!(counts, (1, 0));
}

async fn start_scan(app: &Router, token: &str, library_id: &str) -> Value {
    let (status, job) = request(
        app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(token),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    wait_for_job(app, token, job["id"].as_str().expect("Job ID")).await
}

fn write_silent_wave(path: &Path) {
    let samples = [0_u8; 160];
    let data_size = samples.len() as u32;
    let mut bytes = Vec::with_capacity(44 + samples.len());
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.extend_from_slice(&samples);
    fs::write(path, bytes).expect("写入 WAV fixture");
}
