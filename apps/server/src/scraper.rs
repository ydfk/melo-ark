use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use strsim::normalized_levenshtein;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    providers::{
        ProviderCapabilities, ProviderTrack, TrackQuery, metadata_provider, metadata_registry,
    },
    state::AppState,
    tag_operations::{self, OperationResponse, TagPreviewRequest, TagSet},
    text_normalization::{compact_match_key, version_mismatch},
};

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSetting {
    pub provider_id: String,
    pub display_name: String,
    pub kind: String,
    pub enabled: bool,
    pub priority: i64,
    pub maturity: String,
    pub base_url: Option<String>,
    pub timeout_ms: i64,
    pub rate_limit_ms: i64,
    pub consecutive_failures: i64,
    pub circuit_open_until: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    #[sqlx(skip)]
    pub capabilities: Option<ProviderCapabilities>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub enabled: Option<bool>,
    pub priority: Option<i64>,
    pub base_url: Option<String>,
    pub timeout_ms: Option<i64>,
    pub rate_limit_ms: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeSearchRequest {
    pub track_id: Uuid,
    #[serde(default)]
    pub provider_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeCandidate {
    pub id: Uuid,
    pub track_id: Uuid,
    pub provider_id: String,
    pub provider_item_id: String,
    pub title: String,
    #[sqlx(json)]
    pub artists_json: serde_json::Value,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub version_label: Option<String>,
    pub artwork_url: Option<String>,
    pub score: i64,
    pub confidence: String,
    #[sqlx(json)]
    pub differences_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFailure {
    pub provider_id: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeSearchResponse {
    pub candidates: Vec<ScrapeCandidate>,
    pub failures: Vec<ProviderFailure>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeApplyRequest {
    pub candidate_id: Uuid,
    pub confirmation: String,
    #[serde(default)]
    pub include_artwork: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchScrapeRequest {
    pub track_ids: Vec<Uuid>,
    #[serde(default)]
    pub provider_ids: Vec<String>,
}

#[derive(Debug, FromRow)]
struct TrackRow {
    id: Uuid,
    title: String,
    artists: String,
    album: Option<String>,
    duration_ms: Option<i64>,
    track_no: Option<i64>,
    year: Option<i64>,
    version_label: Option<String>,
}

pub async fn list_providers(state: &AppState) -> Result<Vec<ProviderSetting>, AppError> {
    let mut settings = sqlx::query_as::<_, ProviderSetting>(
        r#"SELECT provider_id, display_name, kind, enabled, priority, maturity, base_url,
          timeout_ms, rate_limit_ms, consecutive_failures, circuit_open_until,
          last_success_at, last_error FROM provider_settings ORDER BY priority"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for setting in &mut settings {
        setting.capabilities =
            metadata_provider(&setting.provider_id).map(|item| item.capabilities());
    }
    Ok(settings)
}

pub async fn update_provider(
    state: &AppState,
    id: &str,
    request: UpdateProviderRequest,
) -> Result<ProviderSetting, AppError> {
    if let Some(url) = request.base_url.as_deref() {
        validate_base_url(url)?;
    }
    if request
        .priority
        .is_some_and(|value| !(0..=10_000).contains(&value))
    {
        return Err(AppError::BadRequest(
            "数据源优先级必须在 0 到 10000 之间".to_owned(),
        ));
    }
    if request
        .timeout_ms
        .is_some_and(|value| !(100..=120_000).contains(&value))
    {
        return Err(AppError::BadRequest(
            "数据源超时必须在 100 到 120000 毫秒之间".to_owned(),
        ));
    }
    if request
        .rate_limit_ms
        .is_some_and(|value| !(0..=60_000).contains(&value))
    {
        return Err(AppError::BadRequest(
            "数据源请求间隔必须在 0 到 60000 毫秒之间".to_owned(),
        ));
    }
    let affected = sqlx::query(
        r#"UPDATE provider_settings SET enabled = COALESCE(?, enabled),
          priority = COALESCE(?, priority), base_url = COALESCE(?, base_url),
          timeout_ms = COALESCE(?, timeout_ms), rate_limit_ms = COALESCE(?, rate_limit_ms),
          updated_at = ? WHERE provider_id = ?"#,
    )
    .bind(request.enabled)
    .bind(request.priority)
    .bind(request.base_url)
    .bind(request.timeout_ms)
    .bind(request.rate_limit_ms)
    .bind(Utc::now())
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound("在线数据源不存在".to_owned()));
    }
    let mut items = list_providers(state).await?;
    items
        .drain(..)
        .find(|item| item.provider_id == id)
        .ok_or_else(|| AppError::NotFound("在线数据源不存在".to_owned()))
}

pub async fn search(
    state: &AppState,
    request: ScrapeSearchRequest,
) -> Result<ScrapeSearchResponse, AppError> {
    let query = load_track_query(state, request.track_id).await?;
    let mut failures = Vec::new();
    let mut candidates = Vec::new();
    let configured = list_providers(state).await?;
    for provider in metadata_registry() {
        if !request.provider_ids.is_empty()
            && !request.provider_ids.iter().any(|id| id == provider.id())
        {
            continue;
        }
        let Some(setting) = configured
            .iter()
            .find(|setting| setting.provider_id == provider.id() && setting.enabled)
        else {
            continue;
        };
        let Some(base_url) = setting.base_url.as_deref() else {
            failures.push(ProviderFailure {
                provider_id: provider.id().to_owned(),
                code: "not_configured".to_owned(),
                message: "数据源未配置服务地址".to_owned(),
            });
            continue;
        };
        if setting
            .circuit_open_until
            .is_some_and(|until| until > Utc::now())
        {
            failures.push(ProviderFailure {
                provider_id: provider.id().to_owned(),
                code: "circuit_open".to_owned(),
                message: "数据源正在熔断冷却".to_owned(),
            });
            continue;
        }
        match cached_or_search(state, provider.as_ref(), setting, base_url, &query).await {
            Ok(items) => {
                record_success(state, provider.id()).await?;
                for item in items {
                    candidates.push(
                        persist_candidate(state, request.track_id, provider.id(), &query, item)
                            .await?,
                    );
                }
            }
            Err(error) => {
                record_failure(state, provider.id(), &error.to_string()).await?;
                failures.push(ProviderFailure {
                    provider_id: provider.id().to_owned(),
                    code: error.code().to_owned(),
                    message: error.to_string(),
                });
            }
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.score));
    Ok(ScrapeSearchResponse {
        candidates,
        failures,
    })
}

pub async fn create_batch_job(
    state: &AppState,
    request: BatchScrapeRequest,
) -> Result<crate::jobs::JobResponse, AppError> {
    if request.track_ids.is_empty() {
        return Err(AppError::BadRequest("至少选择一首曲目".to_owned()));
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("INSERT INTO jobs (id, kind, status, source_type, source_id, total_items, created_at, updated_at) VALUES (?, 'scrape', 'queued', 'workspace', 'library', ?, ?, ?)")
        .bind(id)
        .bind(i64::try_from(request.track_ids.len()).unwrap_or(i64::MAX))
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("INSERT INTO scrape_jobs (job_id, provider_ids_json, created_at) VALUES (?, ?, ?)")
        .bind(id)
        .bind(serde_json::to_string(&request.provider_ids).map_err(AppError::internal)?)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    for track_id in request.track_ids {
        sqlx::query("INSERT INTO job_items (id, job_id, item_key, status, retryable, updated_at) VALUES (?, ?, ?, 'pending', 1, ?)")
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(track_id.to_string())
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
    }
    transaction.commit().await.map_err(AppError::internal)?;
    let job = crate::jobs::fetch_job(&state.pool, id).await?;
    crate::jobs::record_log(
        state,
        id,
        "info",
        "queued",
        None,
        None,
        "元数据匹配任务已加入队列",
    )
    .await?;
    crate::jobs::emit(state, job.clone());
    spawn_batch_job(state.clone(), id);
    Ok(job)
}

pub fn spawn_batch_job(state: AppState, id: Uuid) {
    tokio::spawn(async move {
        if let Err(error) = run_batch_job(&state, id).await {
            let _ = sqlx::query("UPDATE jobs SET status = 'failed', error_message = ?, finished_at = ?, updated_at = ? WHERE id = ?")
                .bind(error.to_string())
                .bind(Utc::now())
                .bind(Utc::now())
                .bind(id)
                .execute(&state.pool)
                .await;
            if let Ok(job) = crate::jobs::fetch_job(&state.pool, id).await {
                let _ = crate::jobs::record_log(
                    &state,
                    id,
                    "error",
                    "failed",
                    None,
                    None,
                    &error.to_string(),
                )
                .await;
                crate::jobs::emit(&state, job);
            }
        }
    });
}

async fn run_batch_job(state: &AppState, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE jobs SET status = 'running', started_at = COALESCE(started_at, ?), finished_at = NULL, updated_at = ? WHERE id = ? AND status IN ('queued', 'interrupted')")
        .bind(Utc::now()).bind(Utc::now()).bind(id).execute(&state.pool).await.map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        id,
        "info",
        "started",
        None,
        None,
        "元数据匹配任务开始执行",
    )
    .await?;
    let provider_ids_json: String =
        sqlx::query_scalar("SELECT provider_ids_json FROM scrape_jobs WHERE job_id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    let provider_ids: Vec<String> =
        serde_json::from_str(&provider_ids_json).map_err(AppError::internal)?;
    let items = sqlx::query_as::<_, (Uuid, String)>("SELECT id, item_key FROM job_items WHERE job_id = ? AND status IN ('pending', 'failed') ORDER BY rowid")
        .bind(id).fetch_all(&state.pool).await.map_err(AppError::internal)?;
    for (item_id, item_key) in items {
        loop {
            let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
                .bind(id)
                .fetch_one(&state.pool)
                .await
                .map_err(AppError::internal)?;
            match status.as_str() {
                "paused" => tokio::time::sleep(Duration::from_millis(200)).await,
                "cancel_requested" => {
                    sqlx::query("UPDATE jobs SET status = 'cancelled', finished_at = ?, updated_at = ? WHERE id = ?")
                        .bind(Utc::now()).bind(Utc::now()).bind(id).execute(&state.pool).await.map_err(AppError::internal)?;
                    crate::jobs::record_log(
                        state,
                        id,
                        "warn",
                        "cancelled",
                        None,
                        None,
                        "元数据匹配任务已取消",
                    )
                    .await?;
                    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, id).await?);
                    return Ok(());
                }
                _ => break,
            }
        }
        sqlx::query("UPDATE job_items SET status = 'running', attempt_count = attempt_count + 1, updated_at = ? WHERE id = ?")
            .bind(Utc::now()).bind(item_id).execute(&state.pool).await.map_err(AppError::internal)?;
        let attempt: i64 = sqlx::query_scalar("SELECT attempt_count FROM job_items WHERE id = ?")
            .bind(item_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
        crate::jobs::record_log(
            state,
            id,
            "info",
            "item_started",
            Some(&item_key),
            Some(attempt),
            "开始匹配元数据",
        )
        .await?;
        let result = match Uuid::parse_str(&item_key) {
            Ok(track_id) => search(
                state,
                ScrapeSearchRequest {
                    track_id,
                    provider_ids: provider_ids.clone(),
                },
            )
            .await
            .map(|response| response.candidates.len()),
            Err(error) => Err(AppError::internal(error)),
        };
        let (status, message, retryable) = match result {
            Ok(count) if count > 0 => ("success", Some(format!("保存 {count} 个候选")), false),
            Ok(_) => ("failed", Some("没有可用候选".to_owned()), true),
            Err(error) => ("failed", Some(error.to_string()), true),
        };
        sqlx::query("UPDATE job_items SET status = ?, message = ?, retryable = ?, updated_at = ? WHERE id = ?")
            .bind(status).bind(&message).bind(retryable).bind(Utc::now()).bind(item_id)
            .execute(&state.pool).await.map_err(AppError::internal)?;
        let level = if status == "failed" { "error" } else { "info" };
        crate::jobs::record_log(
            state,
            id,
            level,
            status,
            Some(&item_key),
            Some(attempt),
            message.as_deref().unwrap_or("处理成功"),
        )
        .await?;
        sqlx::query(r#"UPDATE jobs SET current_item = ?, processed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status IN ('success','skipped','failed')),
          success_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'success'), failed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'), updated_at = ? WHERE id = ?"#)
            .bind(&item_key).bind(id).bind(id).bind(id).bind(Utc::now()).bind(id)
            .execute(&state.pool).await.map_err(AppError::internal)?;
        crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, id).await?);
    }
    let failed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    let status = if failed == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
    sqlx::query("UPDATE jobs SET status = ?, current_item = NULL, finished_at = ?, updated_at = ? WHERE id = ?")
        .bind(status).bind(Utc::now()).bind(Utc::now()).bind(id).execute(&state.pool).await.map_err(AppError::internal)?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, id).await?);
    crate::jobs::record_log(
        state,
        id,
        "info",
        status,
        None,
        None,
        "元数据匹配任务处理完成",
    )
    .await?;
    Ok(())
}

