use std::{collections::BTreeSet, sync::Arc};

use sqlx::SqlitePool;
use tokio::sync::{Mutex, Semaphore, broadcast};

use crate::{
    config::{AiConfig, AnalysisConfig, JwtConfig, PlaybackConfig, ProviderConfig, ScanConfig},
    jobs::JobEvent,
    runtime_settings::SharedRuntimeSettings,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt: JwtConfig,
    pub scan: ScanConfig,
    pub providers: ProviderConfig,
    pub analysis: AnalysisConfig,
    pub ai: AiConfig,
    pub playback: PlaybackConfig,
    pub runtime: SharedRuntimeSettings,
    pub environment_locks: Arc<BTreeSet<String>>,
    pub app_config: Arc<crate::config::AppConfig>,
    pub http: reqwest::Client,
    pub provider_last_request:
        Arc<tokio::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>>,
    pub events: broadcast::Sender<JobEvent>,
    pub watch_generation: Arc<AtomicU64>,
    pub scan_semaphore: Arc<Semaphore>,
    pub analysis_semaphore: Arc<Semaphore>,
    pub transcode_semaphore: Arc<Semaphore>,
    pub login_failures: Arc<Mutex<std::collections::HashMap<String, Vec<std::time::Instant>>>>,
}
use std::sync::atomic::AtomicU64;
