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
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

async fn test_app() -> (Router, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
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
            path: temp_dir
                .path()
                .join("test.sqlite")
                .to_string_lossy()
                .into_owned(),
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
    (build_app(&config).await.expect("build app"), temp_dir)
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, String) {
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
        .oneshot(builder.body(request_body).expect("build request"))
        .await
        .expect("send request");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    (
        status,
        String::from_utf8(bytes.to_vec()).expect("utf8 body"),
    )
}

#[tokio::test]
async fn health_and_openapi_follow_the_contract() {
    let (app, _temp_dir) = test_app().await;

    let (status, body) = request(&app, "GET", "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let health: Value = serde_json::from_str(&body).expect("health json");
    assert_eq!(health["status"], "ok");
    assert_eq!(health["service"], "meloark");

    let (status, body) = request(&app, "GET", "/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let spec: Value = serde_json::from_str(&body).expect("openapi json");
    assert_eq!(spec["openapi"], "3.1.0");
    for path in [
        "/api/health",
        "/api/auth/setup-status",
        "/api/auth/setup",
        "/api/auth/login",
        "/api/auth/profile",
        "/api/tracks/{id}",
        "/api/tracks/{id}/files",
        "/api/tracks/{id}/operations",
        "/api/tags/preview",
        "/api/tags/apply",
        "/api/organizer/preview",
        "/api/organizer/apply",
        "/api/trash/preview",
        "/api/trash/restore",
        "/api/trash",
        "/api/trash/purge/preview",
        "/api/trash/purge/apply",
        "/api/providers",
        "/api/scrape/search",
        "/api/lyrics/search",
        "/api/duplicates/groups",
        "/api/ai/status",
        "/api/media/{id}/stream",
        "/api/media/{id}/transcode",
        "/api/media/{id}/play-token",
        "/api/artwork/{id}",
        "/api/playback/history",
        "/api/favorites",
        "/api/playlists",
    ] {
        assert!(spec["paths"].get(path).is_some(), "missing path {path}");
    }
    assert!(spec["components"]["securitySchemes"]["bearerAuth"].is_object());

    let (status, body) = request(&app, "GET", "/openapi.yaml", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("openapi: 3.1.0"));

    let (status, _) = request(&app, "GET", "/docs/", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn setup_login_and_profile_are_interoperable() {
    let (app, _temp_dir) = test_app().await;
    let credentials = json!({"username": "alice", "password": "pass123"});

    let (status, body) = request(
        &app,
        "POST",
        "/api/auth/setup",
        Some(credentials.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let user: Value = serde_json::from_str(&body).expect("user json");
    assert_eq!(user["username"], "alice");
    assert!(user["createdAt"].is_string());

    let (status, body) = request(
        &app,
        "POST",
        "/api/auth/login",
        Some(credentials.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let login: Value = serde_json::from_str(&body).expect("token json");
    let token = login["token"].as_str().expect("token");

    let (status, body) = request(&app, "GET", "/api/auth/profile", None, Some(token)).await;
    assert_eq!(status, StatusCode::OK);
    let profile: Value = serde_json::from_str(&body).expect("profile json");
    assert_eq!(profile["id"], user["id"]);

    let (status, body) = request(&app, "POST", "/api/auth/setup", Some(credentials), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let problem: Value = serde_json::from_str(&body).expect("problem json");
    assert_eq!(problem["status"], 409);
}

#[tokio::test]
async fn invalid_or_missing_credentials_use_problem_responses() {
    let (app, _temp_dir) = test_app().await;

    let (status, body) = request(&app, "GET", "/api/auth/profile", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let problem: Value = serde_json::from_str(&body).expect("problem json");
    assert_eq!(problem["title"], "Unauthorized");

    let (status, body) = request(
        &app,
        "POST",
        "/api/auth/setup",
        Some(json!({"username": "alice", "password": "short"})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let problem: Value = serde_json::from_str(&body).expect("problem json");
    assert_eq!(problem["status"], 422);
}

#[tokio::test]
async fn repeated_login_failures_are_rate_limited_without_leaking_credentials() {
    let (app, _temp_dir) = test_app().await;
    let credentials = json!({"username": "alice", "password": "pass123"});
    request(&app, "POST", "/api/auth/setup", Some(credentials), None).await;

    for _ in 0..5 {
        let (status, body) = request(
            &app,
            "POST",
            "/api/auth/login",
            Some(json!({"username": "alice", "password": "wrong-password"})),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!body.contains("wrong-password"));
    }
    let (status, body) = request(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({"username": "alice", "password": "pass123"})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let problem: Value = serde_json::from_str(&body).expect("problem json");
    assert_eq!(problem["status"], 429);
}
