use std::{collections::BTreeSet, sync::Arc};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use utoipa::ToSchema;

use crate::{
    auth::{decrypt_subsonic_secret, encrypt_subsonic_secret},
    config::AppConfig,
    error::AppError,
};

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditableSettings {
    pub scan_workers: usize,
    pub reconcile_interval_sec: u64,
    pub watch_debounce_sec: u64,
    pub source_cache_ttl_sec: i64,
    pub source_retry_attempts: usize,
    pub source_circuit_breaker_failures: i64,
    pub source_circuit_breaker_cooldown_sec: i64,
    pub analysis_workers: usize,
    pub fingerprint_threshold: f64,
    pub ai_enabled: bool,
    pub ai_base_url: String,
    pub ai_model: String,
    pub ai_timeout_sec: u64,
    pub transcode_workers: usize,
    pub transcode_cache_max_bytes: i64,
    pub organizer_template: String,
    pub organizer_cross_platform_safe: bool,
}

#[derive(Clone, Debug)]
pub struct RuntimeSettings {
    pub editable: EditableSettings,
    pub ai_api_key: String,
}

pub type SharedRuntimeSettings = Arc<RwLock<RuntimeSettings>>;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InfrastructureSettings {
    pub host: String,
    pub port: u16,
    pub database_path: String,
    pub ffmpeg_path: String,
    pub fpcalc_path: String,
    pub transcode_cache_dir: String,
    pub platform: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResponse {
    pub values: EditableSettings,
    pub ai_api_key_configured: bool,
    pub locked_by_environment: Vec<String>,
    pub restart_required_fields: Vec<String>,
    pub infrastructure: InfrastructureSettings,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub values: EditableSettings,
    pub ai_api_key: Option<String>,
    #[serde(default)]
    pub clear_ai_api_key: bool,
}

pub fn defaults(config: &AppConfig) -> EditableSettings {
    EditableSettings {
        scan_workers: config.scan.io_workers,
        reconcile_interval_sec: config.scan.reconcile_interval_sec,
        watch_debounce_sec: config.scan.watch_debounce_sec,
        source_cache_ttl_sec: config.providers.cache_ttl_sec,
        source_retry_attempts: config.providers.retry_attempts,
        source_circuit_breaker_failures: config.providers.circuit_breaker_failures,
        source_circuit_breaker_cooldown_sec: config.providers.circuit_breaker_cooldown_sec,
        analysis_workers: config.analysis.workers,
        fingerprint_threshold: config.analysis.fingerprint_threshold,
        ai_enabled: config.ai.enabled,
        ai_base_url: config.ai.base_url.clone(),
        ai_model: config.ai.model.clone(),
        ai_timeout_sec: config.ai.timeout_sec,
        transcode_workers: config.playback.transcode_workers,
        transcode_cache_max_bytes: config.playback.cache_max_bytes,
        organizer_template: "{artist}/{album}/{track:02} - {title}.{ext}".to_owned(),
        organizer_cross_platform_safe: true,
    }
}

pub async fn load(
    pool: &SqlitePool,
    config: &AppConfig,
    locked: &BTreeSet<String>,
) -> Result<RuntimeSettings, AppError> {
    let base = defaults(config);
    let stored = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT settings_json, ai_api_key_ciphertext FROM runtime_settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;
    let (mut editable, stored_key) = if let Some((json, key)) = stored {
        (
            serde_json::from_str::<EditableSettings>(&json).map_err(AppError::internal)?,
            key,
        )
    } else {
        (base.clone(), None)
    };
    apply_environment_values(&mut editable, &base, locked);
    let ai_api_key = if locked.contains("ai.apiKey") {
        config.ai.api_key.clone()
    } else if let Some(ciphertext) = stored_key {
        decrypt_subsonic_secret(&ciphertext, &config.jwt)?
    } else {
        config.ai.api_key.clone()
    };
    Ok(RuntimeSettings {
        editable,
        ai_api_key,
    })
}

pub async fn save(
    pool: &SqlitePool,
    config: &AppConfig,
    settings: &RuntimeSettings,
) -> Result<(), AppError> {
    let json = serde_json::to_string(&settings.editable).map_err(AppError::internal)?;
    let encrypted_key = if settings.ai_api_key.is_empty() {
        None
    } else {
        Some(encrypt_subsonic_secret(&settings.ai_api_key, &config.jwt)?)
    };
    sqlx::query(
        r#"INSERT INTO runtime_settings (id, settings_json, ai_api_key_ciphertext, updated_at)
           VALUES (1, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET settings_json = excluded.settings_json,
             ai_api_key_ciphertext = excluded.ai_api_key_ciphertext,
             updated_at = excluded.updated_at"#,
    )
    .bind(json)
    .bind(encrypted_key)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(AppError::internal)?;
    Ok(())
}

pub fn detect_environment_locks() -> BTreeSet<String> {
    [
        ("MELOARK__SCAN__IO_WORKERS", "scanWorkers"),
        (
            "MELOARK__SCAN__RECONCILE_INTERVAL_SEC",
            "reconcileIntervalSec",
        ),
        ("MELOARK__SCAN__WATCH_DEBOUNCE_SEC", "watchDebounceSec"),
        ("MELOARK__PROVIDERS__CACHE_TTL_SEC", "sourceCacheTtlSec"),
        ("MELOARK__PROVIDERS__RETRY_ATTEMPTS", "sourceRetryAttempts"),
        (
            "MELOARK__PROVIDERS__CIRCUIT_BREAKER_FAILURES",
            "sourceCircuitBreakerFailures",
        ),
        (
            "MELOARK__PROVIDERS__CIRCUIT_BREAKER_COOLDOWN_SEC",
            "sourceCircuitBreakerCooldownSec",
        ),
        ("MELOARK__ANALYSIS__WORKERS", "analysisWorkers"),
        (
            "MELOARK__ANALYSIS__FINGERPRINT_THRESHOLD",
            "fingerprintThreshold",
        ),
        ("MELOARK__AI__ENABLED", "aiEnabled"),
        ("MELOARK__AI__BASE_URL", "aiBaseUrl"),
        ("MELOARK__AI__MODEL", "aiModel"),
        ("MELOARK__AI__TIMEOUT_SEC", "aiTimeoutSec"),
        ("MELOARK__AI__API_KEY", "ai.apiKey"),
        ("MELOARK__PLAYBACK__TRANSCODE_WORKERS", "transcodeWorkers"),
        (
            "MELOARK__PLAYBACK__CACHE_MAX_BYTES",
            "transcodeCacheMaxBytes",
        ),
    ]
    .into_iter()
    .filter(|(variable, _)| std::env::var_os(variable).is_some())
    .map(|(_, field)| field.to_owned())
    .collect()
}

pub fn validate(settings: &EditableSettings) -> Result<(), AppError> {
    if settings.scan_workers == 0
        || settings.analysis_workers == 0
        || settings.transcode_workers == 0
    {
        return Err(AppError::BadRequest("并发数必须大于 0".to_owned()));
    }
    if settings.reconcile_interval_sec < 60 {
        return Err(AppError::BadRequest(
            "定期扫描间隔不能小于 60 秒".to_owned(),
        ));
    }
    if settings.watch_debounce_sec == 0 || settings.watch_debounce_sec > 300 {
        return Err(AppError::BadRequest(
            "文件监听延迟必须在 1 到 300 秒之间".to_owned(),
        ));
    }
    if settings.source_cache_ttl_sec <= 0
        || settings.source_retry_attempts > 5
        || settings.source_circuit_breaker_failures <= 0
        || settings.source_circuit_breaker_cooldown_sec <= 0
    {
        return Err(AppError::BadRequest("在线数据源策略参数无效".to_owned()));
    }
    if !(0.0..=1.0).contains(&settings.fingerprint_threshold) {
        return Err(AppError::BadRequest(
            "指纹匹配阈值必须在 0 到 1 之间".to_owned(),
        ));
    }
    if settings.ai_enabled
        && (settings.ai_base_url.trim().is_empty() || settings.ai_model.trim().is_empty())
    {
        return Err(AppError::BadRequest(
            "启用 AI 时必须填写服务地址和模型".to_owned(),
        ));
    }
    if settings.ai_timeout_sec == 0 || settings.transcode_cache_max_bytes <= 0 {
        return Err(AppError::BadRequest("AI 超时或转码缓存上限无效".to_owned()));
    }
    if settings.organizer_template.len() > 512
        || !settings.organizer_template.contains("{title}")
        || !settings.organizer_template.contains("{ext}")
    {
        return Err(AppError::BadRequest(
            "整理模板必须包含 {title} 和 {ext}，且不能超过 512 字节".to_owned(),
        ));
    }
    Ok(())
}

fn apply_environment_values(
    target: &mut EditableSettings,
    base: &EditableSettings,
    locked: &BTreeSet<String>,
) {
    macro_rules! apply {
        ($field:ident, $name:literal) => {
            if locked.contains($name) {
                target.$field = base.$field.clone();
            }
        };
    }
    apply!(scan_workers, "scanWorkers");
    apply!(reconcile_interval_sec, "reconcileIntervalSec");
    apply!(watch_debounce_sec, "watchDebounceSec");
    apply!(source_cache_ttl_sec, "sourceCacheTtlSec");
    apply!(source_retry_attempts, "sourceRetryAttempts");
    apply!(
        source_circuit_breaker_failures,
        "sourceCircuitBreakerFailures"
    );
    apply!(
        source_circuit_breaker_cooldown_sec,
        "sourceCircuitBreakerCooldownSec"
    );
    apply!(analysis_workers, "analysisWorkers");
    apply!(fingerprint_threshold, "fingerprintThreshold");
    apply!(ai_enabled, "aiEnabled");
    apply!(ai_base_url, "aiBaseUrl");
    apply!(ai_model, "aiModel");
    apply!(ai_timeout_sec, "aiTimeoutSec");
    apply!(transcode_workers, "transcodeWorkers");
    apply!(transcode_cache_max_bytes, "transcodeCacheMaxBytes");
}
