use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

use super::{
    MarkedReviewCount, ReviewBatchItem, ReviewBatchItemPage, ReviewBatchPreviewRequest, ReviewPage,
    ReviewRecord, ReviewSelection,
};

pub async fn list(
    state: &AppState,
    status: Option<&str>,
    kind: Option<&str>,
    marked: Option<bool>,
    page: i64,
    per_page: i64,
) -> Result<ReviewPage, AppError> {
    validate_selection(status, kind)?;
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let mut count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM review_items");
    push_review_filters(&mut count, status, kind, marked);
    let total = count
        .build_query_scalar::<i64>()
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;

    let mut marked_count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM review_items");
    push_review_filters(&mut marked_count, status, kind, Some(true));
    let marked_total = marked_count
        .build_query_scalar::<i64>()
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;

    let mut rows = QueryBuilder::<Sqlite>::new(
        r#"SELECT id, kind, status, marked, title, detail, track_id, media_file_id,
             library_id, confidence, payload_json, created_at, updated_at FROM review_items"#,
    );
    push_review_filters(&mut rows, status, kind, marked);
    rows.push(" ORDER BY marked DESC, updated_at DESC, id DESC LIMIT ")
        .push_bind(per_page)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = rows
        .build_query_as::<ReviewRecord>()
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
    Ok(ReviewPage {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        per_page,
        total,
        marked_total,
    })
}

pub async fn clear_marks(
    state: &AppState,
    selection: ReviewSelection,
) -> Result<MarkedReviewCount, AppError> {
    validate_selection(Some(&selection.status), selection.kind.as_deref())?;
    let mut query =
        QueryBuilder::<Sqlite>::new("UPDATE review_items SET marked = 0, updated_at = ");
    query.push_bind(Utc::now());
    push_review_filters(
        &mut query,
        Some(&selection.status),
        selection.kind.as_deref(),
        Some(true),
    );
    let result = query
        .build()
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
    Ok(MarkedReviewCount {
        count: i64::try_from(result.rows_affected()).unwrap_or(i64::MAX),
    })
}

pub(super) async fn resolve_preview_ids(
    state: &AppState,
    request: &ReviewBatchPreviewRequest,
) -> Result<Vec<Uuid>, AppError> {
    match (&request.review_ids, &request.selection) {
        (Some(ids), None) => Ok(ids.clone()),
        (None, Some(selection)) => {
            validate_selection(Some(&selection.status), selection.kind.as_deref())?;
            let mut query = QueryBuilder::<Sqlite>::new("SELECT id FROM review_items");
            push_review_filters(
                &mut query,
                Some(&selection.status),
                selection.kind.as_deref(),
                Some(true),
            );
            query.push(" ORDER BY updated_at DESC, id DESC");
            query
                .build_query_scalar::<Uuid>()
                .fetch_all(&state.pool)
                .await
                .map_err(AppError::internal)
        }
        _ => Err(AppError::BadRequest(
            "reviewIds 与 selection 必须且只能提供一个".to_owned(),
        )),
    }
}

pub async fn list_preview_items(
    state: &AppState,
    user_id: Uuid,
    preview_id: Uuid,
    page: i64,
    per_page: i64,
) -> Result<ReviewBatchItemPage, AppError> {
    let owner =
        sqlx::query_scalar::<_, Uuid>("SELECT created_by FROM review_batch_previews WHERE id = ?")
            .bind(preview_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::NotFound("批量预览不存在".to_owned()))?;
    if owner != user_id {
        return Err(AppError::NotFound("批量预览不存在".to_owned()));
    }
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM review_batch_preview_items WHERE preview_id = ?",
    )
    .bind(preview_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let items = sqlx::query_as::<_, ReviewBatchItem>(
        r#"SELECT review_id, title, eligible, reason
           FROM review_batch_preview_items WHERE preview_id = ?
           ORDER BY position LIMIT ? OFFSET ?"#,
    )
    .bind(preview_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(ReviewBatchItemPage {
        items,
        page,
        per_page,
        total,
    })
}

fn validate_selection(status: Option<&str>, kind: Option<&str>) -> Result<(), AppError> {
    if status.is_some_and(|value| !matches!(value, "pending" | "resolved" | "ignored")) {
        return Err(AppError::BadRequest("待处理状态不合法".to_owned()));
    }
    if kind.is_some_and(|value| {
        !matches!(
            value,
            "metadata_candidate"
                | "missing_artwork"
                | "missing_lyrics"
                | "incomplete_tags"
                | "duplicate"
                | "quality_variant"
                | "organize_required"
                | "hardlink_conflict"
                | "not_writable"
                | "parse_failed"
                | "job_failed"
                | "source_missing"
        )
    }) {
        return Err(AppError::BadRequest("待处理类型不合法".to_owned()));
    }
    Ok(())
}

fn push_review_filters(
    query: &mut QueryBuilder<Sqlite>,
    status: Option<&str>,
    kind: Option<&str>,
    marked: Option<bool>,
) {
    if status.is_none() && kind.is_none() && marked.is_none() {
        return;
    }
    query.push(" WHERE ");
    let mut first = true;
    if let Some(status) = status {
        query.push("status = ").push_bind(status.to_owned());
        first = false;
    }
    if let Some(kind) = kind {
        if !first {
            query.push(" AND ");
        }
        query.push("kind = ").push_bind(kind.to_owned());
        first = false;
    }
    if let Some(marked) = marked {
        if !first {
            query.push(" AND ");
        }
        query.push("marked = ").push_bind(marked);
    }
}
