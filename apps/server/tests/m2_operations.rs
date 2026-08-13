use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use lofty::{file::TaggedFileExt, prelude::Accessor, probe::Probe};
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

struct TestContext {
    app: Router,
    _temp: TempDir,
    database_path: String,
    source: PathBuf,
    managed: PathBuf,
}

async fn context() -> TestContext {
    let temp = tempfile::tempdir().expect("创建测试目录");
    let source = temp.path().join("source");
    let managed = temp.path().join("managed");
    fs::create_dir_all(&source).expect("创建源曲库");
    fs::create_dir_all(&managed).expect("创建整理曲库");
    write_silent_wave(&source.join("03 - 晴天.wav"));
    let database_path = temp
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
    let app = build_app(&config).await.expect("构建服务");
    let pool = db::connect(&database_path).await.expect("连接测试数据库");
    sqlx::query("UPDATE provider_settings SET enabled = 0")
        .execute(&pool)
        .await
        .expect("关闭在线服务");
    TestContext {
        app,
        _temp: temp,
        database_path,
        source,
        managed,
    }
}

#[tokio::test]
async fn tag_and_hardlink_operations_require_preview_and_support_undo() {
    let context = context().await;
    let token = authenticate(&context.app).await;
    let (source_library, managed_library) =
        create_library(&context.app, &token, &context.source, &context.managed).await;
    let scan = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{source_library}/scan"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(scan.0, StatusCode::ACCEPTED);
    wait_for_job(
        &context.app,
        &token,
        scan.1["id"].as_str().expect("任务 ID"),
    )
    .await;
    wait_for_ingest_job(&context.app, &token).await;

    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let (media_uuid, _track_uuid, managed_media_path) = wait_for_managed_media(&pool).await;
    let media_id = media_uuid.to_string();

    let (status, tag_preview) = request(
        &context.app,
        "POST",
        "/api/tags/preview",
        Some(json!({
            "mediaIds": [media_id],
        "set": {
            "title": "晴天", "artists": ["周杰倫"], "album": "葉惠美", "trackNo": 3,
            "coverDataBase64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        },
            "transforms": [{ "kind": "traditionalToSimplified", "fields": ["artists", "album"] }]
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tag_preview["status"], "previewed");
    assert!(
        tag_preview["items"][0]["diffs"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    let operation_id = tag_preview["id"].as_str().expect("操作 ID");
    let (status, _) = request(
        &context.app,
        "POST",
        "/api/tags/apply",
        Some(json!({
            "operationId": operation_id, "confirmation": "yes"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, applied) = request(
        &context.app,
        "POST",
        "/api/tags/apply",
        Some(json!({
            "operationId": operation_id, "confirmation": "APPLY"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(applied["status"], "completed", "{applied}");
    let (status, tag_job) = request(
        &context.app,
        "GET",
        &format!("/api/jobs/{operation_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tag_job["kind"], "tag_edit");
    assert_eq!(tag_job["status"], "completed");
    let tagged = Probe::open(&managed_media_path)
        .and_then(|probe| probe.read())
        .expect("读取写入后的 Tag");
    let tag = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .expect("主 Tag");
    assert_eq!(tag.title().as_deref(), Some("晴天"));
    assert_eq!(tag.artist().as_deref(), Some("周杰伦"));
    assert_eq!(tag.pictures().len(), 1);
    let (status, rescan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{managed_library}/scan"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    wait_for_job(
        &context.app,
        &token,
        rescan["id"].as_str().expect("Tag 后重扫任务 ID"),
    )
    .await;
    let indexed_title: String = sqlx::query_scalar(
        "SELECT t.title FROM media_files mf JOIN tracks t ON t.id = mf.track_id WHERE mf.id = ?",
    )
    .bind(uuid::Uuid::parse_str(&media_id).expect("媒体 UUID"))
    .fetch_one(&pool)
    .await
    .expect("读取重扫后的标题");
    assert_eq!(indexed_title, "晴天");
    let artwork_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM artworks WHERE media_file_id = ?")
            .bind(media_uuid)
            .fetch_one(&pool)
            .await
            .expect("读取封面元信息");
    assert_eq!(artwork_count, 1);
    let current_track_uuid: uuid::Uuid =
        sqlx::query_scalar("SELECT track_id FROM media_files WHERE id = ?")
            .bind(media_uuid)
            .fetch_one(&pool)
            .await
            .expect("读取重扫后的曲目 ID");
    let (status, history) = request(
        &context.app,
        "GET",
        &format!("/api/tracks/{current_track_uuid}/operations"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history[0]["operationId"], operation_id);
    assert_eq!(history[0]["kind"], "tag_edit");
    assert_eq!(history[0]["status"], "success");
    let normalized_text: String =
        sqlx::query_scalar("SELECT normalized_text FROM track_search WHERE track_id = ?")
            .bind(current_track_uuid)
            .fetch_one(&pool)
            .await
            .expect("读取拼音辅助索引");
    assert!(
        normalized_text.split_whitespace().any(|term| term == "zjl"),
        "拼音首字母索引缺失：{normalized_text}"
    );
    for search in ["zjl", "zhoujielun", "周杰倫"] {
        let encoded = url::form_urlencoded::byte_serialize(search.as_bytes()).collect::<String>();
        let (status, tracks) = request(
            &context.app,
            "GET",
            &format!("/api/tracks?search={encoded}"),
            None,
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "搜索 {search} 失败");
        assert_eq!(tracks["total"], 1, "搜索 {search} 未命中");
    }

    let (status, organize_preview) = request(
        &context.app,
        "POST",
        "/api/organizer/preview",
        Some(json!({
            "mediaIds": [media_id], "targetLibraryId": managed_library,
            "template": "{artist_initial}/{artist}/{album}/{track:02} - {title}.{ext}", "crossPlatformSafe": true
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        organize_preview["items"][0]["preflight"]["sameFilesystem"],
        true
    );
    let organizer_id = organize_preview["id"].as_str().expect("整理操作 ID");
    let (status, organized) = request(
        &context.app,
        "POST",
        "/api/organizer/apply",
        Some(json!({
            "operationId": organizer_id, "confirmation": "APPLY"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(organized["status"], "completed");
    let (_, organizer_job) = request(
        &context.app,
        "GET",
        &format!("/api/jobs/{organizer_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(organizer_job["kind"], "organize");
    assert_eq!(organizer_job["status"], "completed");
    let target = PathBuf::from(
        organized["items"][0]["targetPath"]
            .as_str()
            .expect("目标路径"),
    );
    assert!(target.to_string_lossy().contains("/Z/周杰伦/"));
    assert!(target.is_file());
    assert!(same_physical(&managed_media_path, &target));

    let managed_library_id = uuid::Uuid::parse_str(&managed_library).expect("目标曲库 UUID");
    let initial_target_scan: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM jobs WHERE kind = 'scan' AND library_id = ? ORDER BY rowid DESC LIMIT 1",
    )
    .bind(managed_library_id)
    .fetch_one(&pool)
    .await
    .expect("整理后的目标曲库扫描任务");
    wait_for_job(&context.app, &token, &initial_target_scan.to_string()).await;

    // 固定制造一个正在运行的同曲库扫描，验证撤销触发的后续扫描不会被吞掉。
    let blocking_scan_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO jobs (id, kind, status, library_id, created_at, started_at, updated_at) VALUES (?, 'scan', 'running', ?, ?, ?, ?)",
    )
    .bind(blocking_scan_id)
    .bind(managed_library_id)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("创建阻塞扫描任务");

    let (status, undone) = request(
        &context.app,
        "POST",
        "/api/organizer/undo",
        Some(json!({
            "operationId": organizer_id, "confirmation": "UNDO"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(undone["status"], "rolled_back");
    assert!(!target.exists());
    assert!(context.source.join("03 - 晴天.wav").is_file());
    let rescan_job_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM jobs WHERE kind = 'scan' AND library_id = ? AND status = 'queued' ORDER BY rowid DESC LIMIT 1",
    )
    .bind(managed_library_id)
    .fetch_one(&pool)
    .await
    .expect("撤销后排队的目标曲库重扫任务");
    let finished_at = chrono::Utc::now();
    sqlx::query(
        "UPDATE jobs SET status = 'completed', finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(finished_at)
    .bind(finished_at)
    .bind(blocking_scan_id)
    .execute(&pool)
    .await
    .expect("释放阻塞扫描任务");
    wait_for_job(&context.app, &token, &rescan_job_id.to_string()).await;
    let managed_media_count: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN available = 0 THEN 1 ELSE 0 END), 0) FROM media_files WHERE library_id = ?",
    )
    .bind(managed_library_id)
    .fetch_one(&pool)
    .await
    .expect("读取撤销后的目标索引");
    assert_eq!(managed_media_count.0 - managed_media_count.1, 1);
    assert!(managed_media_count.1 <= 1);

    fs::create_dir_all(target.parent().expect("冲突目标父目录")).expect("创建冲突目录");
    fs::write(&target, b"different file").expect("创建冲突文件");
    let (_, conflict_preview) = request(
        &context.app,
        "POST",
        "/api/organizer/preview",
        Some(json!({
            "mediaIds": [media_id], "targetLibraryId": managed_library,
            "template": "{artist_initial}/{artist}/{album}/{track:02} - {title}.{ext}", "crossPlatformSafe": true
        })),
        Some(&token),
    )
    .await;
    assert_eq!(
        conflict_preview["items"][0]["preflight"]["pathConflict"],
        true
    );
    let conflict_id = conflict_preview["id"].as_str().expect("冲突操作 ID");
    let (_, conflict_apply) = request(
        &context.app,
        "POST",
        "/api/organizer/apply",
        Some(json!({ "operationId": conflict_id, "confirmation": "APPLY" })),
        Some(&token),
    )
    .await;
    assert_eq!(conflict_apply["status"], "completed_with_errors");
    assert_eq!(fs::read(&target).expect("读取冲突文件"), b"different file");

    let (status, tag_undo) = request(
        &context.app,
        "POST",
        "/api/tags/undo",
        Some(json!({ "operationId": operation_id, "confirmation": "UNDO" })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tag_undo["status"], "rolled_back");
    let restored = Probe::open(&managed_media_path)
        .and_then(|probe| probe.read())
        .expect("读取撤销后的 Tag");
    assert!(
        restored
            .primary_tag()
            .or_else(|| restored.first_tag())
            .is_none_or(|tag| tag.title().is_none())
    );
}

#[tokio::test]
async fn trash_requires_preview_and_restores_without_overwrite() {
    let context = context().await;
    let token = authenticate(&context.app).await;
    let (library_id, _) =
        create_library(&context.app, &token, &context.source, &context.managed).await;
    let (_, scan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(&token),
    )
    .await;
    wait_for_job(&context.app, &token, scan["id"].as_str().expect("任务 ID")).await;
    wait_for_ingest_job(&context.app, &token).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let (media_uuid, _, managed_media_path) = wait_for_managed_media(&pool).await;
    let media_id = media_uuid.to_string();
    let (status, preview) = request(
        &context.app,
        "POST",
        "/api/trash/preview",
        Some(json!({ "mediaIds": [media_id] })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let operation_id = preview["id"].as_str().expect("回收站操作 ID");
    let trash_path = PathBuf::from(
        preview["items"][0]["targetPath"]
            .as_str()
            .expect("回收站路径"),
    );
    let source_path = context.source.join("03 - 晴天.wav");
    let (status, _) = request(
        &context.app,
        "POST",
        "/api/trash/apply",
        Some(json!({ "operationId": operation_id, "confirmation": "TRASH" })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(source_path.exists());
    assert!(!managed_media_path.exists());
    assert!(trash_path.exists());
    let (_, trash_job) = request(
        &context.app,
        "GET",
        &format!("/api/jobs/{operation_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(trash_job["kind"], "trash");
    assert_eq!(trash_job["status"], "completed");
    let (status, restored) = request(
        &context.app,
        "POST",
        "/api/trash/restore",
        Some(json!({ "operationId": operation_id, "confirmation": "RESTORE" })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restored["status"], "rolled_back");
    assert!(source_path.exists());
    assert!(managed_media_path.exists());
    assert!(!trash_path.exists());
}

#[tokio::test]
async fn permanent_trash_purge_requires_preview_and_exact_confirmation() {
    let context = context().await;
    let token = authenticate(&context.app).await;
    let (library_id, _) =
        create_library(&context.app, &token, &context.source, &context.managed).await;
    let (_, scan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(&token),
    )
    .await;
    wait_for_job(&context.app, &token, scan["id"].as_str().expect("任务 ID")).await;
    wait_for_ingest_job(&context.app, &token).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let (media_id, _, managed_media_path) = wait_for_managed_media(&pool).await;
    let (status, trash_preview) = request(
        &context.app,
        "POST",
        "/api/trash/preview",
        Some(json!({ "mediaIds": [media_id] })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{trash_preview}");
    let operation_id = trash_preview["id"].as_str().expect("回收站操作 ID");
    let trash_path = PathBuf::from(
        trash_preview["items"][0]["targetPath"]
            .as_str()
            .expect("回收站路径"),
    );
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/trash/apply",
            Some(json!({ "operationId": operation_id, "confirmation": "TRASH" })),
            Some(&token),
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, purge_preview) = request(
        &context.app,
        "POST",
        "/api/trash/purge/preview",
        Some(json!({ "trashOperationId": operation_id })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(purge_preview["status"], "previewed");
    assert_eq!(purge_preview["items"][0]["status"], "previewed");
    let purge_id = purge_preview["id"].as_str().expect("永久清理 ID");
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/trash/purge/apply",
            Some(json!({ "purgeId": purge_id, "confirmation": "DELETE" })),
            Some(&token),
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, purged) = request(
        &context.app,
        "POST",
        "/api/trash/purge/apply",
        Some(json!({
            "purgeId": purge_id,
            "confirmation": "PURGE_PERMANENTLY"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(purged["status"], "completed");
    assert!(!trash_path.exists());
    assert!(context.source.join("03 - 晴天.wav").exists());
    assert!(!managed_media_path.exists());
    let (status, entries) = request(&context.app, "GET", "/api/trash", None, Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entries[0]["purgeStatus"], "completed");
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/trash/restore",
            Some(json!({ "operationId": operation_id, "confirmation": "RESTORE" })),
            Some(&token),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[cfg(unix)]
#[tokio::test]
async fn permanent_trash_purge_rejects_symlinks() {
    use std::os::unix::fs::symlink;

    let context = context().await;
    let token = authenticate(&context.app).await;
    let (library_id, _) =
        create_library(&context.app, &token, &context.source, &context.managed).await;
    let (_, scan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(&token),
    )
    .await;
    wait_for_job(&context.app, &token, scan["id"].as_str().expect("任务 ID")).await;
    wait_for_ingest_job(&context.app, &token).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let (media_id, _, _) = wait_for_managed_media(&pool).await;
    let (status, preview) = request(
        &context.app,
        "POST",
        "/api/trash/preview",
        Some(json!({ "mediaIds": [media_id] })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let operation_id = preview["id"].as_str().expect("回收站操作 ID");
    let trash_path = PathBuf::from(preview["items"][0]["targetPath"].as_str().expect("路径"));
    assert_eq!(
        request(
            &context.app,
            "POST",
            "/api/trash/apply",
            Some(json!({ "operationId": operation_id, "confirmation": "TRASH" })),
            Some(&token),
        )
        .await
        .0,
        StatusCode::OK
    );
    fs::remove_file(&trash_path).expect("移除测试回收站文件");
    let sentinel = context.managed.join("sentinel.wav");
    write_silent_wave(&sentinel);
    symlink(&sentinel, &trash_path).expect("创建测试符号链接");
    let (status, purge) = request(
        &context.app,
        "POST",
        "/api/trash/purge/preview",
        Some(json!({ "trashOperationId": operation_id })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(purge["items"][0]["status"], "failed");
    assert!(
        purge["items"][0]["errorMessage"]
            .as_str()
            .expect("错误信息")
            .contains("符号链接")
    );
    let (status, applied) = request(
        &context.app,
        "POST",
        "/api/trash/purge/apply",
        Some(json!({
            "purgeId": purge["id"],
            "confirmation": "PURGE_PERMANENTLY"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(applied["status"], "completed_with_errors");
    assert!(sentinel.exists());
}

async fn create_library(
    app: &Router,
    token: &str,
    source_path: &Path,
    organized_path: &Path,
) -> (String, String) {
    let (status, body) = request(
        app,
        "POST",
        "/api/libraries",
        Some(json!({
            "sourcePath": source_path, "organizedPath": organized_path,
            "watchEnabled": false, "autoIngestEnabled": true
        })),
        Some(token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    (
        body["sources"][0]["id"]
            .as_str()
            .expect("来源 ID")
            .to_owned(),
        body["organizedLibraryId"]
            .as_str()
            .expect("整理目录 ID")
            .to_owned(),
    )
}

async fn wait_for_managed_media(pool: &sqlx::SqlitePool) -> (uuid::Uuid, uuid::Uuid, PathBuf) {
    for _ in 0..200 {
        if let Some((media_id, track_id, library_path, relative_path)) =
            sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, String)>(
                "SELECT mf.id, mf.track_id, l.path, mf.relative_path
                 FROM media_files mf
                 JOIN libraries l ON l.id = mf.library_id
                 WHERE l.role = 'managed' AND mf.available = 1
                 ORDER BY mf.created_at DESC
                 LIMIT 1",
            )
            .fetch_optional(pool)
            .await
            .expect("查询整理媒体")
        {
            return (
                media_id,
                track_id,
                PathBuf::from(library_path).join(relative_path),
            );
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("整理媒体未生成")
}

async fn authenticate(app: &Router) -> String {
    let credentials = json!({"username": "admin", "password": "pass123"});
    assert_eq!(
        request(
            app,
            "POST",
            "/api/auth/setup",
            Some(credentials.clone()),
            None
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (status, body) = request(app, "POST", "/api/auth/login", Some(credentials), None).await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().expect("JWT").to_owned()
}

async fn wait_for_job(app: &Router, token: &str, id: &str) {
    for _ in 0..200 {
        let (_, body) = request(app, "GET", &format!("/api/jobs/{id}"), None, Some(token)).await;
        if matches!(
            body["status"].as_str(),
            Some("completed" | "completed_with_errors" | "failed")
        ) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("任务未结束")
}

async fn wait_for_ingest_job(app: &Router, token: &str) {
    for _ in 0..200 {
        let (_, jobs) = request(app, "GET", "/api/jobs?limit=100", None, Some(token)).await;
        let Some(job) = jobs
            .as_array()
            .and_then(|items| items.iter().find(|job| job["kind"] == "ingest"))
        else {
            tokio::time::sleep(Duration::from_millis(25)).await;
            continue;
        };
        if matches!(
            job["status"].as_str(),
            Some("completed" | "completed_with_errors" | "failed")
        ) {
            assert_ne!(job["status"], "failed", "接入任务失败：{job}");
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("接入任务未结束")
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let payload = if let Some(body) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(body.to_string())
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
    fs::write(path, bytes).expect("写入 WAV")
}

#[cfg(unix)]
fn same_physical(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let left = left.metadata().expect("源 metadata");
    let right = right.metadata().expect("目标 metadata");
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_physical(_left: &Path, _right: &Path) -> bool {
    true
}
