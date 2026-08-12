use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use crate::{
    error::{AppError, Problem},
    runtime_settings::{
        EditableSettings, InfrastructureSettings, SettingsResponse, UpdateSettingsRequest, save,
        validate,
    },
    state::AppState,
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/settings", get(get_settings).patch(update_settings))
}

#[utoipa::path(
    get,
    path = "/api/settings",
    tag = "settings",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "当前运行设置", body = SettingsResponse))
)]
pub async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(response(&state).await))
}

#[utoipa::path(
    patch,
    path = "/api/settings",
    tag = "settings",
    security(("bearerAuth" = [])),
    request_body = UpdateSettingsRequest,
    responses(
        (status = 200, description = "运行设置已保存", body = SettingsResponse),
        (status = 409, description = "字段由环境变量管理", body = Problem),
        (status = 422, description = "设置值无效", body = Problem)
    )
)]
pub async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, AppError> {
    require_user_id(&headers, &state)?;
    validate(&request.values)?;
    let current = state.runtime.read().await.clone();
    let changed_locked =
        locked_changes(&current.editable, &request.values, &state.environment_locks);
    let key_locked = state.environment_locks.contains("ai.apiKey")
        && (request.ai_api_key.is_some() || request.clear_ai_api_key);
    if !changed_locked.is_empty() || key_locked {
        let mut fields = changed_locked;
        if key_locked {
            fields.push("ai.apiKey".to_owned());
        }
        return Err(AppError::Conflict(format!(
            "以下设置由环境变量管理：{}",
            fields.join("、")
        )));
    }

    let mut updated = current;
    updated.editable = request.values;
    if request.clear_ai_api_key {
        updated.ai_api_key.clear();
    } else if let Some(api_key) = request.ai_api_key {
        updated.ai_api_key = api_key.trim().to_owned();
    }
    if updated.editable.ai_enabled && updated.ai_api_key.is_empty() {
        return Err(AppError::BadRequest(
            "启用 AI 时必须配置 API Key".to_owned(),
        ));
    }
    save(&state.pool, &state.app_config, &updated).await?;
    *state.runtime.write().await = updated;
    crate::scanner::refresh_watchers(state.clone());
    Ok(Json(response(&state).await))
}

async fn response(state: &AppState) -> SettingsResponse {
    let runtime = state.runtime.read().await;
    SettingsResponse {
        values: runtime.editable.clone(),
        ai_api_key_configured: !runtime.ai_api_key.is_empty(),
        locked_by_environment: state.environment_locks.iter().cloned().collect(),
        restart_required_fields: vec![
            "scanWorkers".to_owned(),
            "analysisWorkers".to_owned(),
            "transcodeWorkers".to_owned(),
        ],
        infrastructure: InfrastructureSettings {
            host: state.app_config.app.host.clone(),
            port: state.app_config.app.port,
            database_path: state.app_config.database.path.clone(),
            ffmpeg_path: state.app_config.playback.ffmpeg_path.clone(),
            fpcalc_path: state.app_config.analysis.fpcalc_path.clone(),
            transcode_cache_dir: state.app_config.playback.cache_dir.clone(),
            platform: "linux/amd64".to_owned(),
        },
    }
}

fn locked_changes(
    current: &EditableSettings,
    requested: &EditableSettings,
    locked: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    let candidates = [
        (
            "scanWorkers",
            current.scan_workers != requested.scan_workers,
        ),
        (
            "reconcileIntervalSec",
            current.reconcile_interval_sec != requested.reconcile_interval_sec,
        ),
        (
            "watchDebounceSec",
            current.watch_debounce_sec != requested.watch_debounce_sec,
        ),
        (
            "sourceCacheTtlSec",
            current.source_cache_ttl_sec != requested.source_cache_ttl_sec,
        ),
        (
            "sourceRetryAttempts",
            current.source_retry_attempts != requested.source_retry_attempts,
        ),
        (
            "sourceCircuitBreakerFailures",
            current.source_circuit_breaker_failures != requested.source_circuit_breaker_failures,
        ),
        (
            "sourceCircuitBreakerCooldownSec",
            current.source_circuit_breaker_cooldown_sec
                != requested.source_circuit_breaker_cooldown_sec,
        ),
        (
            "analysisWorkers",
            current.analysis_workers != requested.analysis_workers,
        ),
        (
            "fingerprintThreshold",
            current.fingerprint_threshold != requested.fingerprint_threshold,
        ),
        ("aiEnabled", current.ai_enabled != requested.ai_enabled),
        ("aiBaseUrl", current.ai_base_url != requested.ai_base_url),
        ("aiModel", current.ai_model != requested.ai_model),
        (
            "aiTimeoutSec",
            current.ai_timeout_sec != requested.ai_timeout_sec,
        ),
        (
            "transcodeWorkers",
            current.transcode_workers != requested.transcode_workers,
        ),
        (
            "transcodeCacheMaxBytes",
            current.transcode_cache_max_bytes != requested.transcode_cache_max_bytes,
        ),
    ];
    candidates
        .into_iter()
        .filter(|(field, changed)| *changed && locked.contains(*field))
        .map(|(field, _)| field.to_owned())
        .collect()
}