pub async fn retry_batch_job(state: &AppState, id: Uuid) -> Result<(), AppError> {
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("UPDATE job_items SET status = 'pending', error_code = NULL, message = NULL, updated_at = ? WHERE job_id = ? AND status = 'failed'")
        .bind(now).bind(id).execute(&mut *transaction).await.map_err(AppError::internal)?;
    sqlx::query("UPDATE jobs SET status = 'queued', processed_items = success_items + skipped_items, failed_items = 0, error_message = NULL, finished_at = NULL, updated_at = ? WHERE id = ?")
        .bind(now).bind(id).execute(&mut *transaction).await.map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    spawn_batch_job(state.clone(), id);
    Ok(())
}

async fn cached_or_search(
    state: &AppState,
    provider: &dyn crate::providers::MetadataProvider,
    setting: &ProviderSetting,
    base_url: &str,
    query: &TrackQuery,
) -> Result<Vec<ProviderTrack>, crate::providers::ProviderError> {
    let cache_key = format!(
        "{}:{}",
        provider.id(),
        serde_json::to_string(query).unwrap_or_default()
    );
    if let Ok(Some(cached)) = sqlx::query_scalar::<_, String>(
        "SELECT response_json FROM provider_cache WHERE cache_key = ? AND expires_at > ?",
    )
    .bind(&cache_key)
    .bind(Utc::now())
    .fetch_optional(&state.pool)
    .await
        && let Ok(items) = serde_json::from_str(&cached)
    {
        return Ok(items);
    }
    let timeout = Duration::from_millis(setting.timeout_ms.clamp(100, 120_000) as u64);
    let policy = state.runtime.read().await.editable.clone();
    let mut attempt = 0_usize;
    let items = loop {
        wait_for_rate_slot(state, provider.id(), setting.rate_limit_ms).await;
        match provider.search_track(state, base_url, query, timeout).await {
            Ok(items) => break items,
            Err(error) if error.is_retryable() && attempt < policy.source_retry_attempts => {
                let delay_ms = 200 * (1_u64 << attempt.min(4));
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            Err(error) => return Err(error),
        }
    };
    let now = Utc::now();
    let _ = sqlx::query(
        r#"INSERT INTO provider_cache (cache_key, provider_id, response_json, expires_at, created_at)
        VALUES (?, ?, ?, ?, ?) ON CONFLICT(cache_key) DO UPDATE SET response_json = excluded.response_json,
        expires_at = excluded.expires_at, created_at = excluded.created_at"#,
    )
    .bind(cache_key)
    .bind(provider.id())
    .bind(serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_owned()))
    .bind(now + chrono::Duration::seconds(policy.source_cache_ttl_sec))
    .bind(now)
    .execute(&state.pool)
    .await;
    Ok(items)
}

