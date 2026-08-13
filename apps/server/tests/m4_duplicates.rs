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
    let temp = tempfile::tempdir().expect("测试目录");
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("来源目录");
    write_wave(&source.join("晴天.wav"), 0);
    fs::copy(source.join("晴天.wav"), source.join("晴天 Copy.wav")).expect("完整 copy");
    fs::hard_link(source.join("晴天.wav"), source.join("晴天 Alias.wav")).expect("hardlink");
    write_wave(&source.join("晴天 高码率.wav"), 1);
    for (index, label) in ["Live", "Remix", "Remaster", "Instrumental"]
        .iter()
        .enumerate()
    {
        write_wave(&source.join(format!("晴天 {label}.wav")), (index + 2) as u8);
    }
    let fpcalc = temp.path().join("fpcalc-fixture.sh");
    fs::write(
        &fpcalc,
        r#"#!/bin/sh
case "$3" in
  *Live*) fp="4294967295,0,0" ;;
  *Remix*) fp="0,4294967295,0" ;;
  *Remaster*) fp="0,0,4294967295" ;;
  *Instrumental*) fp="4294967295,4294967295,0" ;;
  *) fp="1,2,3" ;;
esac
printf '{"duration":1.0,"fingerprint":"%s"}\n' "$fp"
exit 3
"#,
    )
    .expect("fpcalc fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fpcalc, fs::Permissions::from_mode(0o755)).expect("可执行权限");
    }
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
        analysis: AnalysisConfig {
            workers: 1,
            fpcalc_path: fpcalc.to_string_lossy().into_owned(),
            fingerprint_threshold: 0.88,
        },
        ai: AiConfig::default(),
        playback: PlaybackConfig::default(),
    };
    Context {
        app: build_app(&config).await.expect("服务"),
        _temp: temp,
        database,
        source,
    }
}

