use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    jobs::JobResponse,
    state::AppState,
    text_normalization::{compact_match_key, version_mismatch},
};

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    #[serde(default)]
    pub media_ids: Vec<Uuid>,
    #[serde(default = "yes")]
    pub calculate_hash: bool,
    #[serde(default = "yes")]
    pub calculate_fingerprint: bool,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupQuery {
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: Uuid,
    pub kind: String,
    pub confidence: i64,
    pub reclaimable_bytes: i64,
    pub reason: String,
    pub members: Vec<DuplicateMember>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMember {
    pub media_file_id: Uuid,
    pub track_id: Uuid,
    pub title: String,
    pub version_label: Option<String>,
    pub artist: String,
    pub path: String,
    pub extension: String,
    pub file_size: i64,
    pub device_id: String,
    pub inode: String,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub duration_ms: Option<i64>,
    pub has_artwork: bool,
    pub similarity: Option<f64>,
    pub quality_score: i64,
    pub recommended_keep: bool,
}

#[derive(Debug, Clone, FromRow)]
struct AnalysisTarget {
    id: Uuid,
    track_id: Uuid,
    library_path: String,
    relative_path: String,
    extension: String,
    file_size: i64,
    device_id: String,
    inode: String,
    codec: Option<String>,
    bitrate: Option<i64>,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
    channels: Option<i64>,
    duration_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct AnalyzedMedia {
    target: AnalysisTarget,
    full_hash: Option<String>,
    fingerprint: Option<Vec<u32>>,
    fingerprint_duration_ms: Option<i64>,
    quality_score: i64,
}

type PendingMember = (Uuid, Option<f64>, i64);
type PendingGroup = (String, i64, i64, String, Vec<PendingMember>);

pub async fn create_job(
    state: &AppState,
    request: AnalyzeRequest,
) -> Result<JobResponse, AppError> {
    if !request.calculate_hash && !request.calculate_fingerprint {
        return Err(AppError::BadRequest(
            "Hash 与 Fingerprint 至少选择一项".to_owned(),
        ));
    }
    let targets = load_targets(state, &request.media_ids).await?;
    if targets.is_empty() {
        return Err(AppError::BadRequest("没有可分析的媒体文件".to_owned()));
    }
    let mut physical = HashSet::new();
    let targets: Vec<_> = targets
        .into_iter()
        .filter(|item| physical.insert((item.device_id.clone(), item.inode.clone())))
        .collect();
    let id = Uuid::new_v4();
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("INSERT INTO jobs (id, kind, status, source_type, source_id, total_items, created_at, updated_at) VALUES (?, 'analyze', 'queued', 'workspace', 'duplicates', ?, ?, ?)")
        .bind(id).bind(i64::try_from(targets.len()).unwrap_or(i64::MAX)).bind(now).bind(now)
        .execute(&mut *transaction).await.map_err(AppError::internal)?;
    sqlx::query("INSERT INTO analysis_jobs (job_id, calculate_hash, calculate_fingerprint, created_at) VALUES (?, ?, ?, ?)")
        .bind(id).bind(request.calculate_hash).bind(request.calculate_fingerprint).bind(now)
        .execute(&mut *transaction).await.map_err(AppError::internal)?;
    for target in targets {
        sqlx::query("INSERT INTO job_items (id, job_id, item_key, status, retryable, updated_at) VALUES (?, ?, ?, 'pending', 1, ?)")
            .bind(Uuid::new_v4()).bind(id).bind(target.id.to_string()).bind(now)
            .execute(&mut *transaction).await.map_err(AppError::internal)?;
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
        "重复文件分析已加入队列",
    )
    .await?;
    crate::jobs::emit(state, job.clone());
    spawn_job(state.clone(), id);
    Ok(job)
}

pub fn spawn_job(state: AppState, id: Uuid) {
    tokio::spawn(async move {
        if let Err(error) = run_job(&state, id).await {
            let _ = sqlx::query("UPDATE jobs SET status = 'failed', error_message = ?, finished_at = ?, updated_at = ? WHERE id = ?")
                .bind(error.to_string()).bind(Utc::now()).bind(Utc::now()).bind(id).execute(&state.pool).await;
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

pub async fn retry_job(state: &AppState, id: Uuid) -> Result<(), AppError> {
    let now = Utc::now();
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("UPDATE job_items SET status = 'pending', message = NULL, error_code = NULL, updated_at = ? WHERE job_id = ? AND status = 'failed'")
        .bind(now).bind(id).execute(&mut *transaction).await.map_err(AppError::internal)?;
    sqlx::query("UPDATE jobs SET status = 'queued', processed_items = success_items + skipped_items, failed_items = 0, error_message = NULL, finished_at = NULL, updated_at = ? WHERE id = ?")
        .bind(now).bind(id).execute(&mut *transaction).await.map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    spawn_job(state.clone(), id);
    Ok(())
}

async fn run_job(state: &AppState, id: Uuid) -> Result<(), AppError> {
    let _permit = state
        .analysis_semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(AppError::internal)?;
    sqlx::query("UPDATE jobs SET status = 'running', started_at = COALESCE(started_at, ?), finished_at = NULL, updated_at = ? WHERE id = ? AND status IN ('queued','interrupted')")
        .bind(Utc::now()).bind(Utc::now()).bind(id).execute(&state.pool).await.map_err(AppError::internal)?;
    crate::jobs::record_log(
        state,
        id,
        "info",
        "started",
        None,
        None,
        "重复文件分析开始执行",
    )
    .await?;
    let (calculate_hash, calculate_fingerprint): (bool, bool) = sqlx::query_as(
        "SELECT calculate_hash, calculate_fingerprint FROM analysis_jobs WHERE job_id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let items = sqlx::query_as::<_, (Uuid, String)>("SELECT id, item_key FROM job_items WHERE job_id = ? AND status IN ('pending','failed') ORDER BY rowid")
        .bind(id).fetch_all(&state.pool).await.map_err(AppError::internal)?;
    for (item_id, item_key) in items {
        if !wait_runnable(state, id).await? {
            return Ok(());
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
            "开始分析文件",
        )
        .await?;
        let result = match Uuid::parse_str(&item_key) {
            Ok(media_id) => {
                analyze_one(state, media_id, calculate_hash, calculate_fingerprint).await
            }
            Err(error) => Err(AppError::internal(error)),
        };
        let (status, message) = match result {
            Ok(()) => ("success", None),
            Err(error) => ("failed", Some(error.to_string())),
        };
        sqlx::query("UPDATE job_items SET status = ?, message = ?, retryable = ?, updated_at = ? WHERE id = ?")
            .bind(status).bind(&message).bind(status == "failed").bind(Utc::now()).bind(item_id).execute(&state.pool).await.map_err(AppError::internal)?;
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
        update_job_progress(state, id, &item_key).await?;
    }
    rebuild_groups(state).await?;
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
    crate::jobs::record_log(state, id, "info", status, None, None, "重复文件分析完成").await?;
    Ok(())
}

async fn wait_runnable(state: &AppState, id: Uuid) -> Result<bool, AppError> {
    loop {
        let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
        match status.as_str() {
            "paused" => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            "cancel_requested" => {
                sqlx::query("UPDATE jobs SET status = 'cancelled', finished_at = ?, updated_at = ? WHERE id = ?")
                    .bind(Utc::now()).bind(Utc::now()).bind(id).execute(&state.pool).await.map_err(AppError::internal)?;
                crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, id).await?);
                crate::jobs::record_log(
                    state,
                    id,
                    "warn",
                    "cancelled",
                    None,
                    None,
                    "重复文件分析已取消",
                )
                .await?;
                return Ok(false);
            }
            _ => return Ok(true),
        }
    }
}

async fn update_job_progress(state: &AppState, id: Uuid, item: &str) -> Result<(), AppError> {
    sqlx::query(r#"UPDATE jobs SET current_item = ?, processed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status IN ('success','skipped','failed')),
      success_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'success'), failed_items = (SELECT COUNT(*) FROM job_items WHERE job_id = ? AND status = 'failed'), updated_at = ? WHERE id = ?"#)
      .bind(item).bind(id).bind(id).bind(id).bind(Utc::now()).bind(id).execute(&state.pool).await.map_err(AppError::internal)?;
    crate::jobs::emit(state, crate::jobs::fetch_job(&state.pool, id).await?);
    Ok(())
}

async fn analyze_one(
    state: &AppState,
    media_id: Uuid,
    calculate_hash: bool,
    calculate_fingerprint: bool,
) -> Result<(), AppError> {
    let target = load_target(state, media_id).await?;
    let path = safe_path(&target)?;
    let hash = if calculate_hash {
        Some(hash_file(path.clone()).await?)
    } else {
        None
    };
    let fingerprint = if calculate_fingerprint {
        Some(run_fpcalc(state, &path).await?)
    } else {
        None
    };
    let quality = quality_score(&target);
    let now = Utc::now();
    let fingerprint_json = fingerprint
        .as_ref()
        .map(|item| serde_json::to_string(&item.0).unwrap_or_default());
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query(r#"UPDATE media_files SET full_hash = COALESCE(?, full_hash), hash_status = CASE WHEN ? THEN 'completed' ELSE hash_status END,
      fingerprint_json = COALESCE(?, fingerprint_json), fingerprint_duration_ms = COALESCE(?, fingerprint_duration_ms), fingerprint_status = CASE WHEN ? THEN 'completed' ELSE fingerprint_status END,
      quality_score = ?, analysis_error = NULL, updated_at = ? WHERE device_id = ? AND inode = ?"#)
      .bind(hash.as_deref()).bind(calculate_hash)
      .bind(fingerprint_json.as_deref())
      .bind(fingerprint.as_ref().map(|item| item.1)).bind(calculate_fingerprint)
      .bind(quality).bind(now).bind(&target.device_id).bind(&target.inode)
      .execute(&mut *transaction).await.map_err(AppError::internal)?;
    let aliases = sqlx::query_as::<_, (Uuid, i64, i64)>(
        "SELECT id, file_size, mtime_ms FROM media_files WHERE device_id = ? AND inode = ?",
    )
    .bind(&target.device_id)
    .bind(&target.inode)
    .fetch_all(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    for (alias_id, file_size, mtime_ms) in aliases {
        if let Some(hash) = hash.as_deref() {
            sqlx::query(
                r#"INSERT INTO audio_hashes
                  (media_file_id, blake3, calculated_at, source_size, source_mtime)
                  VALUES (?, ?, ?, ?, ?)
                  ON CONFLICT(media_file_id) DO UPDATE SET blake3=excluded.blake3,
                    calculated_at=excluded.calculated_at, source_size=excluded.source_size,
                    source_mtime=excluded.source_mtime"#,
            )
            .bind(alias_id)
            .bind(hash)
            .bind(now)
            .bind(file_size)
            .bind(mtime_ms)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
        }
        if let Some((_, duration_ms)) = fingerprint.as_ref() {
            sqlx::query(
                r#"INSERT INTO audio_fingerprints
                  (media_file_id, algorithm, fingerprint, duration_ms, calculated_at)
                  VALUES (?, 'chromaprint', ?, ?, ?)
                  ON CONFLICT(media_file_id) DO UPDATE SET algorithm=excluded.algorithm,
                    fingerprint=excluded.fingerprint, duration_ms=excluded.duration_ms,
                    calculated_at=excluded.calculated_at"#,
            )
            .bind(alias_id)
            .bind(fingerprint_json.as_deref().unwrap_or("[]"))
            .bind(duration_ms)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
        }
    }
    transaction.commit().await.map_err(AppError::internal)?;
    Ok(())
}

async fn hash_file(path: PathBuf) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        let mut file = File::open(path).map_err(AppError::internal)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(AppError::internal)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize().to_hex().to_string())
    })
    .await
    .map_err(AppError::internal)?
}

async fn run_fpcalc(state: &AppState, path: &Path) -> Result<(Vec<u32>, i64), AppError> {
    let output = tokio::process::Command::new(&state.analysis.fpcalc_path)
        .arg("-raw")
        .arg("-json")
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| AppError::BadRequest(format!("fpcalc 不可用：{error}")))?;
    // Debian 的 fpcalc 在部分有效音频末尾会返回非零退出码，但 stdout 仍包含完整指纹。
    // 优先验证机器可读结果，只有没有可用 JSON/指纹时才按进程失败处理。
    let value: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(value) => value,
        Err(_) if !output.status.success() => {
            return Err(AppError::BadRequest(format!(
                "fpcalc 失败：{}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Err(error) => {
            return Err(AppError::BadRequest(format!("fpcalc JSON 无效：{error}")));
        }
    };
    let fingerprint: Vec<u32> = if let Some(items) = value["fingerprint"].as_array() {
        items
            .iter()
            .filter_map(serde_json::Value::as_u64)
            .filter_map(|item| u32::try_from(item).ok())
            .collect()
    } else {
        value["fingerprint"]
            .as_str()
            .unwrap_or_default()
            .split(',')
            .filter_map(|item| item.trim().parse().ok())
            .collect()
    };
    if fingerprint.is_empty() {
        if !output.status.success() {
            return Err(AppError::BadRequest(format!(
                "fpcalc 失败：{}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        return Err(AppError::BadRequest("fpcalc 没有返回指纹".to_owned()));
    }
    let duration_ms = value["duration"]
        .as_f64()
        .map(|item| (item * 1000.0).round() as i64)
        .unwrap_or_default();
    Ok((fingerprint, duration_ms))
}

fn quality_score(item: &AnalysisTarget) -> i64 {
    let codec = item
        .codec
        .as_deref()
        .unwrap_or(&item.extension)
        .to_ascii_lowercase();
    let mut score = if matches!(codec.as_str(), "flac" | "alac" | "wav" | "aiff") {
        55
    } else if matches!(codec.as_str(), "mp3" | "aac" | "opus" | "vorbis") {
        30
    } else {
        20
    };
    score += match item.bit_depth.unwrap_or_default() {
        24.. => 18,
        16..=23 => 12,
        _ => 4,
    };
    score += match item.sample_rate.unwrap_or_default() {
        88_200.. => 15,
        44_100.. => 10,
        22_050.. => 5,
        _ => 0,
    };
    score += match item.bitrate.unwrap_or_default() {
        900_000.. => 10,
        320_000.. => 8,
        192_000.. => 5,
        96_000.. => 2,
        _ => 0,
    };
    if item.channels.unwrap_or(2) >= 2 {
        score += 2;
    }
    score.clamp(0, 100)
}

pub async fn rebuild_groups(state: &AppState) -> Result<(), AppError> {
    let fingerprint_threshold = state.runtime.read().await.editable.fingerprint_threshold;
    let targets = load_targets(state, &[]).await?;
    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, Option<i64>, Option<i64>)>("SELECT id, full_hash, fingerprint_json, fingerprint_duration_ms, quality_score FROM media_files")
        .fetch_all(&state.pool).await.map_err(AppError::internal)?;
    let lookup: HashMap<Uuid, _> = rows.into_iter().map(|row| (row.0, row)).collect();
    let media: Vec<AnalyzedMedia> = targets
        .into_iter()
        .map(|target| {
            let row = lookup.get(&target.id);
            AnalyzedMedia {
                quality_score: row
                    .and_then(|item| item.4)
                    .unwrap_or_else(|| quality_score(&target)),
                full_hash: row.and_then(|item| item.1.clone()),
                fingerprint: row
                    .and_then(|item| item.2.as_deref())
                    .and_then(|json| serde_json::from_str(json).ok()),
                fingerprint_duration_ms: row.and_then(|item| item.3),
                target,
            }
        })
        .collect();
    let mut groups: Vec<PendingGroup> = Vec::new();
    group_by_key(
        &media,
        |item| Some(format!("{}:{}", item.target.device_id, item.target.inode)),
        2,
    )
    .into_iter()
    .for_each(|items| {
        groups.push((
            "hardlink_alias".to_owned(),
            100,
            0,
            "相同 device + inode；这些路径共享同一个物理文件，不计为空间浪费".to_owned(),
            members(&items, None),
        ));
    });
    group_by_key(&media, |item| item.full_hash.clone(), 2)
        .into_iter()
        .filter(|items| distinct_physical(items) > 1)
        .for_each(|items| {
            let reclaim = reclaimable(&items);
            groups.push((
                "binary_exact".to_owned(),
                100,
                reclaim,
                "BLAKE3 全文件 Hash 完全一致".to_owned(),
                members(&items, None),
            ));
        });
    group_by_key(&media, |item| Some(item.target.track_id.to_string()), 2)
        .into_iter()
        .filter(|items| distinct_physical(items) > 1)
        .for_each(|items| {
            groups.push((
                "quality_variant".to_owned(),
                92,
                0,
                "同一逻辑 Track 的不同技术规格；Quality Score 只比较编码参数，不代表听感"
                    .to_owned(),
                members(&items, None),
            ));
        });
    for left in 0..media.len() {
        for right in (left + 1)..media.len() {
            let a = &media[left];
            let b = &media[right];
            if physical_key(a) == physical_key(b)
                || a.full_hash.is_some() && a.full_hash == b.full_hash
            {
                continue;
            }
            if let (Some(af), Some(bf)) = (&a.fingerprint, &b.fingerprint) {
                let duration_diff = (a.fingerprint_duration_ms.unwrap_or_default()
                    - b.fingerprint_duration_ms.unwrap_or_default())
                .abs();
                let similarity = fingerprint_similarity(af, bf);
                if similarity >= fingerprint_threshold && duration_diff <= 2_500 {
                    groups.push((
                        "audio_duplicate".to_owned(),
                        (similarity * 100.0).round() as i64,
                        0,
                        format!(
                            "Chromaprint 相似度 {:.1}%，时长差 {} ms",
                            similarity * 100.0,
                            duration_diff
                        ),
                        members(&[a, b], Some(similarity)),
                    ));
                }
            }
        }
    }
    add_possible_groups(&media, &mut groups);
    persist_groups(state, groups).await
}

fn group_by_key<F>(media: &[AnalyzedMedia], key: F, minimum: usize) -> Vec<Vec<&AnalyzedMedia>>
where
    F: Fn(&AnalyzedMedia) -> Option<String>,
{
    let mut grouped: HashMap<String, Vec<&AnalyzedMedia>> = HashMap::new();
    for item in media {
        if let Some(key) = key(item) {
            grouped.entry(key).or_default().push(item);
        }
    }
    grouped
        .into_values()
        .filter(|items| items.len() >= minimum)
        .collect()
}
fn physical_key(item: &AnalyzedMedia) -> (&str, &str) {
    (&item.target.device_id, &item.target.inode)
}
fn distinct_physical(items: &[&AnalyzedMedia]) -> usize {
    items
        .iter()
        .map(|item| physical_key(item))
        .collect::<HashSet<_>>()
        .len()
}
fn reclaimable(items: &[&AnalyzedMedia]) -> i64 {
    let mut seen = HashSet::new();
    let mut sizes: Vec<_> = items
        .iter()
        .filter(|item| seen.insert(physical_key(item)))
        .map(|item| item.target.file_size)
        .collect();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes.into_iter().skip(1).sum()
}
fn members(items: &[&AnalyzedMedia], similarity: Option<f64>) -> Vec<(Uuid, Option<f64>, i64)> {
    items
        .iter()
        .map(|item| (item.target.id, similarity, item.quality_score))
        .collect()
}

fn add_possible_groups(media: &[AnalyzedMedia], groups: &mut Vec<PendingGroup>) {
    for left in 0..media.len() {
        for right in (left + 1)..media.len() {
            let a = &media[left];
            let b = &media[right];
            if a.target.track_id == b.target.track_id || physical_key(a) == physical_key(b) {
                continue;
            }
            let title_a = Path::new(&a.target.relative_path)
                .file_stem()
                .and_then(|item| item.to_str())
                .unwrap_or_default();
            let title_b = Path::new(&b.target.relative_path)
                .file_stem()
                .and_then(|item| item.to_str())
                .unwrap_or_default();
            let similarity = strsim::normalized_levenshtein(
                &compact_match_key(title_a),
                &compact_match_key(title_b),
            );
            let duration_ok = match (a.target.duration_ms, b.target.duration_ms) {
                (Some(x), Some(y)) => (x - y).abs() <= 5_000,
                _ => false,
            };
            if similarity >= 0.86 && duration_ok {
                let mismatch = version_mismatch(title_a, title_b);
                let confidence =
                    ((similarity * 100.0) as i64 - if mismatch { 35 } else { 0 }).max(1);
                groups.push((
                    "possible_duplicate".to_owned(),
                    confidence,
                    0,
                    if mismatch {
                        "文件名相似但版本关键词不同；必须人工判断".to_owned()
                    } else {
                        "文件名与时长相似，缺少足够音频证据".to_owned()
                    },
                    members(&[a, b], Some(similarity)),
                ));
            }
        }
    }
}
pub fn fingerprint_similarity(left: &[u32], right: &[u32]) -> f64 {
    let length = left.len().min(right.len());
    if length == 0 {
        return 0.0;
    }
    let differing: u32 = left
        .iter()
        .zip(right)
        .take(length)
        .map(|(a, b)| (a ^ b).count_ones())
        .sum();
    1.0 - f64::from(differing) / (length as f64 * 32.0)
}

async fn persist_groups(state: &AppState, groups: Vec<PendingGroup>) -> Result<(), AppError> {
    let now = Utc::now();
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("DELETE FROM duplicate_groups")
        .execute(&mut *tx)
        .await
        .map_err(AppError::internal)?;
    for (kind, confidence, reclaimable_bytes, reason, mut members) in groups {
        members.sort_by_key(|item| std::cmp::Reverse(item.2));
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO duplicate_groups (id, kind, confidence, reclaimable_bytes, reason, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(id).bind(kind).bind(confidence.clamp(0,100)).bind(reclaimable_bytes).bind(reason).bind(now).bind(now).execute(&mut *tx).await.map_err(AppError::internal)?;
        for (index, (media_id, similarity, quality)) in members.into_iter().enumerate() {
            sqlx::query("INSERT INTO duplicate_group_members (group_id, media_file_id, similarity, quality_score, recommended_keep) VALUES (?, ?, ?, ?, ?)")
                .bind(id).bind(media_id).bind(similarity).bind(quality).bind(index == 0).execute(&mut *tx).await.map_err(AppError::internal)?;
        }
    }
    tx.commit().await.map_err(AppError::internal)?;
    Ok(())
}

pub async fn list_groups(
    state: &AppState,
    kind: Option<&str>,
) -> Result<Vec<DuplicateGroup>, AppError> {
    let group_rows = if let Some(kind) = kind {
        sqlx::query_as::<_, (Uuid,String,i64,i64,String)>("SELECT id, kind, confidence, reclaimable_bytes, reason FROM duplicate_groups WHERE kind = ? ORDER BY reclaimable_bytes DESC, confidence DESC").bind(kind).fetch_all(&state.pool).await
    } else { sqlx::query_as::<_, (Uuid,String,i64,i64,String)>("SELECT id, kind, confidence, reclaimable_bytes, reason FROM duplicate_groups ORDER BY reclaimable_bytes DESC, confidence DESC").fetch_all(&state.pool).await }.map_err(AppError::internal)?;
    let mut result = Vec::new();
    for row in group_rows {
        result.push(DuplicateGroup {
            id: row.0,
            kind: row.1,
            confidence: row.2,
            reclaimable_bytes: row.3,
            reason: row.4,
            members: load_members(state, row.0).await?,
        });
    }
    Ok(result)
}

pub async fn get_group(state: &AppState, id: Uuid) -> Result<DuplicateGroup, AppError> {
    let row = sqlx::query_as::<_, (Uuid, String, i64, i64, String)>(
        "SELECT id, kind, confidence, reclaimable_bytes, reason FROM duplicate_groups WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("重复组不存在".to_owned()))?;
    Ok(DuplicateGroup {
        id: row.0,
        kind: row.1,
        confidence: row.2,
        reclaimable_bytes: row.3,
        reason: row.4,
        members: load_members(state, row.0).await?,
    })
}

async fn load_members(state: &AppState, id: Uuid) -> Result<Vec<DuplicateMember>, AppError> {
    sqlx::query_as::<_, DuplicateMember>(r#"SELECT mf.id AS media_file_id, mf.track_id, t.title, t.version_label,
      COALESCE((SELECT GROUP_CONCAT(a.name, '; ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id), '未知艺术家') AS artist,
      l.path || '/' || mf.relative_path AS path, mf.extension, mf.file_size, mf.device_id, mf.inode,
      mf.codec, mf.bitrate, mf.sample_rate, mf.bit_depth, mf.duration_ms, mf.has_artwork, dgm.similarity,
      dgm.quality_score, dgm.recommended_keep FROM duplicate_group_members dgm JOIN media_files mf ON mf.id = dgm.media_file_id
      JOIN tracks t ON t.id = mf.track_id JOIN libraries l ON l.id = mf.library_id WHERE dgm.group_id = ? ORDER BY dgm.recommended_keep DESC, dgm.quality_score DESC"#)
      .bind(id).fetch_all(&state.pool).await.map_err(AppError::internal)
}

async fn load_targets(state: &AppState, ids: &[Uuid]) -> Result<Vec<AnalysisTarget>, AppError> {
    if ids.is_empty() {
        sqlx::query_as::<_, AnalysisTarget>(r#"SELECT mf.id, mf.track_id, l.path AS library_path, mf.relative_path, mf.extension, mf.file_size, mf.device_id, mf.inode, mf.codec, mf.bitrate, mf.sample_rate, mf.bit_depth, mf.channels, mf.duration_ms FROM media_files mf JOIN libraries l ON l.id = mf.library_id ORDER BY mf.id"#).fetch_all(&state.pool).await.map_err(AppError::internal)
    } else {
        let mut items = Vec::new();
        for id in ids {
            items.push(load_target(state, *id).await?);
        }
        Ok(items)
    }
}
async fn load_target(state: &AppState, id: Uuid) -> Result<AnalysisTarget, AppError> {
    sqlx::query_as::<_, AnalysisTarget>(r#"SELECT mf.id, mf.track_id, l.path AS library_path, mf.relative_path, mf.extension, mf.file_size, mf.device_id, mf.inode, mf.codec, mf.bitrate, mf.sample_rate, mf.bit_depth, mf.channels, mf.duration_ms FROM media_files mf JOIN libraries l ON l.id = mf.library_id WHERE mf.id = ?"#)
      .bind(id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(|| AppError::NotFound("媒体文件不存在".to_owned()))
}
fn safe_path(target: &AnalysisTarget) -> Result<PathBuf, AppError> {
    let root = Path::new(&target.library_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    let path = root
        .join(&target.relative_path)
        .canonicalize()
        .map_err(AppError::internal)?;
    if !path.starts_with(root) {
        return Err(AppError::BadRequest("媒体路径超出曲库范围".to_owned()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fingerprint_hamming_similarity_is_bounded() {
        assert_eq!(fingerprint_similarity(&[0, 0], &[0, 0]), 1.0);
        assert_eq!(fingerprint_similarity(&[0], &[u32::MAX]), 0.0);
    }
    #[test]
    fn versions_are_not_collapsed() {
        for label in ["Live", "Remix", "Remaster", "Instrumental", "伴奏"] {
            assert!(version_mismatch("晴天", &format!("晴天 {label}")));
        }
    }
}
