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
    managed_path: String,
}

async fn test_context() -> TestContext {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("meloark_server=error")
        .with_test_writer()
        .try_init();
    let temp_dir = tempfile::tempdir().expect("创建测试目录");
    let library = temp_dir.path().join("library");
    fs::create_dir(&library).expect("创建曲库目录");
    write_silent_wave(&library.join("sample.wav"));
    fs::hard_link(library.join("sample.wav"), library.join("alias.wav"))
        .expect("创建 Hardlink fixture");
    let managed = temp_dir.path().join("managed");
    fs::create_dir(&managed).expect("创建已整理曲库目录");
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
        managed_path: managed.to_string_lossy().into_owned(),
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
            "sourcePath": context.library_path,
            "organizedPath": context.managed_path,
            "watchEnabled": false,
            "autoIngestEnabled": false
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let library_id = library["sources"][0]["id"].as_str().expect("来源 ID");
    let library_uuid = uuid::Uuid::parse_str(library_id).expect("Library UUID");
    assert!(library.get("role").is_none());
    assert_eq!(library["organizedPath"], context.managed_path);

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

    fs::remove_file(Path::new(&context.library_path).join("alias.wav")).expect("移除来源文件");
    let missing_scan = start_scan(&context.app, &token, library_id).await;
    assert_eq!(missing_scan["status"], "completed");
    let missing: (bool, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT available, missing_since FROM media_files WHERE library_id = ? AND relative_path = 'alias.wav'",
    )
    .bind(library_uuid)
    .fetch_one(&pool)
    .await
    .expect("读取来源缺失状态");
    assert!(!missing.0);
    assert!(missing.1.is_some());
    let (status, reviews) = request(
        &context.app,
        "GET",
        "/api/reviews?status=pending&kind=source_missing",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reviews}");
    assert_eq!(reviews["total"], 1);

    fs::hard_link(
        Path::new(&context.library_path).join("sample.wav"),
        Path::new(&context.library_path).join("alias.wav"),
    )
    .expect("恢复来源文件");
    let recovered_scan = start_scan(&context.app, &token, library_id).await;
    assert_eq!(recovered_scan["status"], "completed");
    let recovered: (bool, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT available, missing_since FROM media_files WHERE library_id = ? AND relative_path = 'alias.wav'",
    )
    .bind(library_uuid)
    .fetch_one(&pool)
    .await
    .expect("读取来源恢复状态");
    assert!(recovered.0);
    assert!(recovered.1.is_none());
    let (status, resolved_reviews) = request(
        &context.app,
        "GET",
        "/api/reviews?status=resolved&kind=source_missing",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resolved_reviews["total"], 1);

    let (status, tracks) = request(
        &context.app,
        "GET",
        "/api/tracks?page=1&perPage=20",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tracks["total"], 0);
    let (status, missing_lyrics) = request(
        &context.app,
        "GET",
        "/api/tracks?filter=missing_lyrics",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(missing_lyrics["total"], 0);
    let (status, dashboard) = request(
        &context.app,
        "GET",
        "/api/dashboard/stats",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dashboard["trackCount"], 0);
    assert_eq!(dashboard["mediaFileCount"], 0);
    assert_eq!(dashboard["missingLyricsCount"], 0);
    assert_eq!(dashboard["missingCoverCount"], 0);
    assert!(dashboard["recentScanAt"].is_string());
    assert!(
        dashboard["formatDistribution"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(
        dashboard["recentAdded"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
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

#[tokio::test]
async fn library_groups_reuse_targets_and_create_directories_safely() {
    let context = test_context().await;
    let token = authenticate(&context.app).await;
    let root = Path::new(&context.library_path)
        .parent()
        .expect("测试根目录");
    let (status, created) = request(
        &context.app,
        "POST",
        "/api/filesystem/directories",
        Some(json!({"parentPath": root, "name": "organized-new"})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let organized_path = created["path"].as_str().expect("新目录路径");
    assert!(Path::new(organized_path).is_dir());
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/filesystem/directories",
            Some(json!({"parentPath": root, "name": "../escape"})),
            Some(&token),
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let second_source = root.join("source-two");
    fs::create_dir(&second_source).expect("第二来源目录");
    let mut source_ids = Vec::new();
    let mut target_id = String::new();
    for source_path in [Path::new(&context.library_path), second_source.as_path()] {
        let (status, group) = request(
            &context.app,
            "POST",
            "/api/libraries",
            Some(json!({
                "sourcePath": source_path,
                "organizedPath": organized_path,
                "watchEnabled": false,
                "autoIngestEnabled": true
            })),
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(group.get("role").is_none());
        let current_target = group["organizedLibraryId"].as_str().expect("整理目录 ID");
        if target_id.is_empty() {
            target_id = current_target.to_owned();
        } else {
            assert_eq!(current_target, target_id);
        }
        let canonical_source = source_path.canonicalize().expect("来源规范路径");
        let source = group["sources"]
            .as_array()
            .expect("来源列表")
            .iter()
            .find(|item| item["sourcePath"] == canonical_source.to_string_lossy().as_ref())
            .expect("当前来源");
        source_ids.push(source["id"].as_str().expect("来源 ID").to_owned());
    }
    let (status, groups) = request(&context.app, "GET", "/api/libraries", None, Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(groups.as_array().map(Vec::len), Some(1));
    assert_eq!(groups[0]["sources"].as_array().map(Vec::len), Some(2));

    let replacement_target = root.join("organized-replacement");
    fs::create_dir(&replacement_target).expect("新整理目录");
    for (index, source_id) in source_ids.iter().enumerate() {
        let (status, updated) = request(
            &context.app,
            "PATCH",
            &format!("/api/libraries/{source_id}"),
            Some(json!({"organizedPath": replacement_target})),
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            updated["organizedPath"],
            replacement_target
                .canonicalize()
                .expect("新整理目录规范路径")
                .to_string_lossy()
                .as_ref()
        );
        let (_, current_groups) =
            request(&context.app, "GET", "/api/libraries", None, Some(&token)).await;
        assert_eq!(current_groups.as_array().map(Vec::len), Some(2 - index));
    }
    assert!(Path::new(organized_path).is_dir());

    let nested = Path::new(&context.library_path).join("nested-target");
    fs::create_dir(&nested).expect("嵌套目录");
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/libraries",
            Some(json!({
                "sourcePath": context.library_path,
                "organizedPath": nested,
                "watchEnabled": false,
                "autoIngestEnabled": true
            })),
            Some(&token),
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    for source_id in source_ids {
        assert_eq!(
            request(
                &context.app,
                "DELETE",
                &format!("/api/libraries/{source_id}"),
                None,
                Some(&token),
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    let (_, groups) = request(&context.app, "GET", "/api/libraries", None, Some(&token)).await;
    assert!(groups.as_array().is_some_and(Vec::is_empty));
    assert!(Path::new(organized_path).is_dir());

    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let legacy_id = uuid::Uuid::new_v4();
    let legacy_source = root.join("legacy-source");
    fs::create_dir(&legacy_source).expect("旧来源目录");
    sqlx::query("INSERT INTO libraries (id,name,path,role,created_at,updated_at) VALUES (?,?,?,'source',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)")
        .bind(legacy_id)
        .bind(legacy_source.to_string_lossy().as_ref())
        .bind(legacy_source.to_string_lossy().as_ref())
        .execute(&pool)
        .await
        .expect("插入旧来源");
    let (_, groups) = request(&context.app, "GET", "/api/libraries", None, Some(&token)).await;
    assert_eq!(groups[0]["status"], "needsTarget");
    assert_eq!(groups[0]["sources"][0]["autoIngestEnabled"], false);
    let (status, configured) = request(
        &context.app,
        "PATCH",
        &format!("/api/libraries/{legacy_id}"),
        Some(json!({
            "organizedPath": organized_path,
            "autoIngestEnabled": true
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(configured["status"], "ready");
    assert_eq!(configured["sources"][0]["autoIngestEnabled"], true);
}

#[tokio::test]
async fn auto_ingest_groups_new_files_into_one_batch_and_one_target_scan() {
    let context = test_context().await;
    fs::remove_file(Path::new(&context.library_path).join("alias.wav")).expect("移除额外 fixture");
    write_silent_wave(&Path::new(&context.library_path).join("second.wav"));
    let token = authenticate(&context.app).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接测试数据库");
    sqlx::query("UPDATE provider_settings SET enabled = 0")
        .execute(&pool)
        .await
        .expect("关闭在线服务");

    let (status, source) = request(
        &context.app,
        "POST",
        "/api/libraries",
        Some(json!({
            "sourcePath": context.library_path,
            "organizedPath": context.managed_path,
            "watchEnabled": false,
            "autoIngestEnabled": true
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let source_id = source["sources"][0]["id"].as_str().expect("来源曲库 ID");
    let managed_id = source["organizedLibraryId"].as_str().expect("整理目录 ID");

    let scan = start_scan(&context.app, &token, source_id).await;
    let scan_id = scan["id"].as_str().expect("扫描任务 ID");
    let ingest_id = wait_for_job_kind(&context.app, &token, &pool, "ingest").await;
    let ingest = wait_for_job(&context.app, &token, &ingest_id).await;
    assert_eq!(ingest["status"], "completed", "{ingest}");
    assert_eq!(ingest["parentJobId"], scan_id);
    assert_eq!(ingest["totalItems"], 2);
    assert_eq!(ingest["successItems"], 2);

    let ingest_jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE kind = 'ingest'")
        .fetch_one(&pool)
        .await
        .expect("读取接入任务数量");
    let ingest_records: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ingest_records WHERE job_id = ?")
            .bind(uuid::Uuid::parse_str(&ingest_id).expect("接入任务 UUID"))
            .fetch_one(&pool)
            .await
            .expect("读取接入记录数量");
    let target_scans: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE kind = 'scan' AND library_id = ?")
            .bind(uuid::Uuid::parse_str(managed_id).expect("整理目录 UUID"))
            .fetch_one(&pool)
            .await
            .expect("读取整理目录扫描数量");
    assert_eq!((ingest_jobs, ingest_records, target_scans), (1, 2, 1));

    let repeated = start_scan(&context.app, &token, source_id).await;
    assert_eq!(repeated["status"], "completed");
    let repeated_ingest_jobs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE kind = 'ingest'")
            .fetch_one(&pool)
            .await
            .expect("读取重复扫描后的接入任务数量");
    assert_eq!(repeated_ingest_jobs, 1);
}

#[tokio::test]
async fn auto_ingest_batch_keeps_successes_and_retries_only_failed_items() {
    let context = test_context().await;
    fs::remove_file(Path::new(&context.library_path).join("alias.wav")).expect("移除额外 fixture");
    write_silent_wave(&Path::new(&context.library_path).join("second.wav"));
    let conflict = Path::new(&context.managed_path)
        .join("未知艺术家")
        .join("未分类")
        .join("sample.wav");
    fs::create_dir_all(conflict.parent().expect("冲突目录")).expect("创建冲突目录");
    write_silent_wave(&conflict);

    let token = authenticate(&context.app).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接测试数据库");
    sqlx::query("UPDATE provider_settings SET enabled = 0")
        .execute(&pool)
        .await
        .expect("关闭在线服务");
    let (status, source) = request(
        &context.app,
        "POST",
        "/api/libraries",
        Some(json!({
            "sourcePath": context.library_path,
            "organizedPath": context.managed_path,
            "watchEnabled": false,
            "autoIngestEnabled": true
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let source_id = source["sources"][0]["id"].as_str().expect("来源曲库 ID");

    start_scan(&context.app, &token, source_id).await;
    let ingest_id = wait_for_job_kind(&context.app, &token, &pool, "ingest").await;
    let failed = wait_for_job(&context.app, &token, &ingest_id).await;
    assert_eq!(failed["status"], "completed_with_errors", "{failed}");
    assert_eq!(failed["successItems"], 1);
    assert_eq!(failed["failedItems"], 1);

    fs::remove_file(conflict).expect("移除冲突文件");
    let (status, _) = request(
        &context.app,
        "POST",
        &format!("/api/jobs/{ingest_id}/retry-failed"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let retried = wait_for_job(&context.app, &token, &ingest_id).await;
    assert_eq!(retried["status"], "completed", "{retried}");
    assert_eq!(retried["successItems"], 2);
    assert_eq!(retried["failedItems"], 0);
}

#[tokio::test]
async fn auto_ingest_is_idempotent_and_keeps_managed_copy_when_source_disappears() {
    let context = test_context().await;
    fs::remove_file(Path::new(&context.library_path).join("alias.wav")).expect("移除额外 fixture");
    let token = authenticate(&context.app).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接测试数据库");
    sqlx::query("UPDATE provider_settings SET enabled = 0")
        .execute(&pool)
        .await
        .expect("关闭在线服务");

    let (status, source) = request(
        &context.app,
        "POST",
        "/api/libraries",
        Some(json!({
            "sourcePath": context.library_path,
            "organizedPath": context.managed_path,
            "watchEnabled": false,
            "autoIngestEnabled": true
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let managed_id = source["organizedLibraryId"].as_str().expect("整理目录 ID");
    let managed_uuid = uuid::Uuid::parse_str(managed_id).expect("整理目录 UUID");
    assert_eq!(source["sources"][0]["autoIngestEnabled"], true);
    let source_id = source["sources"][0]["id"].as_str().expect("来源曲库 ID");
    let source_uuid = uuid::Uuid::parse_str(source_id).expect("来源曲库 UUID");

    let scan = start_scan(&context.app, &token, source_id).await;
    assert_eq!(scan["status"], "completed");
    let ingest_id = wait_for_job_kind(&context.app, &token, &pool, "ingest").await;
    let ingest_uuid = uuid::Uuid::parse_str(&ingest_id).expect("接入任务 UUID");
    let ingest = wait_for_job(&context.app, &token, &ingest_id).await;
    assert_eq!(ingest["status"], "completed", "{ingest}");
    assert_eq!(ingest["sourceType"], "library");
    assert_eq!(ingest["sourceId"], source_id);

    let (stage, target_relative_path): (String, String) =
        sqlx::query_as("SELECT stage, target_relative_path FROM ingest_records WHERE job_id = ?")
            .bind(ingest_uuid)
            .fetch_one(&pool)
            .await
            .expect("读取接入记录");
    assert_eq!(stage, "completed");
    let managed_file = Path::new(&context.managed_path).join(target_relative_path);
    assert!(managed_file.is_file());

    let (status, reviews) = request(
        &context.app,
        "GET",
        "/api/reviews?status=pending&kind=missing_lyrics",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reviews}");
    let review_id = reviews["items"][0]["id"]
        .as_str()
        .expect("缺失歌词待处理 ID");
    let review_uuid = uuid::Uuid::parse_str(review_id).expect("待处理 UUID");
    let (track_id, media_id): (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT track_id, media_file_id FROM review_items WHERE id = ?")
            .bind(review_uuid)
            .fetch_one(&pool)
            .await
            .expect("读取待处理关联媒体");
    let now = chrono::Utc::now();
    sqlx::query(
        r#"INSERT INTO lyrics
             (id, track_id, media_file_id, format, language, content, synced,
              coverage_percent, quality_score, storage, active, created_at, updated_at)
           VALUES (?, ?, ?, 'lrc', 'zh', '[00:01.00]测试歌词', 1, 100, 90,
             'candidate', 0, ?, ?)"#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(track_id)
    .bind(media_id)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("写入歌词候选");
    let (status, marked) = request(
        &context.app,
        "PATCH",
        &format!("/api/reviews/{review_id}"),
        Some(json!({"marked": true})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{marked}");
    assert_eq!(marked["marked"], true);
    let (status, preview) = request(
        &context.app,
        "POST",
        "/api/reviews/batch/preview",
        Some(json!({"reviewIds": [review_id], "rule": "best_lyrics"})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["eligibleItems"], 1);
    let preview_id = preview["id"].as_str().expect("批量预览 ID");
    let (status, batch) = request(
        &context.app,
        "POST",
        "/api/reviews/batch/apply",
        Some(json!({"previewId": preview_id, "confirmation": "APPLY"})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{batch}");
    let completed_batch = wait_for_job(
        &context.app,
        &token,
        batch["id"].as_str().expect("批量任务 ID"),
    )
    .await;
    assert_eq!(completed_batch["status"], "completed", "{completed_batch}");
    assert_eq!(
        fs::read_to_string(managed_file.with_extension("lrc")).expect("读取外置歌词"),
        "[00:01.00]测试歌词"
    );

    let repeated = start_scan(&context.app, &token, source_id).await;
    assert_eq!(repeated["status"], "completed");
    let ingest_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_records")
        .fetch_one(&pool)
        .await
        .expect("读取接入记录数量");
    assert_eq!(ingest_count, 1);

    fs::remove_file(Path::new(&context.library_path).join("sample.wav")).expect("移除来源文件");
    let missing_scan = start_scan(&context.app, &token, source_id).await;
    assert_eq!(missing_scan["status"], "completed");
    let availability: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM media_files WHERE library_id = ? AND available = 0), (SELECT COUNT(*) FROM media_files WHERE library_id = ? AND available = 1)",
    )
    .bind(source_uuid)
    .bind(managed_uuid)
    .fetch_one(&pool)
    .await
    .expect("读取来源与整理副本状态");
    assert_eq!(availability, (1, 1));
    let (status, catalog) = request(&context.app, "GET", "/api/catalog/tracks", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["total"], 1);

    fs::hard_link(
        &managed_file,
        Path::new(&context.library_path).join("sample.wav"),
    )
    .expect("从整理副本恢复来源");
    let recovery_scan = start_scan(&context.app, &token, source_id).await;
    assert_eq!(recovery_scan["status"], "completed");
    let ingest_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_records")
        .fetch_one(&pool)
        .await
        .expect("读取恢复后的接入记录数量");
    assert_eq!(ingest_count, 1);
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

async fn wait_for_job_kind(
    app: &Router,
    token: &str,
    pool: &sqlx::SqlitePool,
    kind: &str,
) -> String {
    for _ in 0..400 {
        let id: Option<uuid::Uuid> = sqlx::query_scalar(
            "SELECT id FROM jobs WHERE kind = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(kind)
        .fetch_optional(pool)
        .await
        .expect("读取任务");
        if let Some(id) = id {
            let id = id.to_string();
            let (status, _) =
                request(app, "GET", &format!("/api/jobs/{id}"), None, Some(token)).await;
            assert_eq!(status, StatusCode::OK);
            return id;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("未创建 {kind} 任务")
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
