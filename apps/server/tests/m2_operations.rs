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
    TestContext {
        app: build_app(&config).await.expect("构建服务"),
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
    let source_library = create_library(
        &context.app,
        &token,
        "来源",
        &context.source,
        "source",
        true,
    )
    .await;
    let managed_library = create_library(
        &context.app,
        &token,
        "已整理",
        &context.managed,
        "managed",
        true,
    )
    .await;
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

    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let (media_uuid, _track_uuid) = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid)>(
        "SELECT id, track_id FROM media_files LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("媒体与曲目 ID");
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
    let tagged = Probe::open(context.source.join("03 - 晴天.wav"))
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
        &format!("/api/libraries/{source_library}/scan"),
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
        sqlx::query_scalar("SELECT normalized_text FROM track_search LIMIT 1")
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
    assert!(same_physical(
        &context.source.join("03 - 晴天.wav"),
        &target
    ));

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
    let (_, jobs) = request(&context.app, "GET", "/api/jobs", None, Some(&token)).await;
    let rescan_job = jobs
        .as_array()
        .and_then(|items| {
            items.iter().find(|job| {
                job["kind"] == "scan" && job["libraryId"].as_str() == Some(&managed_library)
            })
        })
        .expect("撤销后目标曲库重扫任务");
    wait_for_job(
        &context.app,
        &token,
        rescan_job["id"].as_str().expect("撤销后重扫任务 ID"),
    )
    .await;
    let managed_media_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE library_id = ?")
            .bind(uuid::Uuid::parse_str(&managed_library).expect("目标曲库 UUID"))
            .fetch_one(&pool)
            .await
            .expect("读取撤销后的目标索引");
    assert_eq!(managed_media_count, 0);

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
    let restored = Probe::open(context.source.join("03 - 晴天.wav"))
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
    let library_id = create_library(
        &context.app,
        &token,
        "可写来源",
        &context.source,
        "source",
        true,
    )
    .await;
    let (_, scan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(&token),
    )
    .await;
    wait_for_job(&context.app, &token, scan["id"].as_str().expect("任务 ID")).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let media_id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM media_files LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("媒体 ID")
        .to_string();
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
    assert!(!source_path.exists());
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
    assert!(!trash_path.exists());
}

#[tokio::test]
async fn permanent_trash_purge_requires_preview_and_exact_confirmation() {
    let context = context().await;
    let token = authenticate(&context.app).await;
    let library_id = create_library(
        &context.app,
        &token,
        "永久清理来源",
        &context.source,
        "source",
        true,
    )
    .await;
    let (_, scan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(&token),
    )
    .await;
    wait_for_job(&context.app, &token, scan["id"].as_str().expect("任务 ID")).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let media_id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM media_files LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("媒体 ID");
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
    assert!(!context.source.join("03 - 晴天.wav").exists());
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
    let library_id = create_library(
        &context.app,
        &token,
        "符号链接来源",
        &context.source,
        "source",
        true,
    )
    .await;
    let (_, scan) = request(
        &context.app,
        "POST",
        &format!("/api/libraries/{library_id}/scan"),
        None,
        Some(&token),
    )
    .await;
    wait_for_job(&context.app, &token, scan["id"].as_str().expect("任务 ID")).await;
    let pool = db::connect(&context.database_path)
        .await
        .expect("连接数据库");
    let media_id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM media_files LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("媒体 ID");
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
    name: &str,
    path: &Path,
    role: &str,
    writable: bool,
) -> String {
    let (status, body) = request(
        app,
        "POST",
        "/api/libraries",
        Some(json!({
            "name": name, "path": path, "role": role, "scanEnabled": true,
            "watchEnabled": false, "writable": writable
        })),
        Some(token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body["id"].as_str().expect("曲库 ID").to_owned()
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