#[tokio::test]
async fn hash_fingerprint_and_quality_groups_are_separate_and_safe() {
    let context = context().await;
    let token = authenticate_and_scan(&context).await;
    let pool = db::connect(&context.database).await.expect("数据库");
    let original_track: Uuid = sqlx::query_scalar(
        "SELECT mf.track_id FROM media_files mf
         JOIN libraries l ON l.id = mf.library_id
         WHERE l.role = 'managed' AND mf.relative_path LIKE '%晴天.wav'",
    )
    .fetch_one(&pool)
    .await
    .expect("原曲");
    sqlx::query("UPDATE media_files SET track_id = ?, codec = 'flac', bitrate = 1000000, bit_depth = 24, sample_rate = 96000 WHERE id IN (SELECT mf.id FROM media_files mf JOIN libraries l ON l.id = mf.library_id WHERE l.role = 'managed' AND mf.relative_path LIKE '%晴天 高码率.wav')")
        .bind(original_track).execute(&pool).await.expect("构造质量变体");
    sqlx::query("UPDATE media_files SET codec = 'mp3', bitrate = 128000, bit_depth = 16, sample_rate = 44100 WHERE id IN (SELECT mf.id FROM media_files mf JOIN libraries l ON l.id = mf.library_id WHERE l.role = 'managed' AND mf.relative_path LIKE '%晴天.wav')")
        .execute(&pool).await.expect("构造低码率");

    let (status, job) = request(
        &context.app,
        "POST",
        "/api/duplicates/analyze",
        Some(json!({"mediaIds":[],"calculateHash":true,"calculateFingerprint":true})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let finished = wait_job(&context.app, &token, job["id"].as_str().expect("任务 ID")).await;
    assert_eq!(finished["status"], "completed");
    let analysis_counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT COUNT(*) FROM media_files mf JOIN libraries l ON l.id = mf.library_id WHERE l.role = 'managed' AND mf.available = 1),
           (SELECT COUNT(*) FROM audio_hashes ah JOIN media_files mf ON mf.id = ah.media_file_id JOIN libraries l ON l.id = mf.library_id WHERE l.role = 'managed'),
           (SELECT COUNT(*) FROM audio_fingerprints af JOIN media_files mf ON mf.id = af.media_file_id JOIN libraries l ON l.id = mf.library_id WHERE l.role = 'managed')",
    )
    .fetch_one(&pool)
    .await
    .expect("读取独立分析记录");
    assert_eq!(analysis_counts.0, analysis_counts.1);
    assert_eq!(analysis_counts.0, analysis_counts.2);
    let (status, groups) = request(
        &context.app,
        "GET",
        "/api/duplicates/groups",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let groups = groups.as_array().expect("重复组");
    for kind in [
        "hardlink_alias",
        "binary_exact",
        "audio_duplicate",
        "quality_variant",
    ] {
        assert!(
            groups.iter().any(|group| group["kind"] == kind),
            "缺少 {kind}"
        );
    }
    let alias = groups
        .iter()
        .find(|group| group["kind"] == "hardlink_alias")
        .expect("alias");
    assert_eq!(alias["reclaimableBytes"], 0);
    let quality = groups
        .iter()
        .find(|group| group["kind"] == "quality_variant")
        .expect("quality");
    let scores: Vec<_> = quality["members"]
        .as_array()
        .expect("成员")
        .iter()
        .filter_map(|item| item["qualityScore"].as_i64())
        .collect();
    assert!(scores.iter().max() > scores.iter().min());

    let candidate = groups
        .iter()
        .find(|group| group["kind"] == "binary_exact")
        .expect("exact")["members"][1]["mediaFileId"]
        .as_str()
        .expect("媒体 ID");
    let (status, preview) = request(
        &context.app,
        "POST",
        "/api/trash/preview",
        Some(json!({"mediaIds":[candidate]})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["status"], "previewed");
    assert!(context.source.join("晴天 Copy.wav").exists());

    let (_, ai) = request(&context.app, "GET", "/api/ai/status", None, Some(&token)).await;
    assert_eq!(ai["enabled"], false);
    assert_eq!(ai["uploadsAudio"], false);
    let (status, _) = request(
        &context.app,
        "POST",
        "/api/ai/duplicates/explain",
        Some(json!({"groupId": alias["id"], "confirmation":"SEND_METADATA"})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

async fn authenticate_and_scan(context: &Context) -> String {
    let credentials = json!({"username":"admin","password":"pass123"});
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
    let (_, library) = request(&context.app,"POST","/api/libraries",Some(json!({"sourcePath":context.source,"organizedPath":organized,"watchEnabled":false,"autoIngestEnabled":true})),Some(&token)).await;
    let (_, job) = request(
        &context.app,
        "POST",
        &format!(
            "/api/libraries/{}/scan",
            library["sources"][0]["id"].as_str().expect("来源")
        ),
        None,
        Some(&token),
    )
    .await;
    wait_job(&context.app, &token, job["id"].as_str().expect("任务")).await;
    wait_for_ingest_job(&context.app, &token).await;
    token
}

async fn wait_for_ingest_job(app: &Router, token: &str) {
    for _ in 0..2_000 {
        let (_, jobs) = request(app, "GET", "/api/jobs?limit=100", None, Some(token)).await;
        let ingest_jobs: Vec<_> = jobs
            .as_array()
            .into_iter()
            .flatten()
            .filter(|job| job["kind"] == "ingest")
            .collect();
        if !ingest_jobs.is_empty()
            && ingest_jobs.iter().all(|job| {
                matches!(
                    job["status"].as_str(),
                    Some("completed" | "completed_with_errors" | "failed")
                )
            })
        {
            assert!(
                ingest_jobs.iter().all(|job| job["status"] != "failed"),
                "存在失败的接入任务：{ingest_jobs:?}"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("接入任务未完成")
}
async fn wait_job(app: &Router, token: &str, id: &str) -> Value {
    for _ in 0..400 {
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
        .expect("body")
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
fn write_wave(path: &Path, fill: u8) {
    let samples = [fill; 160];
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
    fs::write(path, bytes).expect("WAV");
}
