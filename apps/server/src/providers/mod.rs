mod kugou;
mod kuwo;
mod migu;
mod musicbrainz;
mod netease;
mod qq;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub metadata: bool,
    pub artwork: bool,
    pub lyrics: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackQuery {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub track_no: Option<i64>,
    pub year: Option<i64>,
    pub version_label: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTrack {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub version_label: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCandidate {
    pub url: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("在线数据源尚未配置")]
    NotConfigured,
    #[error("在线数据源暂不支持此能力")]
    Unsupported,
    #[error("在线数据源请求超时")]
    Timeout,
    #[error("在线数据源请求错误：{0}")]
    Http(String),
    #[error("在线数据源响应格式已变化：{0}")]
    InvalidResponse(String),
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Unsupported => "unsupported",
            Self::Timeout => "timeout",
            Self::Http(_) => "http_error",
            Self::InvalidResponse(_) => "invalid_response",
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::Http(_))
    }
}

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn search_track(
        &self,
        state: &AppState,
        base_url: &str,
        query: &TrackQuery,
        timeout: Duration,
    ) -> Result<Vec<ProviderTrack>, ProviderError>;

    async fn get_track(
        &self,
        _state: &AppState,
        _base_url: &str,
        _id: &str,
    ) -> Result<ProviderTrack, ProviderError> {
        Err(ProviderError::Unsupported)
    }

    async fn get_cover(
        &self,
        _state: &AppState,
        _base_url: &str,
        _id: &str,
    ) -> Result<Vec<ArtworkCandidate>, ProviderError> {
        Err(ProviderError::Unsupported)
    }
}

pub fn metadata_registry() -> Vec<Arc<dyn MetadataProvider>> {
    vec![
        Arc::new(qq::QqProvider),
        Arc::new(netease::NeteaseProvider),
        Arc::new(kugou::KugouProvider),
        Arc::new(kuwo::KuwoProvider),
        Arc::new(migu::MiguProvider),
        Arc::new(musicbrainz::MusicBrainzProvider),
    ]
}

pub fn metadata_provider(id: &str) -> Option<Arc<dyn MetadataProvider>> {
    metadata_registry()
        .into_iter()
        .find(|provider| provider.id() == id)
}

pub(crate) fn search_text(query: &TrackQuery) -> String {
    std::iter::once(query.title.as_str())
        .chain(query.artists.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn request_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Http(error.to_string())
    }
}

pub(crate) fn as_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|item| item.parse().ok()))
}
