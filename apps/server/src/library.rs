use std::{
    fs::Metadata,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;

pub const DEFAULT_EXCLUDES: &[&str] = &["@eaDir", ".recycle", ".meloark-trash", "lost+found"];
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "mp4", "ogg", "opus", "wav", "aiff", "aif", "ape", "wma", "wv", "dsf",
    "dff",
];

#[derive(Clone, Debug, FromRow)]
pub struct LibraryRecord {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub scan_enabled: bool,
    pub watch_enabled: bool,
    pub writable: bool,
    pub role: String,
    pub target_library_id: Option<Uuid>,
    pub auto_ingest_enabled: bool,
    pub exclude_patterns: String,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySourceResponse {
    pub id: Uuid,
    pub source_path: String,
    pub scan_enabled: bool,
    pub watch_enabled: bool,
    pub auto_ingest_enabled: bool,
    pub exclude_patterns: Vec<String>,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<LibraryRecord> for LibrarySourceResponse {
    fn from(record: LibraryRecord) -> Self {
        let exclude_patterns = serde_json::from_str(&record.exclude_patterns).unwrap_or_default();
        Self {
            id: record.id,
            source_path: record.path,
            scan_enabled: record.scan_enabled,
            watch_enabled: record.watch_enabled,
            auto_ingest_enabled: record.auto_ingest_enabled,
            exclude_patterns,
            last_scan_at: record.last_scan_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryGroupResponse {
    pub organized_library_id: Option<Uuid>,
    pub organized_path: Option<String>,
    pub status: String,
    pub sources: Vec<LibrarySourceResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLibraryRequest {
    pub source_path: String,
    pub organized_path: String,
    #[serde(default)]
    pub watch_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_ingest_enabled: bool,
    #[serde(default = "default_excludes")]
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryRequest {
    pub source_path: Option<String>,
    pub organized_path: Option<String>,
    pub watch_enabled: Option<bool>,
    pub auto_ingest_enabled: Option<bool>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PathPreflightRequest {
    pub path: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PathPreflightResponse {
    pub canonical_path: String,
    pub exists: bool,
    pub directory: bool,
    pub readable: bool,
    pub writable: bool,
    pub device_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityResponse {
    pub extension: &'static str,
    pub metadata_read: bool,
    pub metadata_write: bool,
    pub direct_browser_play: bool,
    pub ffmpeg_transcode: bool,
    pub fingerprint: bool,
}

pub fn capabilities() -> Vec<CapabilityResponse> {
    SUPPORTED_EXTENSIONS
        .iter()
        .map(|extension| CapabilityResponse {
            extension,
            metadata_read: !matches!(*extension, "wma" | "dsf" | "dff"),
            metadata_write: matches!(
                *extension,
                "mp3" | "flac" | "m4a" | "mp4" | "ogg" | "opus" | "wav" | "aiff" | "aif"
            ),
            direct_browser_play: matches!(
                *extension,
                "mp3" | "m4a" | "mp4" | "ogg" | "opus" | "wav"
            ),
            ffmpeg_transcode: true,
            fingerprint: true,
        })
        .collect()
}

pub fn preflight_path(input: &str) -> Result<(PathBuf, PathPreflightResponse), AppError> {
    let requested = Path::new(input);
    if !requested.is_absolute() {
        return Err(AppError::BadRequest("曲库必须使用绝对路径".to_owned()));
    }
    let canonical = requested
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("无法访问曲库：{error}")))?;
    let metadata = canonical
        .metadata()
        .map_err(|error| AppError::BadRequest(format!("无法读取曲库：{error}")))?;
    if !metadata.is_dir() {
        return Err(AppError::BadRequest("曲库路径必须是目录".to_owned()));
    }
    let response = PathPreflightResponse {
        canonical_path: canonical.to_string_lossy().into_owned(),
        exists: true,
        directory: true,
        readable: std::fs::read_dir(&canonical).is_ok(),
        writable: !metadata.permissions().readonly(),
        device_id: device_id(&metadata),
    };
    if !response.readable {
        return Err(AppError::BadRequest("曲库当前不可读".to_owned()));
    }
    Ok((canonical, response))
}

pub fn validate_role(role: &str) -> Result<(), AppError> {
    if matches!(role, "source" | "managed" | "both") {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "role 只能是 source、managed 或 both".to_owned(),
        ))
    }
}

pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            SUPPORTED_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

pub fn path_is_excluded(path: &Path, root: &Path, patterns: &[String]) -> bool {
    path.strip_prefix(root).ok().is_some_and(|relative| {
        relative.components().any(|component| {
            let name = component.as_os_str().to_string_lossy();
            patterns.iter().any(|pattern| name == pattern.as_str())
        })
    })
}

fn default_true() -> bool {
    true
}

fn default_excludes() -> Vec<String> {
    DEFAULT_EXCLUDES.iter().map(ToString::to_string).collect()
}

#[cfg(unix)]
fn device_id(metadata: &Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    metadata.dev().to_string()
}

#[cfg(not(unix))]
fn device_id(_metadata: &Metadata) -> String {
    "unsupported".to_owned()
}