async fn wait_for_rate_slot(state: &AppState, provider_id: &str, rate_limit_ms: i64) {
    let mut slots = state.provider_last_request.lock().await;
    if let Some(last) = slots.get(provider_id) {
        let interval = Duration::from_millis(rate_limit_ms.max(0) as u64);
        if let Some(wait) = interval.checked_sub(last.elapsed()) {
            tokio::time::sleep(wait).await;
        }
    }
    slots.insert(provider_id.to_owned(), std::time::Instant::now());
}

async fn record_success(state: &AppState, provider_id: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE provider_settings SET consecutive_failures = 0, circuit_open_until = NULL, last_success_at = ?, last_error = NULL, updated_at = ? WHERE provider_id = ?")
        .bind(Utc::now()).bind(Utc::now()).bind(provider_id).execute(&state.pool).await.map_err(AppError::internal)?;
    Ok(())
}

async fn record_failure(
    state: &AppState,
    provider_id: &str,
    message: &str,
) -> Result<(), AppError> {
    let policy = state.runtime.read().await.editable.clone();
    sqlx::query(
        r#"UPDATE provider_settings SET consecutive_failures = consecutive_failures + 1,
        circuit_open_until = CASE WHEN consecutive_failures + 1 >= ? THEN ? ELSE circuit_open_until END,
        last_error = ?, updated_at = ? WHERE provider_id = ?"#,
    )
    .bind(policy.source_circuit_breaker_failures)
    .bind(Utc::now() + chrono::Duration::seconds(policy.source_circuit_breaker_cooldown_sec))
    .bind(message)
    .bind(Utc::now())
    .bind(provider_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(())
}

async fn persist_candidate(
    state: &AppState,
    track_id: Uuid,
    provider_id: &str,
    query: &TrackQuery,
    item: ProviderTrack,
) -> Result<ScrapeCandidate, AppError> {
    let (score, differences) = score_candidate(query, &item);
    let confidence = confidence(score);
    let id = Uuid::new_v4();
    let artists = serde_json::to_value(&item.artists).map_err(AppError::internal)?;
    let differences_json = serde_json::to_value(&differences).map_err(AppError::internal)?;
    sqlx::query(
        r#"INSERT INTO scrape_candidates (id, track_id, provider_id, provider_item_id, title,
        artists_json, album, duration_ms, year, track_no, version_label, artwork_url, score,
        confidence, differences_json, raw_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(track_id, provider_id, provider_item_id) DO UPDATE SET title = excluded.title,
        artists_json = excluded.artists_json, album = excluded.album, duration_ms = excluded.duration_ms,
        year = excluded.year, track_no = excluded.track_no, version_label = excluded.version_label,
        artwork_url = excluded.artwork_url, score = excluded.score, confidence = excluded.confidence,
        differences_json = excluded.differences_json, raw_json = excluded.raw_json, created_at = excluded.created_at"#,
    )
    .bind(id).bind(track_id).bind(provider_id).bind(&item.id).bind(&item.title)
    .bind(&artists).bind(&item.album).bind(item.duration_ms).bind(item.year).bind(item.track_no)
    .bind(&item.version_label).bind(&item.artwork_url).bind(score).bind(confidence)
    .bind(&differences_json).bind(serde_json::to_string(&item).map_err(AppError::internal)?)
    .bind(Utc::now()).execute(&state.pool).await.map_err(AppError::internal)?;
    fetch_candidate_by_identity(state, track_id, provider_id, &item.id).await
}

pub async fn list_candidates(
    state: &AppState,
    track_id: Uuid,
) -> Result<Vec<ScrapeCandidate>, AppError> {
    sqlx::query_as::<_, ScrapeCandidate>(
        r#"SELECT id, track_id, provider_id, provider_item_id, title,
      artists_json, album, duration_ms, year, track_no, version_label, artwork_url, score,
      confidence, differences_json FROM scrape_candidates WHERE track_id = ? ORDER BY score DESC"#,
    )
    .bind(track_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)
}

async fn fetch_candidate_by_identity(
    state: &AppState,
    track_id: Uuid,
    provider_id: &str,
    provider_item_id: &str,
) -> Result<ScrapeCandidate, AppError> {
    sqlx::query_as::<_, ScrapeCandidate>(r#"SELECT id, track_id, provider_id, provider_item_id, title,
      artists_json, album, duration_ms, year, track_no, version_label, artwork_url, score,
      confidence, differences_json FROM scrape_candidates WHERE track_id = ? AND provider_id = ? AND provider_item_id = ?"#)
        .bind(track_id).bind(provider_id).bind(provider_item_id).fetch_one(&state.pool).await.map_err(AppError::internal)
}

pub async fn apply_candidate(
    state: &AppState,
    user_id: Uuid,
    request: ScrapeApplyRequest,
) -> Result<OperationResponse, AppError> {
    let candidate = sqlx::query_as::<_, ScrapeCandidate>(
        r#"SELECT id, track_id, provider_id, provider_item_id, title,
      artists_json, album, duration_ms, year, track_no, version_label, artwork_url, score,
      confidence, differences_json FROM scrape_candidates WHERE id = ?"#,
    )
    .bind(request.candidate_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("刮削候选不存在".to_owned()))?;
    let expected = match candidate.score {
        95.. => "APPLY",
        80..=94 => "APPLY_REVIEWED",
        _ => "APPLY_LOW_CONFIDENCE",
    };
    if request.confirmation != expected {
        return Err(AppError::BadRequest(format!(
            "此候选需要显式确认 {expected}"
        )));
    }
    let media_ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM media_files WHERE track_id = ?")
        .bind(candidate.track_id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let cover_data_base64 = if request.include_artwork {
        if let Some(url) = candidate.artwork_url.as_deref() {
            Some(fetch_artwork(state, url).await?)
        } else {
            None
        }
    } else {
        None
    };
    let artists: Vec<String> =
        serde_json::from_value(candidate.artists_json).map_err(AppError::internal)?;
    tag_operations::preview(
        state,
        user_id,
        TagPreviewRequest {
            media_ids,
            set: TagSet {
                title: Some(candidate.title),
                artists: Some(artists),
                album: candidate.album,
                track_no: candidate
                    .track_no
                    .and_then(|value| u32::try_from(value).ok()),
                year: candidate.year.and_then(|value| u32::try_from(value).ok()),
                cover_data_base64,
                ..TagSet::default()
            },
            clear: Vec::new(),
            transforms: Vec::new(),
        },
    )
    .await
}

async fn fetch_artwork(state: &AppState, url: &str) -> Result<String, AppError> {
    validate_base_url(url)?;
    let response = state
        .http
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?;
    if response
        .content_length()
        .is_some_and(|size| size > 10 * 1024 * 1024)
    {
        return Err(AppError::BadRequest("封面超过 10 MiB".to_owned()));
    }
    let bytes = response.bytes().await.map_err(AppError::internal)?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err(AppError::BadRequest("封面超过 10 MiB".to_owned()));
    }
    Ok(STANDARD.encode(bytes))
}

async fn load_track_query(state: &AppState, track_id: Uuid) -> Result<TrackQuery, AppError> {
    let row = sqlx::query_as::<_, TrackRow>(
        r#"SELECT t.id, t.title,
      COALESCE(GROUP_CONCAT(a.name, ';'), '') AS artists, al.title AS album, t.duration_ms,
      t.track_no, t.year, t.version_label FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
      LEFT JOIN track_artists ta ON ta.track_id = t.id LEFT JOIN artists a ON a.id = ta.artist_id
      WHERE t.id = ? GROUP BY t.id"#,
    )
    .bind(track_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("曲目不存在".to_owned()))?;
    let _ = row.id;
    Ok(TrackQuery {
        title: row.title,
        artists: row
            .artists
            .split(';')
            .filter(|item| !item.is_empty())
            .map(str::to_owned)
            .collect(),
        album: row.album,
        duration_ms: row.duration_ms,
        track_no: row.track_no,
        year: row.year,
        version_label: row.version_label,
    })
}

fn similarity(left: &str, right: &str) -> f64 {
    normalized_levenshtein(&compact_match_key(left), &compact_match_key(right))
}

pub fn score_candidate(query: &TrackQuery, candidate: &ProviderTrack) -> (i64, Vec<String>) {
    let mut score = similarity(&query.title, &candidate.title) * 40.0;
    let query_artists = query.artists.join(";");
    let candidate_artists = candidate.artists.join(";");
    score += similarity(&query_artists, &candidate_artists) * 30.0;
    score += match (&query.album, &candidate.album) {
        (Some(a), Some(b)) => similarity(a, b) * 12.0,
        _ => 6.0,
    };
    let mut differences = Vec::new();
    match (query.duration_ms, candidate.duration_ms) {
        (Some(a), Some(b)) if (a - b).abs() <= 2_000 => score += 10.0,
        (Some(a), Some(b)) if (a - b).abs() <= 10_000 => {
            score += 5.0;
            differences.push("duration".to_owned());
        }
        (Some(_), Some(_)) => {
            score -= 20.0;
            differences.push("duration_mismatch".to_owned());
        }
        _ => score += 5.0,
    }
    score += match (query.track_no, candidate.track_no) {
        (Some(a), Some(b)) if a == b => 4.0,
        (None, _) | (_, None) => 2.0,
        _ => {
            differences.push("track_no".to_owned());
            0.0
        }
    };
    score += match (query.year, candidate.year) {
        (Some(a), Some(b)) if a == b => 4.0,
        (None, _) | (_, None) => 2.0,
        _ => {
            differences.push("year".to_owned());
            0.0
        }
    };
    if candidate_version_mismatch(query, candidate) {
        score -= 35.0;
        differences.push("version_mismatch".to_owned());
    }
    if similarity(&query.title, &candidate.title) < 0.95 {
        differences.push("title".to_owned());
    }
    if similarity(&query_artists, &candidate_artists) < 0.9 {
        differences.push("artists".to_owned());
    }
    (score.round().clamp(0.0, 100.0) as i64, differences)
}

fn candidate_version_mismatch(query: &TrackQuery, candidate: &ProviderTrack) -> bool {
    let left = format!(
        "{} {}",
        query.title,
        query.version_label.as_deref().unwrap_or("")
    )
    .to_lowercase();
    let right = format!(
        "{} {}",
        candidate.title,
        candidate.version_label.as_deref().unwrap_or("")
    );
    version_mismatch(&left, &right)
}

fn confidence(score: i64) -> &'static str {
    if score >= 95 {
        "high"
    } else if score >= 80 {
        "review"
    } else {
        "low"
    }
}

fn validate_base_url(url: &str) -> Result<(), AppError> {
    if url.starts_with("https://")
        || url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://localhost:")
    {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "数据源地址必须使用 HTTPS；测试环境仅允许 localhost HTTP".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> TrackQuery {
        TrackQuery {
            title: "晴天".to_owned(),
            artists: vec!["周杰伦".to_owned()],
            album: Some("叶惠美".to_owned()),
            duration_ms: Some(269_000),
            track_no: Some(3),
            year: Some(2003),
            version_label: None,
        }
    }
    #[test]
    fn exact_candidate_is_high_confidence() {
        let item = ProviderTrack {
            id: "1".to_owned(),
            title: "晴天".to_owned(),
            artists: vec!["周杰伦".to_owned()],
            album: Some("叶惠美".to_owned()),
            duration_ms: Some(269_000),
            year: Some(2003),
            track_no: Some(3),
            version_label: None,
            artwork_url: None,
        };
        assert_eq!(score_candidate(&query(), &item).0, 100);
    }
    #[test]
    fn version_and_duration_mismatch_are_heavily_penalized() {
        let item = ProviderTrack {
            id: "1".to_owned(),
            title: "晴天 Live".to_owned(),
            artists: vec!["周杰伦".to_owned()],
            album: Some("叶惠美".to_owned()),
            duration_ms: Some(400_000),
            year: Some(2003),
            track_no: Some(3),
            version_label: Some("Live".to_owned()),
            artwork_url: None,
        };
        let (score, differences) = score_candidate(&query(), &item);
        assert!(score < 80);
        assert!(differences.contains(&"version_mismatch".to_owned()));
    }
}
