use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

use super::{
    MetadataProvider, ProviderCapabilities, ProviderError, ProviderTrack, TrackQuery, as_i64,
    request_error, search_text,
};
use crate::state::AppState;

pub struct KugouProvider;

#[async_trait]
impl MetadataProvider for KugouProvider {
    fn id(&self) -> &'static str {
        "kugou"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            metadata: true,
            artwork: true,
            lyrics: true,
        }
    }
    async fn search_track(
        &self,
        state: &AppState,
        base_url: &str,
        query: &TrackQuery,
        timeout: Duration,
    ) -> Result<Vec<ProviderTrack>, ProviderError> {
        let response = state
            .http
            .get(format!("{base_url}/song_search_v2"))
            .query(&[
                ("keyword", search_text(query)),
                ("page", "1".to_owned()),
                ("pagesize", "10".to_owned()),
                ("platform", "WebFilter".to_owned()),
            ])
            .timeout(timeout)
            .send()
            .await
            .map_err(request_error)?
            .error_for_status()
            .map_err(request_error)?
            .json::<Value>()
            .await
            .map_err(request_error)?;
        parse_search(&response)
    }
}

pub(crate) fn parse_search(value: &Value) -> Result<Vec<ProviderTrack>, ProviderError> {
    let songs = value
        .pointer("/data/lists")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::InvalidResponse("缺少 data.lists".to_owned()))?;
    Ok(songs
        .iter()
        .filter_map(|song| {
            let title = song["SongName"]
                .as_str()
                .or_else(|| song["songname"].as_str())?;
            let singer = song["SingerName"]
                .as_str()
                .or_else(|| song["singername"].as_str())
                .unwrap_or("未知艺术家");
            Some(ProviderTrack {
                id: song["FileHash"]
                    .as_str()
                    .or_else(|| song["EMixSongID"].as_str())?
                    .to_owned(),
                title: title.to_owned(),
                artists: singer
                    .split(['、', '&'])
                    .map(str::trim)
                    .map(str::to_owned)
                    .collect(),
                album: song["AlbumName"]
                    .as_str()
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned),
                duration_ms: as_i64(&song["Duration"]).map(|seconds| seconds * 1000),
                year: None,
                track_no: None,
                version_label: None,
                artwork_url: song["Image"]
                    .as_str()
                    .map(|url| url.replace("{size}", "400")),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_fixture() {
        let value = serde_json::json!({"data":{"lists":[{"FileHash":"hash","SongName":"晴天","SingerName":"周杰伦","AlbumName":"叶惠美","Duration":269}]}});
        let tracks = super::parse_search(&value).expect("解析酷狗 fixture");
        assert_eq!(tracks[0].duration_ms, Some(269000));
    }
}
