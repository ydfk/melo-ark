use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{duplicates, error::AppError, state::AppState};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_configured: bool,
    pub uploads_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AiRecommendation {
    pub id: Uuid,
    pub relation: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AiDuplicateRequest {
    pub group_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AiRerankRequest {
    pub track_id: Uuid,
    pub candidate_ids: Vec<Uuid>,
    pub confirmation: String,
}

pub fn status(state: &AppState) -> AiStatus {
    AiStatus {
        enabled: state.ai.enabled,
        base_url: state.ai.base_url.clone(),
        model: state.ai.model.clone(),
        api_key_configured: !state.ai.api_key.is_empty(),
        uploads_audio: false,
    }
}

pub async fn explain_duplicate(
    state: &AppState,
    request: AiDuplicateRequest,
) -> Result<AiRecommendation, AppError> {
    if request.confirmation != "SEND_METADATA" {
        return Err(AppError::BadRequest(
            "AI 请求需要显式确认 SEND_METADATA".to_owned(),
        ));
    }
    ensure_enabled(state)?;
    let group = duplicates::get_group(state, request.group_id).await?;
    let payload = serde_json::json!({
        "task": "classify_duplicate_relation",
        "groupKind": group.kind,
        "ruleConfidence": group.confidence,
        "ruleReason": group.reason,
        "files": group.members.iter().map(|item| serde_json::json!({
            "filename": std::path::Path::new(&item.path).file_name().and_then(|name| name.to_str()),
            "title": item.title, "artist": item.artist, "extension": item.extension,
            "durationMs": item.duration_ms, "codec": item.codec, "bitrate": item.bitrate,
            "sampleRate": item.sample_rate, "bitDepth": item.bit_depth, "qualityScore": item.quality_score
        })).collect::<Vec<_>>()
    });
    call_and_store(state, "duplicate_group", request.group_id, payload).await
}

pub async fn rerank_candidates(
    state: &AppState,
    request: AiRerankRequest,
) -> Result<AiRecommendation, AppError> {
    if request.confirmation != "SEND_METADATA" {
        return Err(AppError::BadRequest(
            "AI 请求需要显式确认 SEND_METADATA".to_owned(),
        ));
    }
    ensure_enabled(state)?;
    if request.candidate_ids.is_empty() || request.candidate_ids.len() > 20 {
        return Err(AppError::BadRequest(
            "候选数量必须在 1 到 20 之间".to_owned(),
        ));
    }
    let mut candidates = Vec::new();
    for id in request.candidate_ids {
        if let Some(row) = sqlx::query_as::<_, (Uuid,String,String,String,Option<String>,Option<i64>,i64,String)>("SELECT id, provider_id, title, artists_json, album, duration_ms, score, differences_json FROM scrape_candidates WHERE id = ? AND track_id = ?")
            .bind(id).bind(request.track_id).fetch_optional(&state.pool).await.map_err(AppError::internal)? { candidates.push(row); }
    }
    let payload = serde_json::json!({ "task": "rerank_metadata_candidates", "trackId": request.track_id, "candidates": candidates.iter().map(|item| serde_json::json!({"id":item.0,"provider":item.1,"title":item.2,"artists":serde_json::from_str::<serde_json::Value>(&item.3).ok(),"album":item.4,"durationMs":item.5,"ruleScore":item.6,"differences":serde_json::from_str::<serde_json::Value>(&item.7).ok()})).collect::<Vec<_>>() });
    call_and_store(state, "scrape_candidates", request.track_id, payload).await
}

fn ensure_enabled(state: &AppState) -> Result<(), AppError> {
    if !state.ai.enabled {
        return Err(AppError::BadRequest(
            "AI 未启用；核心功能不依赖 AI".to_owned(),
        ));
    }
    if !(state.ai.base_url.starts_with("https://")
        || state.ai.base_url.starts_with("http://127.0.0.1:")
        || state.ai.base_url.starts_with("http://localhost:"))
    {
        return Err(AppError::BadRequest(
            "AI base_url 必须使用 HTTPS；本地网关可使用 localhost HTTP".to_owned(),
        ));
    }
    Ok(())
}

async fn call_and_store(
    state: &AppState,
    subject_kind: &str,
    subject_id: Uuid,
    payload: serde_json::Value,
) -> Result<AiRecommendation, AppError> {
    let system = "You classify music metadata only. Never recommend deleting a file. Return JSON only: {\"relation\":string,\"confidence\":number between 0 and 1,\"reason\":string}.";
    let request_body = serde_json::json!({ "model": state.ai.model, "messages": [ {"role":"system","content":system}, {"role":"user","content":serde_json::to_string(&payload).map_err(AppError::internal)?} ], "response_format": {"type":"json_object"}, "temperature": 0.1 });
    let endpoint = format!(
        "{}/v1/chat/completions",
        state.ai.base_url.trim_end_matches('/')
    );
    let response: serde_json::Value = state
        .http
        .post(endpoint)
        .bearer_auth(&state.ai.api_key)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(state.ai.timeout_sec))
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)?;
    let content = response
        .pointer("/choices/0/message/content")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::BadRequest("AI 响应缺少 message.content".to_owned()))?;
    let parsed: serde_json::Value = serde_json::from_str(
        content
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim(),
    )
    .map_err(|error| AppError::BadRequest(format!("AI 没有返回有效 JSON：{error}")))?;
    let relation = parsed["relation"]
        .as_str()
        .unwrap_or("uncertain")
        .to_owned();
    let confidence = parsed["confidence"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
    let reason = parsed["reason"]
        .as_str()
        .unwrap_or("AI 未提供原因")
        .to_owned();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO ai_recommendations (id, subject_kind, subject_id, relation, confidence, reason, request_json, response_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(id).bind(subject_kind).bind(subject_id).bind(&relation).bind(confidence).bind(&reason)
        .bind(serde_json::to_string(&payload).map_err(AppError::internal)?).bind(serde_json::to_string(&response).map_err(AppError::internal)?)
        .bind(Utc::now()).execute(&state.pool).await.map_err(AppError::internal)?;
    Ok(AiRecommendation {
        id,
        relation,
        confidence,
        reason,
    })
}
