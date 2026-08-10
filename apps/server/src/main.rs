use meloark_server::{build_app, config::AppConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load("config")?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.logging.filter))
        .init();

    let address = format!("{}:{}", config.app.host, config.app.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    let app = build_app(&config).await?;

    tracing::info!(%address, environment = %config.app.environment, "服务已启动");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "监听退出信号失败");
    }
}
