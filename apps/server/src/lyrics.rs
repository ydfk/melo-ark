use std::path::{Path, PathBuf};

use chrono::Utc;
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::{ItemKey, Tag},
};
use regex::Regex;
use reqwest::header::REFERER;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsRecord {
    pub id: Uuid,
    pub track_id: Uuid,
    pub media_file_id: Option<Uuid>,
    pub provider_id: Option<String>,
    pub provider_item_id: Option<String>,
    pub format: String,
    pub language: Option<String>,
    pub content: String,
    pub translated_content: Option<String>,
    pub synced: bool,
    pub coverage_percent: i64,
    pub quality_score: i64,
    pub storage: String,
    pub external_path: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchRequest {
    pub track_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResponse {
    pub candidates: Vec<LyricsRecord>,
    pub failures: Vec<LyricsFailure>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsFailure {
    pub provider_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyLyricsRequest {
    #[serde(default)]
    pub job_id: Option<Uuid>,
    pub lyrics_id: Uuid,
    pub media_file_id: Uuid,
    pub mode: LyricsWriteMode,
    #[serde(default)]
    pub replace_existing: bool,
    pub confirmation: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum LyricsWriteMode {
    External,
    Embedded,
    Both,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LrcLine {
    pub timestamp_ms: i64,
    pub text: String,
}

#[derive(Debug, FromRow)]
struct MediaPath {
    id: Uuid,
    track_id: Uuid,
    library_id: Uuid,
    library_path: String,
    relative_path: String,
    writable: bool,
    duration_ms: Option<i64>,
}

pub async fn list(state: &AppState, track_id: Uuid) -> Result<Vec<LyricsRecord>, AppError> {
    sqlx::query_as::<_, LyricsRecord>(
        r#"SELECT id, track_id, media_file_id, provider_id,
      provider_item_id, format, language, content, translated_content, synced, coverage_percent,
      quality_score, storage, external_path, active FROM lyrics WHERE track_id = ?
      ORDER BY active DESC, quality_score DESC, created_at DESC"#,
    )
    .bind(track_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)
}

pub async fn search(
    state: &AppState,
    request: LyricsSearchRequest,
) -> Result<LyricsSearchResponse, AppError> {
    let media = load_track_media(state, request.track_id).await?;
    let mut candidates = Vec::new();
    let mut failures = Vec::new();
    for item in &media {
        let path = safe_path(item)?;
        let external = path.with_extension("lrc");
        if external.is_file() {
            match std::fs::read_to_string(&external) {
                Ok(content) => candidates.push(
                    persist(
                        state,
                        item,
                        "local_external",
                        None,
                        content,
                        "external",
                        Some(external.to_string_lossy().into_owned()),
                    )
                    .await?,
                ),
                Err(error) => failures.push(LyricsFailure {
                    provider_id: "local_external".to_owned(),
                    message: error.to_string(),
                }),
            }
        }
        if let Ok(tagged) = Probe::open(&path).and_then(|probe| probe.read())
            && let Some(content) = tagged
                .primary_tag()
                .or_else(|| tagged.first_tag())
                .and_then(|tag| tag.get_string(ItemKey::Lyrics))
            && !content.trim().is_empty()
        {
            candidates.push(
                persist(
                    state,
                    item,
                    "local_embedded",
                    None,
                    content.to_owned(),
                    "embedded",
                    None,
                )
                .await?,
            );
        }
    }
    let remote = sqlx::query_as::<_, (String, String)>("SELECT provider_id, provider_item_id FROM scrape_candidates WHERE track_id = ? AND provider_id IN ('qq', 'netease', 'kugou') ORDER BY score DESC")
        .bind(request.track_id).fetch_all(&state.pool).await.map_err(AppError::internal)?;
    let Some(media_item) = media.first() else {
        return Err(AppError::NotFound("曲目没有物理文件".to_owned()));
    };
    for (provider_id, item_id) in remote {
        match fetch_remote(state, &provider_id, &item_id).await {
            Ok(Some(content)) => candidates.push(
                persist(
                    state,
                    media_item,
                    &provider_id,
                    Some(item_id),
                    content,
                    "candidate",
                    None,
                )
                .await?,
            ),
            Ok(None) => {}
            Err(error) => failures.push(LyricsFailure {
                provider_id,
                message: error.to_string(),
            }),
        }
    }
    candidates.sort_by_key(|item| std::cmp::Reverse(item.quality_score));
    Ok(LyricsSearchResponse {
        candidates,
        failures,
    })
}

async fn fetch_remote(
    state: &AppState,
    provider: &str,
    id: &str,
) -> Result<Option<String>, AppError> {
    match provider {
        "netease" => {
            let value: serde_json::Value = state
                .http
                .get("https://music.163.com/api/song/lyric")
                .query(&[("id", id), ("lv", "-1"), ("tv", "-1")])
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await
                .map_err(AppError::internal)?
                .error_for_status()
                .map_err(AppError::internal)?
                .json()
                .await
                .map_err(AppError::internal)?;
            Ok(value
                .pointer("/lrc/lyric")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned))
        }
        "qq" => {
            let value: serde_json::Value = state
                .http
                .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
                .header(REFERER, "https://y.qq.com/")
                .query(&[("songmid", id), ("format", "json"), ("nobase64", "1")])
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await
                .map_err(AppError::internal)?
                .error_for_status()
                .map_err(AppError::internal)?
                .json()
                .await
                .map_err(AppError::internal)?;
            Ok(value["lyric"].as_str().map(str::to_owned))
        }
        "kugou" => {
            let value: serde_json::Value = state
                .http
                .get("https://lyrics.kugou.com/search")
                .query(&[("ver", "1"), ("man", "yes"), ("client", "pc"), ("hash", id)])
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await
                .map_err(AppError::internal)?
                .error_for_status()
                .map_err(AppError::internal)?
                .json()
                .await
                .map_err(AppError::internal)?;
            let Some(candidate) = value["candidates"]
                .as_array()
                .and_then(|items| items.first())
            else {
                return Ok(None);
            };
            let lyric_id = candidate["id"].as_str().unwrap_or_default();
            let accesskey = candidate["accesskey"].as_str().unwrap_or_default();
            if lyric_id.is_empty() || accesskey.is_empty() {
                return Ok(None);
            }
            let body: serde_json::Value = state
                .http
                .get("https://lyrics.kugou.com/download")
                .query(&[
                    ("ver", "1"),
                    ("client", "pc"),
                    ("id", lyric_id),
                    ("accesskey", accesskey),
                    ("fmt", "lrc"),
                    ("charset", "utf8"),
                ])
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await
                .map_err(AppError::internal)?
                .error_for_status()
                .map_err(AppError::internal)?
                .json()
                .await
                .map_err(AppError::internal)?;
            let decoded = body["content"]
                .as_str()
                .and_then(|content| {
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content).ok()
                })
                .and_then(|bytes| String::from_utf8(bytes).ok());
            Ok(decoded)
        }
        _ => Ok(None),
    }
}

async fn persist(
    state: &AppState,
    media: &MediaPath,
    provider_id: &str,
    provider_item_id: Option<String>,
    content: String,
    storage: &str,
    external_path: Option<String>,
) -> Result<LyricsRecord, AppError> {
    let lines = parse_lrc(&content)?;
    let (quality, coverage) = quality_score(&content, &lines, media.duration_ms);
    let id = Uuid::new_v4();
    let now = Utc::now();
    let synced = !lines.is_empty();
    let format = if synced { "lrc" } else { "plain" };
    sqlx::query(
        r#"INSERT INTO lyrics (id, track_id, media_file_id, provider_id, provider_item_id,
      format, content, synced, coverage_percent, quality_score, storage, external_path, active,
      created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)"#,
    )
    .bind(id)
    .bind(media.track_id)
    .bind(media.id)
    .bind(provider_id)
    .bind(provider_item_id)
    .bind(format)
    .bind(&content)
    .bind(synced)
    .bind(coverage)
    .bind(quality)
    .bind(storage)
    .bind(external_path)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    fetch_one(state, id).await
}

pub async fn apply(
    state: &AppState,
    request: ApplyLyricsRequest,
) -> Result<LyricsRecord, AppError> {
    if request.confirmation != "USE_LYRICS" {
        return Err(AppError::BadRequest("需要显式确认 USE_LYRICS".to_owned()));
    }
    let job_id = request.job_id.unwrap_or_else(Uuid::new_v4);
    let item_key = request.media_file_id.to_string();
    let track_id: Uuid = sqlx::query_scalar("SELECT track_id FROM lyrics WHERE id = ?")
        .bind(request.lyrics_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::NotFound("歌词候选不存在".to_owned()))?;
    let item_id = crate::jobs::start_single_item_job(
        state,
        job_id,
        "lyrics",
        &item_key,
        "track",
        &track_id.to_string(),
    )
    .await?;
    let request_json = serde_json::to_string(&request).map_err(AppError::internal)?;
    if let Err(error) =
        sqlx::query("INSERT INTO lyrics_jobs (job_id, request_json, created_at) VALUES (?, ?, ?)")
            .bind(job_id)
            .bind(request_json)
            .bind(Utc::now())
            .execute(&state.pool)
            .await
    {
        let error = AppError::internal(error);
        let message = error.to_string();
        crate::jobs::finish_single_item_job(state, job_id, item_id, Some(&message)).await?;
        return Err(error);
    }
    run_apply_job(state, job_id, item_id, request).await
}

pub async fn retry_job(state: &AppState, id: Uuid) -> Result<(), AppError> {
    let request_json =
        sqlx::query_scalar::<_, String>("SELECT request_json FROM lyrics_jobs WHERE job_id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::internal)?
            .ok_or_else(|| AppError::NotFound("歌词任务请求不存在".to_owned()))?;
    let request: ApplyLyricsRequest =
        serde_json::from_str(&request_json).map_err(AppError::internal)?;
    let item_key = request.media_file_id.to_string();
    let track_id: Uuid = sqlx::query_scalar("SELECT track_id FROM lyrics WHERE id = ?")
        .bind(request.lyrics_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    let item_id = crate::jobs::start_single_item_job(
        state,
        id,
        "lyrics",
        &item_key,
        "track",
        &track_id.to_string(),
    )
    .await?;
    run_apply_job(state, id, item_id, request).await?;
    Ok(())
}

async fn run_apply_job(
    state: &AppState,
    job_id: Uuid,
    item_id: Uuid,
    request: ApplyLyricsRequest,
) -> Result<LyricsRecord, AppError> {
    let result = apply_confirmed(state, request).await;
    match result {
        Ok(record) => {
            crate::jobs::finish_single_item_job(state, job_id, item_id, None).await?;
            Ok(record)
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(job_error) =
                crate::jobs::finish_single_item_job(state, job_id, item_id, Some(&message)).await
            {
                tracing::error!(job_id = %job_id, error = %job_error, "歌词失败项状态写入失败");
            }
            Err(error)
        }
    }
}

async fn apply_confirmed(
    state: &AppState,
    request: ApplyLyricsRequest,
) -> Result<LyricsRecord, AppError> {
    let lyric = fetch_one(state, request.lyrics_id).await?;
    let media = load_media(state, request.media_file_id).await?;
    if lyric.track_id != media.track_id {
        return Err(AppError::BadRequest("歌词候选与目标曲目不匹配".to_owned()));
    }
    if !media.writable {
        return Err(AppError::BadRequest("曲库未允许写入".to_owned()));
    }
    let path = safe_path(&media)?;
    let external_path = path.with_extension("lrc");
    let writes_external = matches!(
        request.mode,
        LyricsWriteMode::External | LyricsWriteMode::Both
    );
    let writes_embedded = matches!(
        request.mode,
        LyricsWriteMode::Embedded | LyricsWriteMode::Both
    );
    if writes_external && external_path.exists() && !request.replace_existing {
        return Err(AppError::Conflict(
            "外置 LRC 已存在；请明确选择替换".to_owned(),
        ));
    }
    if writes_embedded && embedded_lyrics(&path)?.is_some() && !request.replace_existing {
        return Err(AppError::Conflict(
            "内嵌歌词已存在；请明确选择替换".to_owned(),
        ));
    }
    if writes_external {
        std::fs::write(&external_path, lyric.content.as_bytes()).map_err(AppError::internal)?;
    }
    if writes_embedded {
        write_embedded(&path, &lyric.content)?;
        sqlx::query("UPDATE media_files SET mtime_ms = -1 WHERE id = ?")
            .bind(media.id)
            .execute(&state.pool)
            .await
            .map_err(AppError::internal)?;
    }
    let storage = match request.mode {
        LyricsWriteMode::External => "external",
        LyricsWriteMode::Embedded => "embedded",
        LyricsWriteMode::Both => "both",
    };
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("UPDATE lyrics SET active = 0, updated_at = ? WHERE track_id = ?")
        .bind(Utc::now())
        .bind(lyric.track_id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("UPDATE lyrics SET active = 1, storage = ?, media_file_id = ?, external_path = ?, updated_at = ? WHERE id = ?")
      .bind(storage).bind(media.id).bind(writes_external.then(|| external_path.to_string_lossy().into_owned())).bind(Utc::now()).bind(lyric.id)
      .execute(&mut *transaction).await.map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    if writes_embedded {
        let _ = crate::scanner::enqueue_scan(state.clone(), media.library_id).await;
    }
    fetch_one(state, lyric.id).await
}

fn embedded_lyrics(path: &Path) -> Result<Option<String>, AppError> {
    let tagged = Probe::open(path)
        .and_then(|probe| probe.read())
        .map_err(|error| AppError::BadRequest(format!("Tag 读取失败：{error}")))?;
    Ok(tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .and_then(|tag| tag.get_string(ItemKey::Lyrics))
        .map(str::to_owned))
}

fn write_embedded(path: &Path, content: &str) -> Result<(), AppError> {
    let mut tagged = Probe::open(path)
        .and_then(|probe| probe.read())
        .map_err(|error| AppError::BadRequest(format!("Tag 读取失败：{error}")))?;
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(Tag::new(tagged.primary_tag_type()));
    }
    tagged
        .primary_tag_mut()
        .ok_or_else(|| AppError::BadRequest("该音频格式不能创建主 Tag".to_owned()))?
        .insert_text(ItemKey::Lyrics, content.to_owned());
    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|error| AppError::BadRequest(format!("歌词写入失败：{error}")))
}

async fn fetch_one(state: &AppState, id: Uuid) -> Result<LyricsRecord, AppError> {
    sqlx::query_as::<_, LyricsRecord>(
        r#"SELECT id, track_id, media_file_id, provider_id,
      provider_item_id, format, language, content, translated_content, synced, coverage_percent,
      quality_score, storage, external_path, active FROM lyrics WHERE id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("歌词不存在".to_owned()))
}

async fn load_track_media(state: &AppState, track_id: Uuid) -> Result<Vec<MediaPath>, AppError> {
    sqlx::query_as::<_, MediaPath>(r#"SELECT mf.id, mf.track_id, mf.library_id, l.path AS library_path,
      mf.relative_path, l.writable, mf.duration_ms FROM media_files mf JOIN libraries l ON l.id = mf.library_id
      WHERE mf.track_id = ? ORDER BY mf.file_size DESC"#).bind(track_id).fetch_all(&state.pool).await.map_err(AppError::internal)
}
async fn load_media(state: &AppState, id: Uuid) -> Result<MediaPath, AppError> {
    sqlx::query_as::<_, MediaPath>(r#"SELECT mf.id, mf.track_id, mf.library_id, l.path AS library_path,
      mf.relative_path, l.writable, mf.duration_ms FROM media_files mf JOIN libraries l ON l.id = mf.library_id WHERE mf.id = ?"#)
      .bind(id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(|| AppError::NotFound("媒体文件不存在".to_owned()))
}
fn safe_path(media: &MediaPath) -> Result<PathBuf, AppError> {
    let root = Path::new(&media.library_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    let path = root
        .join(&media.relative_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    if !path.starts_with(root) {
        return Err(AppError::BadRequest("媒体路径超出曲库范围".to_owned()));
    }
    Ok(path)
}

pub fn parse_lrc(content: &str) -> Result<Vec<LrcLine>, AppError> {
    let timestamp =
        Regex::new(r"\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]").map_err(AppError::internal)?;
    let mut lines = Vec::new();
    for raw in content.lines() {
        let text = timestamp.replace_all(raw, "").trim().to_owned();
        for capture in timestamp.captures_iter(raw) {
            let minutes: i64 = capture[1].parse().unwrap_or(0);
            let seconds: i64 = capture[2].parse().unwrap_or(0);
            let fraction = capture
                .get(3)
                .map_or(0, |value| match value.as_str().len() {
                    1 => value.as_str().parse::<i64>().unwrap_or(0) * 100,
                    2 => value.as_str().parse::<i64>().unwrap_or(0) * 10,
                    _ => value.as_str().parse::<i64>().unwrap_or(0),
                });
            if seconds < 60 && !text.is_empty() {
                lines.push(LrcLine {
                    timestamp_ms: minutes * 60_000 + seconds * 1000 + fraction,
                    text: text.clone(),
                });
            }
        }
    }
    lines.sort_by_key(|line| line.timestamp_ms);
    Ok(lines)
}

pub fn quality_score(content: &str, lines: &[LrcLine], duration_ms: Option<i64>) -> (i64, i64) {
    if content.trim().is_empty() {
        return (0, 0);
    }
    let synced = !lines.is_empty();
    let last = lines.last().map_or(0, |line| line.timestamp_ms);
    let coverage = duration_ms
        .filter(|value| *value > 0)
        .map_or(0, |duration| {
            ((last as f64 / duration as f64) * 100.0)
                .round()
                .clamp(0.0, 100.0) as i64
        });
    let mut score = if synced { 45 } else { 15 };
    score += (coverage as f64 * 0.3).round() as i64;
    if (8..=300).contains(&lines.len()) {
        score += 15;
    }
    let bilingual = lines
        .windows(2)
        .any(|pair| pair[0].timestamp_ms == pair[1].timestamp_ms);
    if bilingual {
        score += 10;
    }
    if duration_ms.is_some_and(|duration| last > duration + 30_000) {
        score -= 35;
    }
    (score.clamp(0, 100), coverage)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_multilingual_lrc_and_scores_coverage() {
        let content = "[00:00.00]晴天\n[00:00.00]Sunny day\n[03:58.50]故事的小黄花";
        let lines = parse_lrc(content).expect("解析 LRC");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].timestamp_ms, 0);
        let (score, coverage) = quality_score(content, &lines, Some(240_000));
        assert!(score >= 80);
        assert_eq!(coverage, 99);
    }
    #[test]
    fn rejects_out_of_range_seconds() {
        let lines = parse_lrc("[01:99.00]bad").expect("解析 LRC");
        assert!(lines.is_empty());
    }
}
