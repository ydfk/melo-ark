use uuid::Uuid;

use crate::{
    error::AppError,
    review::{ReviewIssue, upsert_issue},
    state::AppState,
};

pub(super) async fn generate_candidates_and_reviews(
    state: &AppState,
    job_id: Uuid,
    track_id: Uuid,
    media_id: Uuid,
    library_id: Uuid,
) {
    let has_artwork: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT has_artwork FROM media_files WHERE id = ?), 0)",
    )
    .bind(media_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);
    match crate::scraper::search(
        state,
        crate::scraper::ScrapeSearchRequest {
            track_id,
            provider_ids: Vec::new(),
        },
    )
    .await
    {
        Ok(result) => {
            if let Some(best) = result.candidates.first() {
                let second_score = result.candidates.get(1).map_or(0, |item| item.score);
                if best.score >= 95 && best.score - second_score >= 10 {
                    let _ = upsert_issue(
                        state,
                        ReviewIssue {
                            kind: "metadata_candidate",
                            subject_key: track_id.to_string(),
                            title: "发现高置信元数据",
                            detail: format!("匹配分数 {}，等待确认应用", best.score),
                            track_id: Some(track_id),
                            media_file_id: Some(media_id),
                            library_id: Some(library_id),
                            confidence: Some(best.score as f64 / 100.0),
                            payload: serde_json::json!({
                                "candidateId": best.id,
                                "score": best.score,
                                "lead": best.score - second_score
                            }),
                        },
                    )
                    .await;
                    let _ = upsert_issue(
                        state,
                        ReviewIssue {
                            kind: "organize_required",
                            subject_key: media_id.to_string(),
                            title: "元数据确认后需要检查整理路径",
                            detail: "高置信元数据可能改变目标目录或文件名".to_owned(),
                            track_id: Some(track_id),
                            media_file_id: Some(media_id),
                            library_id: Some(library_id),
                            confidence: Some(best.score as f64 / 100.0),
                            payload: serde_json::json!({ "candidateId": best.id }),
                        },
                    )
                    .await;
                }
                if !has_artwork && best.artwork_url.is_some() {
                    let _ = upsert_issue(
                        state,
                        ReviewIssue {
                            kind: "missing_artwork",
                            subject_key: media_id.to_string(),
                            title: "缺少封面",
                            detail: "已找到可用封面候选".to_owned(),
                            track_id: Some(track_id),
                            media_file_id: Some(media_id),
                            library_id: Some(library_id),
                            confidence: Some(best.score as f64 / 100.0),
                            payload: serde_json::json!({ "candidateId": best.id }),
                        },
                    )
                    .await;
                }
            }
        }
        Err(error) => {
            let _ = crate::jobs::record_log(
                state,
                job_id,
                "warn",
                "metadata_search_failed",
                Some(&track_id.to_string()),
                None,
                &error.to_string(),
            )
            .await;
        }
    }
    if let Err(error) =
        crate::lyrics::search(state, crate::lyrics::LyricsSearchRequest { track_id }).await
    {
        let _ = crate::jobs::record_log(
            state,
            job_id,
            "warn",
            "lyrics_search_failed",
            Some(&track_id.to_string()),
            None,
            &error.to_string(),
        )
        .await;
    }
}

pub(super) async fn create_static_reviews(
    state: &AppState,
    track_id: Uuid,
    media_id: Uuid,
    library_id: Uuid,
    relative_path: &str,
) -> Result<(), AppError> {
    let (title, artists, album, has_artwork): (String, String, Option<String>, bool) =
        sqlx::query_as(
            r#"SELECT t.title,
                 COALESCE((SELECT GROUP_CONCAT(a.name, ';') FROM track_artists ta
                   JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id), ''),
                 al.title, mf.has_artwork
               FROM tracks t JOIN media_files mf ON mf.track_id = t.id
               LEFT JOIN albums al ON al.id = t.album_id
               WHERE t.id = ? AND mf.id = ?"#,
        )
        .bind(track_id)
        .bind(media_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if title.trim().is_empty()
        || title == "未命名曲目"
        || artists.is_empty()
        || artists.contains("未知艺术家")
        || album.as_deref().is_none_or(|value| value == "未分类")
    {
        upsert_issue(
            state,
            ReviewIssue {
                kind: "incomplete_tags",
                subject_key: media_id.to_string(),
                title: "标签信息不完整",
                detail: relative_path.to_owned(),
                track_id: Some(track_id),
                media_file_id: Some(media_id),
                library_id: Some(library_id),
                confidence: None,
                payload: serde_json::json!({}),
            },
        )
        .await?;
    }
    if !has_artwork {
        let candidate_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                 SELECT 1 FROM review_items
                 WHERE kind = 'missing_artwork' AND subject_key = ?
                   AND json_extract(payload_json, '$.candidateId') IS NOT NULL
               )"#,
        )
        .bind(media_id.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if !candidate_exists {
            upsert_issue(
                state,
                ReviewIssue {
                    kind: "missing_artwork",
                    subject_key: media_id.to_string(),
                    title: "缺少封面",
                    detail: relative_path.to_owned(),
                    track_id: Some(track_id),
                    media_file_id: Some(media_id),
                    library_id: Some(library_id),
                    confidence: None,
                    payload: serde_json::json!({}),
                },
            )
            .await?;
        }
    }
    let lyrics_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lyrics WHERE track_id = ? AND active = 1")
            .bind(track_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
    if lyrics_count == 0 {
        upsert_issue(
            state,
            ReviewIssue {
                kind: "missing_lyrics",
                subject_key: track_id.to_string(),
                title: "缺少歌词",
                detail: relative_path.to_owned(),
                track_id: Some(track_id),
                media_file_id: Some(media_id),
                library_id: Some(library_id),
                confidence: None,
                payload: serde_json::json!({}),
            },
        )
        .await?;
    }
    Ok(())
}
