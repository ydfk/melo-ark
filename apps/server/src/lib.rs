#![forbid(unsafe_code)]

pub mod ai;
pub mod auth;
pub mod config;
pub mod db;
pub mod duplicates;
pub mod error;
pub mod jobs;
pub mod library;
pub mod lyrics;
pub mod model;
pub mod opensubsonic;
pub mod organizer;
pub mod playback;
pub mod providers;
pub mod routes;
pub mod runtime_settings;
pub mod scanner;
pub mod scraper;
pub mod state;
pub mod tag_operations;
pub mod text_normalization;
pub mod trash;

use std::path::Path;

use axum::{Router, http::HeaderValue};
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{config::AppConfig, state::AppState};

pub async fn build_app(config: &AppConfig) -> anyhow::Result<Router> {
    let pool = db::connect(&config.database.path).await?;
    if config.app.environment != "test" && auth::ensure_default_admin(&pool, &config.jwt).await? {
        tracing::warn!(
            username = "admin",
            password = "admin",
            "已创建默认管理员，请登录后立即修改密码"
        );
    }
    text_normalization::ensure_search_index(&pool).await?;
    jobs::recover_interrupted(&pool).await?;
    jobs::cleanup_expired_logs(&pool).await?;
    let environment_locks = runtime_settings::detect_environment_locks();
    let runtime = runtime_settings::load(&pool, config, &environment_locks).await?;
    let (events, _) = tokio::sync::broadcast::channel(256);
    let scan_workers = config.scan.io_workers;
    let state = AppState {
        pool,
        jwt: config.jwt.clone(),
        scan: config.scan.clone(),
        providers: config.providers.clone(),
        analysis: config.analysis.clone(),
        ai: config.ai.clone(),
        playback: config.playback.clone(),
        runtime: std::sync::Arc::new(tokio::sync::RwLock::new(runtime)),
        environment_locks: std::sync::Arc::new(environment_locks),
        app_config: std::sync::Arc::new(config.clone()),
        http: reqwest::Client::builder()
            .user_agent(&config.providers.user_agent)
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()?,
        provider_last_request: std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        events,
        watch_generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        scan_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(scan_workers)),
        analysis_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(
            config.analysis.workers,
        )),
        transcode_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(
            config.playback.transcode_workers,
        )),
        login_failures: std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
    };
    scanner::start_background_services(state.clone());
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
            if let Err(error) = jobs::cleanup_expired_logs(&cleanup_state.pool).await {
                tracing::warn!(%error, "清理过期任务日志失败");
            }
        }
    });

    let mut app = routes::router(state)
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<_>| {
                tracing::info_span!("http_request", method = %request.method(), path = %request.uri().path())
            },
        ))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers([axum::http::header::CONTENT_TYPE]),
        )
        .layer(axum::middleware::map_response(
            |mut response: axum::response::Response| async {
                response.headers_mut().insert(
                    axum::http::header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                );
                response
            },
        ));

    if let Some(web_dist) = config.app.web_dist.as_deref()
        && Path::new(web_dist).join("index.html").is_file()
    {
        let index = Path::new(web_dist).join("index.html");
        app =
            app.fallback_service(ServeDir::new(web_dist).not_found_service(ServeFile::new(index)));
    }

    Ok(app)
}
