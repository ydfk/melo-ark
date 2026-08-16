use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    routing::get,
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

struct Context {
    app: Router,
    _temp: TempDir,
    database: String,
    source: std::path::PathBuf,
}

async fn context() -> Context {
    let temp = tempfile::tempdir().expect("创建测试目录");
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("创建来源目录");
    write_silent_wave(&source.join("晴天.wav"));
    let database = temp.path().join("db.sqlite").to_string_lossy().into_owned();
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
    Context {
        app: build_app(&config).await.expect("构建服务"),
        _temp: temp,
        database,
        source,
    }
}

#[tokio::test]
async fn provider_candidates_require_confidence_confirmation_and_batch_job_is_persistent() {
    let context = context().await;
    let token = authenticate_and_scan(&context).await;
    let pool = db::connect(&context.database).await.expect("连接数据库");
    let (track_id, _, _) = managed_media(&pool).await;
    let candidate_id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO scrape_candidates (id, track_id, provider_id, provider_item_id, title, artists_json, album, duration_ms, year, track_no, score, confidence, differences_json, raw_json, created_at)
      VALUES (?, ?, 'musicbrainz', 'fixture-1', '晴天', '["周杰伦"]', '叶惠美', 269000, 2003, 3, 85, 'review', '["album"]', '{}', ?)"#)
      .bind(candidate_id).bind(track_id).bind(chrono::Utc::now()).execute(&pool).await.expect("插入候选");

    let (status, providers) =
        request(&context.app, "GET", "/api/providers", None, Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(providers.as_array().expect("Provider 数组").len(), 7);
    assert!(
        providers
            .as_array()
            .expect("Provider 数组")
            .iter()
            .any(|item| item["providerId"] == "musicbrainz"
                && item["capabilities"]["metadata"] == true)
    );
    let (status, updated_provider) = request(
        &context.app,
        "PATCH",
        "/api/providers/musicbrainz",
        Some(json!({"priority": 55, "timeoutMs": 1500, "rateLimitMs": 250})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated_provider["priority"], 55);
    assert_eq!(updated_provider["timeoutMs"], 1500);
    assert_eq!(updated_provider["rateLimitMs"], 250);
    assert_eq!(
        request(
            &context.app,
            "PATCH",
            "/api/providers/musicbrainz",
            Some(json!({"timeoutMs": 50})),
            Some(&token),
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (status, problem) = request(
        &context.app,
        "POST",
        "/api/scrape/apply",
        Some(
            json!({"candidateId": candidate_id, "confirmation": "APPLY", "includeArtwork": false}),
        ),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        problem["detail"]
            .as_str()
            .is_some_and(|value| value.contains("APPLY_REVIEWED"))
    );
    let (status, operation) = request(&context.app, "POST", "/api/scrape/apply", Some(json!({"candidateId": candidate_id, "confirmation": "APPLY_REVIEWED", "includeArtwork": false})), Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(operation["status"], "previewed");

    let (status, job) = request(
        &context.app,
        "POST",
        "/api/scrape/jobs",
        Some(json!({"trackIds": [track_id], "providerIds": ["kuwo"]})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let job = wait_job(&context.app, &token, job["id"].as_str().expect("任务 ID")).await;
    assert_eq!(job["kind"], "scrape");
    assert_eq!(job["status"], "completed_with_errors");
}

#[tokio::test]
async fn local_lrc_is_scored_and_never_silently_overwritten() {
    let context = context().await;
    let token = authenticate_and_scan(&context).await;
    let pool = db::connect(&context.database).await.expect("连接数据库");
    let (track_id, media_id, media_path) = managed_media(&pool).await;
    let lrc_path = media_path.with_extension("lrc");
    let content =
        "[00:00.00]故事的小黄花\n[00:00.00]The little yellow flower\n[00:00.01]从出生那年就飘着";
    fs::write(&lrc_path, content).expect("写入 LRC");
    let (status, searched) = request(
        &context.app,
        "POST",
        "/api/lyrics/search",
        Some(json!({"trackId": track_id})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let candidate = &searched["candidates"][0];
    assert_eq!(candidate["synced"], true);
    assert!(
        candidate["qualityScore"]
            .as_i64()
            .is_some_and(|score| score > 0)
    );

    let body = json!({"lyricsId": candidate["id"], "mediaFileId": media_id, "mode": "external", "replaceExisting": false, "confirmation": "USE_LYRICS"});
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/lyrics/apply",
            Some(body.clone()),
            Some(&token)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let failed_job_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM jobs WHERE kind = 'lyrics' AND status = 'completed_with_errors' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("失败歌词任务");
    fs::remove_file(&lrc_path).expect("删除原 LRC");
    let (status, retried) = request(
        &context.app,
        "POST",
        &format!("/api/jobs/{failed_job_id}/retry-failed"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried["status"], "completed");
    assert_eq!(retried["successItems"], 1);
    let active: bool = sqlx::query_scalar("SELECT active FROM lyrics WHERE id = ?")
        .bind(Uuid::parse_str(candidate["id"].as_str().expect("歌词 ID")).expect("有效歌词 UUID"))
        .fetch_one(&pool)
        .await
        .expect("歌词激活状态");
    assert!(active);
    assert_eq!(
        fs::read_to_string(&lrc_path).expect("读取生成 LRC"),
        content
    );
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/lyrics/apply",
            Some(body),
            Some(&token)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn provider_retries_transient_http_failure_with_configured_timeout() {
    let context = context().await;
    let token = authenticate_and_scan(&context).await;
    let pool = db::connect(&context.database).await.expect("连接数据库");
    let (track_id, _, _) = managed_media(&pool).await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let mock = Router::new()
        .route("/ws/2/recording", get(flaky_musicbrainz))
        .with_state(attempts.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("绑定 Provider fixture");
    let address = listener.local_addr().expect("Provider fixture 地址");
    let server = tokio::spawn(async move {
        axum::serve(listener, mock)
            .await
            .expect("运行 Provider fixture");
    });
    sqlx::query("UPDATE provider_settings SET enabled = 0")
        .execute(&pool)
        .await
        .expect("关闭其他 Provider");
    sqlx::query(
        "UPDATE provider_settings SET enabled = 1, base_url = ?, timeout_ms = 500, rate_limit_ms = 1 WHERE provider_id = 'musicbrainz'",
    )
    .bind(format!("http://{address}"))
    .execute(&pool)
    .await
    .expect("配置 MusicBrainz fixture");
    sqlx::query("DELETE FROM provider_cache WHERE provider_id = 'musicbrainz'")
        .execute(&pool)
        .await
        .expect("清除接入阶段的数据源缓存");

    let (status, response) = request(
        &context.app,
        "POST",
        "/api/scrape/search",
        Some(json!({"trackId": track_id, "providerIds": ["musicbrainz"]})),
        Some(&token),
    )
    .await;
    server.abort();

    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(response["candidates"][0]["providerId"], "musicbrainz");
    assert_eq!(response["failures"].as_array().map(Vec::len), Some(0));
}

async fn flaky_musicbrainz(State(attempts): State<Arc<AtomicUsize>>) -> (StatusCode, Json<Value>) {
    if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "temporary"})),
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "recordings": [{
                "id": "mb-retry-1",
                "title": "晴天",
                "length": 10,
                "artist-credit": [{"name": "未知艺术家"}],
                "releases": [{"id": "release-1", "title": "未分类", "date": "2003"}]
            }]
        })),
    )
}

async fn authenticate_and_scan(context: &Context) -> String {
    let credentials = json!({"username": "admin", "password": "pass123"});
    request(
        &context.app,
        "POST",
        "/api/auth/setup",
        Some(credentials.clone()),
        None,
    )
    .await;
    let (_, login) = request(
        &context.app,
        "POST",
        "/api/auth/login",
        Some(credentials),
        None,
    )
    .await;
    let token = login["token"].as_str().expect("JWT").to_owned();
    let organized = context
        .source
        .parent()
        .expect("测试根目录")
        .join("organized");
    fs::create_dir(&organized).expect("创建整理目录");
    let (status, library) = request(&context.app, "POST", "/api/libraries", Some(json!({"sourcePath": context.source, "organizedPath": organized, "watchEnabled":false, "autoIngestEnabled":true})), Some(&token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, job) = request(
        &context.app,
        "POST",
        &format!(
            "/api/libraries/{}/scan",
            library["sources"][0]["id"].as_str().expect("来源 ID")
        ),
        None,
        Some(&token),
    )
    .await;
    wait_job(
        &context.app,
        &token,
        job["id"].as_str().expect("扫描任务 ID"),
    )
    .await;
    wait_for_ingest_job(&context.app, &token).await;
    token
}

async fn managed_media(pool: &sqlx::SqlitePool) -> (Uuid, Uuid, std::path::PathBuf) {
    let (track_id, media_id, library_path, relative_path) =
        sqlx::query_as::<_, (Uuid, Uuid, String, String)>(
            "SELECT mf.track_id, mf.id, l.path, mf.relative_path
             FROM media_files mf
             JOIN libraries l ON l.id = mf.library_id
             WHERE l.role = 'managed' AND mf.available = 1
             ORDER BY mf.created_at DESC
             LIMIT 1",
        )
        .fetch_one(pool)
        .await
        .expect("整理媒体");
    (
        track_id,
        media_id,
        std::path::PathBuf::from(library_path).join(relative_path),
    )
}

async fn wait_for_ingest_job(app: &Router, token: &str) {
    for _ in 0..600 {
        let (_, jobs) = request(app, "GET", "/api/jobs?limit=100", None, Some(token)).await;
        let Some(job) = jobs
            .as_array()
            .and_then(|items| items.iter().find(|job| job["kind"] == "ingest"))
        else {
            tokio::time::sleep(Duration::from_millis(20)).await;
            continue;
        };
        if matches!(
            job["status"].as_str(),
            Some("completed" | "completed_with_errors" | "failed")
        ) {
            assert_ne!(job["status"], "failed", "接入任务失败：{job}");
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("接入任务未完成")
}

async fn wait_job(app: &Router, token: &str, id: &str) -> Value {
    for _ in 0..300 {
        let (_, job) = request(app, "GET", &format!("/api/jobs/{id}"), None, Some(token)).await;
        if matches!(
            job["status"].as_str(),
            Some("completed" | "completed_with_errors" | "failed")
        ) {
            return job;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("任务未完成")
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let payload = if let Some(value) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(value.to_string())
    } else {
        Body::empty()
    };
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(payload).expect("请求"))
        .await
        .expect("响应");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("读取响应")
        .to_bytes();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("JSON")
        },
    )
}

fn write_silent_wave(path: &Path) {
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
