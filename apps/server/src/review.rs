use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, jobs::JobResponse, state::AppState};

pub const RULE_METADATA: &str = "high_confidence_metadata";
pub const RULE_LYRICS: &str = "best_lyrics";
pub const RULE_ARTWORK: &str = "missing_artwork";
pub const RULE_REORGANIZE: &str = "reorganize";
pub const RULE_DUPLICATES: &str = "recommended_duplicates";

#[derive(Clone, Debug, FromRow)]
struct ReviewRecord {
    id: Uuid,
    kind: String,
    status: String,
    marked: bool,
    title: String,
    detail: String,
    track_id: Option<Uuid>,
    media_file_id: Option<Uuid>,
    library_id: Option<Uuid>,
    confidence: Option<f64>,
    payload_json: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub marked: bool,
    pub title: String,
    pub detail: String,
    pub track_id: Option<Uuid>,
    pub media_file_id: Option<Uuid>,
    pub library_id: Option<Uuid>,
    pub confidence: Option<f64>,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ReviewRecord> for ReviewItem {
    fn from(value: ReviewRecord) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            status: value.status,
            marked: value.marked,
            title: value.title,
            detail: value.detail,
            track_id: value.track_id,
            media_file_id: value.media_file_id,
            library_id: value.library_id,
            confidence: value.confidence,
            payload: serde_json::from_str(&value.payload_json).unwrap_or_default(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPage {
    pub items: Vec<ReviewItem>,
    pub total: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReviewRequest {
    pub marked: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatchPreviewRequest {
    pub review_ids: Vec<Uuid>,
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatchItem {
    pub review_id: Uuid,
    pub title: String,
    pub eligible: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatchPreview {
    pub id: Uuid,
    pub rule: String,
    pub total_items: i64,
    pub eligible_items: i64,
    pub blocked_items: i64,
    pub items: Vec<ReviewBatchItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReviewBatchRequest {
    pub preview_id: Uuid,
    pub confirmation: String,
}

#[derive(Debug, Clone)]
pub struct ReviewIssue<'a> {
    pub kind: &'a str,
    pub subject_key: String,
    pub title: &'a str,
    pub detail: String,
    pub track_id: Option<Uuid>,
    pub media_file_id: Option<Uuid>,
    pub library_id: Option<Uuid>,
    pub confidence: Option<f64>,
    pub payload: serde_json::Value,
}

pub async fn upsert_issue(state: &AppState, issue: ReviewIssue<'_>) -> Result<Uuid, AppError> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let payload = serde_json::to_string(&issue.payload).map_err(AppError::internal)?;
    sqlx::query(
        r#"INSERT INTO review_items
             (id, kind, status, marked, title, detail, subject_key, track_id,
              media_file_id, library_id, confidence, payload_json, created_at, updated_at)
           VALUES (?, ?, 'pending', 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(kind, subject_key) DO UPDATE SET status = 'pending',
             title = excluded.title, detail = excluded.detail, track_id = excluded.track_id,
             media_file_id = excluded.media_file_id, library_id = excluded.library_id,
             confidence = excluded.confidence, payload_json = excluded.payload_json,
             updated_at = excluded.updated_at"#,
    )
    .bind(id)
    .bind(issue.kind)
    .bind(issue.title)
    .bind(issue.detail)
    .bind(&issue.subject_key)
    .bind(issue.track_id)
    .bind(issue.media_file_id)
    .bind(issue.library_id)
    .bind(issue.confidence)
    .bind(payload)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    sqlx::query_scalar("SELECT id FROM review_items WHERE kind = ? AND subject_key = ?")
        .bind(issue.kind)
        .bind(&issue.subject_key)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)
}

pub async fn list(
    state: &AppState,
    status: Option<&str>,
    kind: Option<&str>,
    marked: Option<bool>,
) -> Result<ReviewPage, AppError> {
    if status.is_some_and(|value| !matches!(value, "pending" | "resolved" | "ignored")) {
        return Err(AppError::BadRequest("待处理状态不合法".to_owned()));
    }
    let rows = sqlx::query_as::<_, ReviewRecord>(
        r#"SELECT id, kind, status, marked, title, detail, track_id, media_file_id,
             library_id, confidence, payload_json, created_at, updated_at
           FROM review_items
           WHERE (? IS NULL OR status = ?) AND (? IS NULL OR kind = ?)
             AND (? IS NULL OR marked = ?)
           ORDER BY marked DESC, updated_at DESC, rowid DESC"#,
    )
    .bind(status)
    .bind(status)
    .bind(kind)
    .bind(kind)
    .bind(marked)
    .bind(marked)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let total = i64::try_from(rows.len()).unwrap_or(i64::MAX);
    Ok(ReviewPage {
        items: rows.into_iter().map(Into::into).collect(),
        total,
    })
}

pub async fn update(
    state: &AppState,
    id: Uuid,
    request: UpdateReviewRequest,
) -> Result<ReviewItem, AppError> {
    if request
        .status
        .as_deref()
        .is_some_and(|value| !matches!(value, "pending" | "resolved" | "ignored"))
    {
        return Err(AppError::BadRequest("待处理状态不合法".to_owned()));
    }
    let changed = sqlx::query(
        r#"UPDATE review_items SET marked = COALESCE(?, marked),
             status = COALESCE(?, status), updated_at = ? WHERE id = ?"#,
    )
    .bind(request.marked)
    .bind(request.status)
    .bind(Utc::now())
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(AppError::NotFound("待处理项不存在".to_owned()));
    }
    fetch_record(state, id).await.map(Into::into)
}

pub async fn preview_batch(
    state: &AppState,
    user_id: Uuid,
    request: ReviewBatchPreviewRequest,
) -> Result<ReviewBatchPreview, AppError> {
    validate_rule(&request.rule)?;
    if request.review_ids.is_empty() {
        return Err(AppError::BadRequest("至少选择一个待处理项".to_owned()));
    }
    let mut items = Vec::with_capacity(request.review_ids.len());
    for id in &request.review_ids {
        let record = fetch_record(state, *id).await?;
        let blocked_reason = eligibility(state, &record, &request.rule).await?;
        let preview_detail = if blocked_reason.is_none() && request.rule == RULE_DUPLICATES {
            duplicate_preview_detail(state, &record).await?
        } else {
            None
        };
        items.push(ReviewBatchItem {
            review_id: *id,
            title: record.title,
            eligible: blocked_reason.is_none(),
            reason: blocked_reason.or(preview_detail),
        });
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        r#"INSERT INTO review_batch_previews
             (id, rule, review_ids_json, items_json, created_by, created_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(id)
    .bind(&request.rule)
    .bind(serde_json::to_string(&request.review_ids).map_err(AppError::internal)?)
    .bind(serde_json::to_string(&items).map_err(AppError::internal)?)
    .bind(user_id)
    .bind(now)
    .bind(now + Duration::minutes(15))
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(preview_response(id, request.rule, items))
}

pub async fn apply_batch(
    state: AppState,
    user_id: Uuid,
    request: ApplyReviewBatchRequest,
) -> Result<JobResponse, AppError> {
    if request.confirmation != "APPLY" {
        return Err(AppError::BadRequest(
            "批量处理必须提交 confirmation=APPLY".to_owned(),
        ));
    }
    let (rule, items_json, owner, expires_at, applied_at) =
        sqlx::query_as::<_, (String, String, Uuid, DateTime<Utc>, Option<DateTime<Utc>>)>(
            "SELECT rule, items_json, created_by, expires_at, applied_at FROM review_batch_previews WHERE id = ?",
        )
        .bind(request.preview_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::NotFound("批量预览不存在".to_owned()))?;
    if owner != user_id {
        return Err(AppError::NotFound("批量预览不存在".to_owned()));
    }
    if expires_at < Utc::now() {
        return Err(AppError::Conflict("批量预览已过期，请重新预览".to_owned()));
    }
    if applied_at.is_some() {
        return Err(AppError::Conflict("批量预览已经执行".to_owned()));
    }
    let items: Vec<ReviewBatchItem> =
        serde_json::from_str(&items_json).map_err(AppError::internal)?;
    let eligible: Vec<_> = items.into_iter().filter(|item| item.eligible).collect();
    if eligible.is_empty() {
        return Err(AppError::Conflict("没有可执行的待处理项".to_owned()));
    }
    let job_id = Uuid::new_v4();
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    let claimed = sqlx::query(
        "UPDATE review_batch_previews SET applied_at = ? WHERE id = ? AND applied_at IS NULL",
    )
    .bind(now)
    .bind(request.preview_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if claimed.rows_affected() == 0 {
        return Err(AppError::Conflict("批量预览已经执行".to_owned()));
    }
    sqlx::query(
        r#"INSERT INTO jobs
             (id, kind, status, source_type, source_id, total_items, created_at, updated_at)
           VALUES (?, 'review_batch', 'queued', 'review', ?, ?, ?, ?)"#,
    )
    .bind(job_id)
    .bind(request.preview_id.to_string())
    .bind(i64::try_from(eligible.len()).unwrap_or(i64::MAX))
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    for item in &eligible {
        sqlx::query(
            r#"INSERT INTO job_items
                 (id, job_id, item_key, status, retryable, updated_at)
               VALUES (?, ?, ?, 'pending', 1, ?)"#,
        )
        .bind(Uuid::new_v4())
        .bind(job_id)
        .bind(item.review_id.to_string())
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    }
    transaction.commit().await.map_err(AppError::internal)?;
    crate::jobs::record_log(
        &state,
        job_id,
        "info",
        "queued",
        None,
        None,
        "待处理批量任务已加入队列",
    )
    .await?;
    let job = crate::jobs::fetch_job(&state.pool, job_id).await?;
    crate::jobs::emit(&state, job.clone());
    tokio::spawn(async move {
        if let Err(error) = run_batch(&state, job_id, &rule, user_id).await {
            let _ = fail_job(&state, job_id, &error.to_string()).await;
        }
    });
    Ok(job)
}

pub async fn resume_batch_job(state: AppState, job_id: Uuid) -> Result<(), AppError> {
    let (rule, user_id) = sqlx::query_as::<_, (String, Uuid)>(
        r#"SELECT rbp.rule, rbp.created_by FROM jobs j
           JOIN review_batch_previews rbp ON rbp.id = j.source_id
           WHERE j.id = ? AND j.kind = 'review_batch'"#,
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("待处理批量任务不存在".to_owned()))?;
    tokio::spawn(async move {
        if let Err(error) = run_batch(&state, job_id, &rule, user_id).await {
            let _ = fail_job(&state, job_id, &error.to_string()).await;
        }
    });
    Ok(())
}

pub async fn resume_pending(state: AppState) -> Result<(), AppError> {
    let job_ids = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT id FROM jobs
           WHERE kind = 'review_batch' AND status IN ('queued', 'interrupted')"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for job_id in job_ids {
        resume_batch_job(state.clone(), job_id).await?;
    }
    Ok(())
}

pub async fn retry_batch_job(state: AppState, job_id: Uuid) -> Result<(), AppError> {
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    let changed = sqlx::query(
        r#"UPDATE job_items SET status = 'pending', message = NULL, retryable = 1, updated_at = ?
           WHERE job_id = ? AND status = 'failed'"#,
    )
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "待处理批量任务当前没有可重试失败项".to_owned(),
        ));
    }
    sqlx::query(
        r#"UPDATE jobs SET status = 'queued', processed_items = success_items + skipped_items,
             failed_items = 0, current_item = NULL, error_message = NULL,
             finished_at = NULL, updated_at = ? WHERE id = ?"#,
    )
    .bind(now)
    .bind(job_id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    resume_batch_job(state, job_id).await
}

async fn run_batch(
    state: &AppState,
    job_id: Uuid,
    rule: &str,
    user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("UPDATE jobs SET status = 'running', started_at = ?, updated_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let items = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, item_key FROM job_items WHERE job_id = ? AND status = 'pending' ORDER BY rowid",
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    for (item_id, item_key) in items {
        sqlx::query(
            "UPDATE job_items SET status = 'running', attempt_count = attempt_count + 1, updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now())
        .bind(item_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        let review_id = Uuid::parse_str(&item_key).map_err(AppError::internal)?;
        let result = execute_rule(state, review_id, rule, user_id).await;
        let error = result.as_ref().err().map(ToString::to_string);
        let succeeded = result.is_ok();
        sqlx::query(
            "UPDATE job_items SET status = ?, message = ?, retryable = ?, updated_at = ? WHERE id = ?",
        )
        .bind(if succeeded { "success" } else { "failed" })
        .bind(error.as_deref())
        .bind(!succeeded)
        .bind(Utc::now())
        .bind(item_id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if succeeded {
            sqlx::query(
                "UPDATE review_items SET status = 'resolved', marked = 0, updated_at = ? WHERE id = ?",
            )
            .bind(Utc::now())
            .bind(review_id)
            .execute(&state.pool)
            .await
            .map_err(AppError::internal)?;
        }
        crate::jobs::record_log(
            state,
            job_id,
            if succeeded { "info" } else { "error" },
            if succeeded { "success" } else { "failed" },
            Some(&item_key),
            Some(1),
            error.as_deref().unwrap_or("处理成功"),
        )
        .await?;
        refresh_job_counts(state, job_id, Some(&item_key)).await?;
    }
    let failed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    let status = if failed == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
    sqlx::query(
        "UPDATE jobs SET status = ?, current_item = NULL, finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(Utc::now())
    .bind(Utc::now())
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    crate::jobs::record_log(state, job_id, "info", status, None, None, "批量处理完成").await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

async fn execute_rule(
    state: &AppState,
    review_id: Uuid,
    rule: &str,
    user_id: Uuid,
) -> Result<(), AppError> {
    let record = fetch_record(state, review_id).await?;
    let payload: serde_json::Value =
        serde_json::from_str(&record.payload_json).map_err(AppError::internal)?;
    match rule {
        RULE_METADATA | RULE_ARTWORK => {
            let candidate_id = payload
                .get("candidateId")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| AppError::Conflict("待处理项没有可用候选".to_owned()))?;
            let operation = crate::scraper::preview_candidate_missing_fields(
                state,
                user_id,
                candidate_id,
                rule == RULE_ARTWORK,
            )
            .await?;
            crate::tag_operations::apply(
                state,
                crate::tag_operations::ApplyOperationRequest {
                    operation_id: operation.id,
                    confirmation: "APPLY".to_owned(),
                },
            )
            .await?;
        }
        RULE_LYRICS => {
            let track_id = record
                .track_id
                .ok_or_else(|| AppError::Conflict("待处理项没有曲目".to_owned()))?;
            let media_id = record
                .media_file_id
                .ok_or_else(|| AppError::Conflict("待处理项没有媒体文件".to_owned()))?;
            let lyrics_id: Uuid = sqlx::query_scalar(
                "SELECT id FROM lyrics WHERE track_id = ? AND quality_score >= 80 ORDER BY quality_score DESC, created_at DESC LIMIT 1",
            )
            .bind(track_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::Conflict("没有达到质量要求的歌词".to_owned()))?;
            crate::lyrics::apply(
                state,
                crate::lyrics::ApplyLyricsRequest {
                    job_id: None,
                    lyrics_id,
                    media_file_id: media_id,
                    mode: crate::lyrics::LyricsWriteMode::External,
                    replace_existing: false,
                    confirmation: "USE_LYRICS".to_owned(),
                },
            )
            .await?;
        }
        RULE_REORGANIZE => {
            let media_id = record
                .media_file_id
                .ok_or_else(|| AppError::Conflict("待处理项没有媒体文件".to_owned()))?;
            let target_id = record
                .library_id
                .ok_or_else(|| AppError::Conflict("待处理项没有已整理曲库".to_owned()))?;
            let valid_target: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM libraries WHERE id = ? AND role = 'managed' AND writable = 1)",
            )
            .bind(target_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .unwrap_or(false);
            if !valid_target {
                return Err(AppError::Conflict("已整理曲库当前不可写".to_owned()));
            }
            let operation = crate::organizer::preview(
                state,
                user_id,
                crate::organizer::OrganizerPreviewRequest {
                    media_ids: vec![media_id],
                    target_library_id: target_id,
                    template: crate::organizer::DEFAULT_TEMPLATE.to_owned(),
                    cross_platform_safe: true,
                },
            )
            .await?;
            crate::organizer::apply(
                state,
                crate::organizer::OrganizerApplyRequest {
                    operation_id: operation.id,
                    confirmation: "APPLY".to_owned(),
                },
            )
            .await?;
        }
        RULE_DUPLICATES => {
            let group_id = payload
                .get("groupId")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| AppError::Conflict("待处理项没有重复组".to_owned()))?;
            let media_ids = sqlx::query_scalar::<_, Uuid>(
                "SELECT media_file_id FROM duplicate_group_members WHERE group_id = ? AND recommended_keep = 0",
            )
            .bind(group_id)
            .fetch_all(&state.pool)
            .await
            .map_err(AppError::internal)?;
            if media_ids.is_empty() {
                return Err(AppError::Conflict("重复组没有可移除项".to_owned()));
            }
            let operation = crate::trash::preview(
                state,
                user_id,
                crate::trash::TrashPreviewRequest { media_ids },
            )
            .await?;
            crate::trash::apply(
                state,
                crate::trash::TrashApplyRequest {
                    operation_id: operation.id,
                    confirmation: "TRASH".to_owned(),
                },
            )
            .await?;
        }
        _ => return Err(AppError::BadRequest("批量规则不合法".to_owned())),
    }
    Ok(())
}

async fn eligibility(
    state: &AppState,
    record: &ReviewRecord,
    rule: &str,
) -> Result<Option<String>, AppError> {
    if record.status != "pending" {
        return Ok(Some("该项已经处理".to_owned()));
    }
    let matches_kind = match rule {
        RULE_METADATA => record.kind == "metadata_candidate" && record.confidence >= Some(0.95),
        RULE_LYRICS => record.kind == "missing_lyrics",
        RULE_ARTWORK => record.kind == "missing_artwork",
        RULE_REORGANIZE => record.kind == "organize_required",
        RULE_DUPLICATES => record.kind == "duplicate",
        _ => false,
    };
    if !matches_kind {
        return Ok(Some("该项不适用于所选规则".to_owned()));
    }
    if matches!(rule, RULE_METADATA | RULE_ARTWORK) {
        let payload: serde_json::Value =
            serde_json::from_str(&record.payload_json).map_err(AppError::internal)?;
        let Some(candidate_id) = payload
            .get("candidateId")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            return Ok(Some("还没有可用的在线候选".to_owned()));
        };
        if rule == RULE_ARTWORK {
            let has_artwork: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM scrape_candidates WHERE id = ? AND artwork_url IS NOT NULL AND artwork_url != '')",
            )
            .bind(candidate_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
            if !has_artwork {
                return Ok(Some("候选中没有可用封面".to_owned()));
            }
        }
    }
    if rule == RULE_LYRICS {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lyrics WHERE track_id = ? AND quality_score >= 80",
        )
        .bind(record.track_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if count == 0 {
            return Ok(Some("没有达到 80 分的歌词候选".to_owned()));
        }
    }
    if rule == RULE_DUPLICATES {
        let payload: serde_json::Value =
            serde_json::from_str(&record.payload_json).map_err(AppError::internal)?;
        let Some(group_id) = payload
            .get("groupId")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            return Ok(Some("待处理项没有重复组".to_owned()));
        };
        let (members, keepers): (i64, i64) = sqlx::query_as(
            r#"SELECT COUNT(*),
                 COALESCE(SUM(CASE WHEN recommended_keep = 1 THEN 1 ELSE 0 END), 0)
               FROM duplicate_group_members WHERE group_id = ?"#,
        )
        .bind(group_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if members < 2 || keepers != 1 {
            return Ok(Some("重复组已变化，请重新分析后再处理".to_owned()));
        }
    }
    Ok(None)
}

async fn duplicate_preview_detail(
    state: &AppState,
    record: &ReviewRecord,
) -> Result<Option<String>, AppError> {
    let payload: serde_json::Value =
        serde_json::from_str(&record.payload_json).map_err(AppError::internal)?;
    let group_id = payload
        .get("groupId")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| AppError::Conflict("待处理项没有重复组".to_owned()))?;
    let members = sqlx::query_as::<_, (String, String, bool, i64, i64)>(
        r#"SELECT l.path, mf.relative_path, dgm.recommended_keep,
             dgm.quality_score, mf.file_size
           FROM duplicate_group_members dgm
           JOIN media_files mf ON mf.id = dgm.media_file_id
           JOIN libraries l ON l.id = mf.library_id
           WHERE dgm.group_id = ?
           ORDER BY dgm.recommended_keep DESC, dgm.quality_score DESC"#,
    )
    .bind(group_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let lines = members
        .into_iter()
        .map(|(root, relative, keep, quality, size)| {
            let action = if keep { "保留" } else { "移入回收站" };
            let path = std::path::Path::new(&root).join(relative);
            format!(
                "{action}：{}（质量 {quality}，{} 字节）",
                path.to_string_lossy(),
                size.max(0)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Some(lines))
}

fn validate_rule(rule: &str) -> Result<(), AppError> {
    if matches!(
        rule,
        RULE_METADATA | RULE_LYRICS | RULE_ARTWORK | RULE_REORGANIZE | RULE_DUPLICATES
    ) {
        Ok(())
    } else {
        Err(AppError::BadRequest("批量规则不合法".to_owned()))
    }
}

fn preview_response(id: Uuid, rule: String, items: Vec<ReviewBatchItem>) -> ReviewBatchPreview {
    let eligible_items = items.iter().filter(|item| item.eligible).count();
    ReviewBatchPreview {
        id,
        rule,
        total_items: i64::try_from(items.len()).unwrap_or(i64::MAX),
        eligible_items: i64::try_from(eligible_items).unwrap_or(i64::MAX),
        blocked_items: i64::try_from(items.len().saturating_sub(eligible_items))
            .unwrap_or(i64::MAX),
        items,
    }
}

async fn fetch_record(state: &AppState, id: Uuid) -> Result<ReviewRecord, AppError> {
    sqlx::query_as::<_, ReviewRecord>(
        r#"SELECT id, kind, status, marked, title, detail, track_id, media_file_id,
             library_id, confidence, payload_json, created_at, updated_at
           FROM review_items WHERE id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("待处理项不存在".to_owned()))
}

async fn refresh_job_counts(
    state: &AppState,
    job_id: Uuid,
    current: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE jobs SET current_item = ?,
             processed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status IN ('success','skipped','failed')),
             success_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'success'),
             failed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'),
             updated_at = ? WHERE id = ?"#,
    )
    .bind(current)
    .bind(job_id)
    .bind(job_id)
    .bind(job_id)
    .bind(Utc::now())
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}

async fn fail_job(state: &AppState, job_id: Uuid, message: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET status = 'failed', error_message = ?, finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(message)
    .bind(Utc::now())
    .bind(Utc::now())
    .bind(job_id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    crate::jobs::record_log(state, job_id, "error", "failed", None, None, message).await?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, job_id).await?);
    Ok(())
}
