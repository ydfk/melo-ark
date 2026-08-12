use std::path::Path;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    pub app: ServerConfig,
    pub jwt: JwtConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub providers: ProviderConfig,
    #[serde(default)]
    pub analysis: AnalysisConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub playback: PlaybackConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlaybackConfig {
    pub ffmpeg_path: String,
    pub transcode_workers: usize,
    pub cache_dir: String,
    pub cache_max_bytes: i64,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            ffmpeg_path: "ffmpeg".to_owned(),
            transcode_workers: 2,
            cache_dir: "/data/cache/transcode".to_owned(),
            cache_max_bytes: 10 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnalysisConfig {
    pub workers: usize,
    pub fpcalc_path: String,
    pub fingerprint_threshold: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            fpcalc_path: "fpcalc".to_owned(),
            fingerprint_threshold: 0.88,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_sec: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://api.openai.com".to_owned(),
            api_key: String::new(),
            model: String::new(),
            timeout_sec: 30,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProviderConfig {
    pub user_agent: String,
    pub cache_ttl_sec: i64,
    pub retry_attempts: usize,
    pub circuit_breaker_failures: i64,
    pub circuit_breaker_cooldown_sec: i64,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            user_agent: "MeloArk/0.1.0 (https://github.com/)".to_owned(),
            cache_ttl_sec: 86_400,
            retry_attempts: 2,
            circuit_breaker_failures: 3,
            circuit_breaker_cooldown_sec: 300,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub environment: String,
    #[serde(default)]
    pub web_dist: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingConfig {
    pub filter: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScanConfig {
    pub io_workers: usize,
    pub reconcile_interval_sec: u64,
    pub watch_debounce_sec: u64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            io_workers: 2,
            reconcile_interval_sec: 21_600,
            watch_debounce_sec: 5,
        }
    }
}

impl AppConfig {
    pub fn load(config_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let config_dir = config_dir.as_ref();
        let default_path = config_dir.join("config.yaml");
        let local_path = config_dir.join("config.local.yaml");

        let mut builder =
            config::Config::builder().add_source(config::File::from(default_path).required(true));
        if local_path.exists() {
            builder = builder.add_source(config::File::from(local_path).required(false));
        }

        builder = builder.add_source(
            config::Environment::with_prefix("MELOARK")
                .prefix_separator("__")
                .separator("__")
                .try_parsing(true),
        );

        let settings: Self = builder.build()?.try_deserialize()?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.app.host.trim().is_empty(), "app.host 不能为空");
        anyhow::ensure!(
            !self.database.path.trim().is_empty(),
            "database.path 不能为空"
        );
        anyhow::ensure!(self.jwt.expiration > 0, "jwt.expiration 必须大于 0");
        anyhow::ensure!(self.jwt.secret.len() >= 16, "jwt.secret 至少需要 16 个字符");
        if self.app.environment == "production" {
            anyhow::ensure!(
                self.jwt.secret != "replace-this-secret-before-production",
                "生产环境必须通过 MELOARK__JWT__SECRET 配置独立密钥"
            );
        }
        anyhow::ensure!(self.scan.io_workers > 0, "scan.io_workers 必须大于 0");
        anyhow::ensure!(
            self.providers.user_agent.contains('/'),
            "providers.user_agent 必须包含应用名与版本"
        );
        anyhow::ensure!(self.providers.cache_ttl_sec > 0, "数据源缓存时间必须大于 0");
        anyhow::ensure!(
            self.providers.retry_attempts <= 5,
            "数据源重试次数不能大于 5"
        );
        anyhow::ensure!(self.analysis.workers > 0, "analysis.workers 必须大于 0");
        anyhow::ensure!(
            (0.0..=1.0).contains(&self.analysis.fingerprint_threshold),
            "analysis.fingerprint_threshold 必须在 0 到 1 之间"
        );
        if self.ai.enabled {
            anyhow::ensure!(
                !self.ai.api_key.trim().is_empty(),
                "启用 AI 时必须配置 api_key"
            );
            anyhow::ensure!(!self.ai.model.trim().is_empty(), "启用 AI 时必须配置 model");
        }
        anyhow::ensure!(
            self.playback.transcode_workers > 0,
            "playback.transcode_workers 必须大于 0"
        );
        anyhow::ensure!(
            self.playback.cache_max_bytes > 0,
            "playback.cache_max_bytes 必须大于 0"
        );
        anyhow::ensure!(
            self.scan.reconcile_interval_sec >= 60,
            "scan.reconcile_interval_sec 不能小于 60 秒"
        );
        Ok(())
    }
}
