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
    let cache = temp.path().join("transcode");
    fs::create_dir(&source).expect("来源目录");
    write_wave(&source.join("夜曲.wav"));
    let ffmpeg = temp.path().join("ffmpeg-fixture.sh");
    fs::write(
        &ffmpeg,
        r#"#!/bin/sh
input=""
output=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-i" ]; then
    shift
    input="$1"
  fi
  output="$1"
  shift
done
cp "$input" "$output"
"#,
    )
    .expect("FFmpeg fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).expect("可执行权限");
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
        analysis: AnalysisConfig::default(),
        ai: AiConfig::default(),
        playback: PlaybackConfig {
            ffmpeg_path: ffmpeg.to_string_lossy().into_owned(),
            transcode_workers: 1,
            cache_dir: cache.to_string_lossy().into_owned(),
            cache_max_bytes: 16 * 1024 * 1024,
        },
    };
    Context {
        app: build_app(&config).await.expect("服务"),
        _temp: temp,
        database,
        source,
    }
}

#[tokio::test]
async fn web_stream_range_transcode_history_favorite_and_playlist_work() {
    let context = context().await;
    let token = authenticate_and_scan(&context).await;
    let pool = db::connect(&context.database).await.expect("数据库");
    let (track_id, media_id): (Uuid, Uuid) =
        sqlx::query_as("SELECT track_id, id FROM media_files LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("媒体");

    let response = raw(
        &context.app,
        "GET",
        &format!("/api/media/{media_id}/stream"),
        None,
        Some(&token),
        Some((header::RANGE, "bytes=0-9")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 0-9/204");
    assert_eq!(body_bytes(response).await.len(), 10);

    let (_, ticket) = json_request(
        &context.app,
        "GET",
        &format!("/api/media/{media_id}/play-token"),
        None,
        Some(&token),
    )
    .await;
    let scoped = ticket["token"].as_str().expect("播放令牌");
    let response = raw(
        &context.app,
        "GET",
        &format!("/api/media/{media_id}/stream?token={scoped}"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await.len(), 204);

    for _ in 0..2 {
        let response = raw(
            &context.app,
            "GET",
            &format!("/api/media/{media_id}/transcode?profile=opus-192"),
            None,
            Some(&token),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_bytes(response).await.len(), 204);
    }
    let cache_entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transcode_cache")
        .fetch_one(&pool)
        .await
        .expect("缓存计数");
    assert_eq!(cache_entries, 1);

    assert_eq!(
        json_request(
            &context.app,
            "POST",
            "/api/playback/scrobble",
            Some(json!({"trackId":track_id,"mediaFileId":media_id,"completed":true})),
            Some(&token),
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, history) = json_request(
        &context.app,
        "GET",
        "/api/playback/history",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(history[0]["trackId"], track_id.to_string());
    let (_, dashboard) = json_request(
        &context.app,
        "GET",
        "/api/dashboard/stats",
        None,
        Some(&token),
    )
    .await;
    assert_eq!(
        dashboard["recentPlayed"][0]["trackId"],
        track_id.to_string()
    );
    assert_eq!(
        json_request(
            &context.app,
            "PUT",
            &format!("/api/favorites/{track_id}"),
            None,
            Some(&token),
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, favorites) =
        json_request(&context.app, "GET", "/api/favorites", None, Some(&token)).await;
    assert_eq!(favorites[0], track_id.to_string());

    let (status, playlist) = json_request(
        &context.app,
        "POST",
        "/api/playlists",
        Some(json!({"name":"夜航","trackIds":[track_id]})),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(playlist["songCount"], 1);
}

#[tokio::test]
async fn symfonium_style_json_xml_browse_search_and_media_contracts_work() {
    let context = context().await;
    let _token = authenticate_and_scan(&context).await;
    let pool = db::connect(&context.database).await.expect("数据库");
    let (track_id, media_id): (Uuid, Uuid) =
        sqlx::query_as("SELECT track_id, id FROM media_files LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("曲目");
    let (_, preview) = json_request(
        &context.app,
        "POST",
        "/api/tags/preview",
        Some(json!({
            "mediaIds": [media_id],
            "set": {
                "coverDataBase64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            }
        })),
        Some(&_token),
    )
    .await;
    let (status, _) = json_request(
        &context.app,
        "POST",
        "/api/tags/apply",
        Some(json!({"operationId": preview["id"], "confirmation": "APPLY"})),
        Some(&_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let response = raw(
        &context.app,
        "GET",
        &format!("/api/artwork/{media_id}"),
        None,
        Some(&_token),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    let salt = "symfonium-fixture";
    let token = format!("{:x}", md5::compute(format!("pass123{salt}").as_bytes()));
    let auth = format!("u=admin&t={token}&s={salt}&v=1.16.1&c=Symfonium");

    let response = raw(
        &context.app,
        "GET",
        &format!("/rest/ping.view?{auth}"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .expect("类型")
            .starts_with("text/xml")
    );
    let xml = String::from_utf8(body_bytes(response).await.to_vec()).expect("XML");
    assert!(xml.contains("openSubsonic=\"true\""));

    for method in [
        "ping",
        "getLicense",
        "getOpenSubsonicExtensions",
        "getMusicFolders",
        "getIndexes",
        "getArtists",
        "getAlbumList2&type=newest&size=10",
        "getRandomSongs&size=1",
        "search3&query=%E5%A4%9C%E6%9B%B2",
        "search3&query=yequ",
    ] {
        let (name, extra) = method
            .split_once('&')
            .map_or((method, ""), |(name, extra)| (name, extra));
        let suffix = if extra.is_empty() {
            String::new()
        } else {
            format!("&{extra}")
        };
        let response = raw(
            &context.app,
            "GET",
            &format!("/rest/{name}.view?{auth}&f=json{suffix}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{name}");
        let body: Value = serde_json::from_slice(&body_bytes(response).await).expect("JSON");
        assert_eq!(body["subsonic-response"]["status"], "ok", "{name}");
        if method == "search3&query=yequ" {
            assert_eq!(
                body["subsonic-response"]["searchResult3"]["song"][0]["title"],
                "夜曲"
            );
        }
    }

    let song_uri = format!("/rest/getSong.view?{auth}&f=json&id=tr:{track_id}");
    let response = raw(&context.app, "GET", &song_uri, None, None, None).await;
    let body: Value = serde_json::from_slice(&body_bytes(response).await).expect("Song JSON");
    assert_eq!(
        body["subsonic-response"]["song"]["id"],
        track_id.to_string()
    );

    let response = raw(
        &context.app,
        "GET",
        &format!("/rest/stream.view?{auth}&id=tr:{track_id}"),
        None,
        None,
        Some((header::RANGE, "bytes=-12")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(body_bytes(response).await.len(), 12);

    let response = raw(
        &context.app,
        "GET",
        &format!("/rest/getCoverArt.view?{auth}&id=mf:{media_id}"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert!(!body_bytes(response).await.is_empty());

    for method in [
        format!("star.view?{auth}&f=json&id=tr:{track_id}"),
        format!("getStarred2.view?{auth}&f=json"),
        format!("getLyricsBySongId.view?{auth}&f=json&id=tr:{track_id}"),
    ] {
        let response = raw(
            &context.app,
            "GET",
            &format!("/rest/{method}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{method}");
        let body: Value = serde_json::from_slice(&body_bytes(response).await).expect("扩展 JSON");
        assert_eq!(body["subsonic-response"]["status"], "ok", "{method}");
    }

    let response = raw(
        &context.app,
        "GET",
        &format!("/rest/createPlaylist.view?{auth}&f=json&name=Symfonium&songId=tr:{track_id}"),
        None,
        None,
        None,
    )
    .await;
    let body: Value = serde_json::from_slice(&body_bytes(response).await).expect("歌单 JSON");
    let playlist_id = body["subsonic-response"]["playlist"]["id"]
        .as_str()
        .expect("歌单 ID");
    for method in [
        format!("getPlaylist.view?{auth}&f=json&id={playlist_id}"),
        format!("deletePlaylist.view?{auth}&f=json&id={playlist_id}"),
    ] {
        let response = raw(
            &context.app,
            "GET",
            &format!("/rest/{method}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{method}");
        let body: Value = serde_json::from_slice(&body_bytes(response).await).expect("歌单响应");
        assert_eq!(body["subsonic-response"]["status"], "ok", "{method}");
    }

    let response = raw(
        &context.app,
        "GET",
        "/rest/ping.view?u=admin&t=wrong&s=salt&f=json",
        None,
        None,
        None,
    )
    .await;
    let body: Value = serde_json::from_slice(&body_bytes(response).await).expect("失败 JSON");
    assert_eq!(body["subsonic-response"]["status"], "failed");
    assert_eq!(body["subsonic-response"]["error"]["code"], 40);
}

async fn authenticate_and_scan(context: &Context) -> String {
    let credentials = json!({"username":"admin","password":"pass123"});
    json_request(
        &context.app,
        "POST",
        "/api/auth/setup",
        Some(credentials.clone()),
        None,
    )
    .await;
    let (_, login) = json_request(
        &context.app,
        "POST",
        "/api/auth/login",
        Some(credentials),
        None,
    )
    .await;
    let token = login["token"].as_str().expect("JWT").to_owned();
    let (_, library) = json_request(
        &context.app,
        "POST",
        "/api/libraries",
        Some(json!({"name":"来源","path":context.source,"role":"source","scanEnabled":true,"watchEnabled":false,"writable":true})),
        Some(&token),
    )
    .await;
    let (_, job) = json_request(
        &context.app,
        "POST",
        &format!(
            "/api/libraries/{}/scan",
            library["id"].as_str().expect("曲库")
        ),
        None,
        Some(&token),
    )
    .await;
    for _ in 0..300 {
        let (_, current) = json_request(
            &context.app,
            "GET",
            &format!("/api/jobs/{}", job["id"].as_str().expect("任务")),
            None,
            Some(&token),
        )
        .await;
        if matches!(
            current["status"].as_str(),
            Some("completed" | "completed_with_errors" | "failed")
        ) {
            assert_eq!(current["status"], "completed");
            return token;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("扫描未完成")
}

async fn json_request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let response = raw(app, method, uri, body, token, None).await;
    let status = response.status();
    let bytes = body_bytes(response).await;
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON")
    };
    (status, value)
}

async fn raw(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
    extra_header: Option<(header::HeaderName, &'static str)>,
) -> axum::response::Response {
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
    if let Some((name, value)) = extra_header {
        builder = builder.header(name, value);
    }
    app.clone()
        .oneshot(builder.body(payload).expect("请求"))
        .await
        .expect("响应")
}

async fn body_bytes(response: axum::response::Response) -> axum::body::Bytes {
    response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
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
    fs::write(path, bytes).expect("WAV");
}
