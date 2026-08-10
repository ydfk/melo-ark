use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

use super::{
    MetadataProvider, ProviderCapabilities, ProviderError, ProviderTrack, TrackQuery, as_i64,
    request_error, search_text,
};
use crate::state::AppState;

pub struct NeteaseProvider;

#[async_trait]
impl MetadataProvider for NeteaseProvider {
    fn id(&self) -> &'static str {
        "netease"
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
            .post(format!("{base_url}/api/search/get/web"))
            .form(&[
                ("s", search_text(query)),
                ("type", "1".to_owned()),
                ("limit", "10".to_owned()),
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
        .pointer("/result/songs")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::InvalidResponse("缺少 result.songs".to_owned()))?;
    Ok(songs
        .iter()
        .filter_map(|song| {
            Some(ProviderTrack {
                id: as_i64(&song["id"])?.to_string(),
                title: song["name"].as_str()?.to_owned(),
                artists: song["artists"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|artist| artist["name"].as_str().map(str::to_owned))
                    .collect(),
                album: song
                    .pointer("/album/name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms: as_i64(&song["duration"]),
                year: None,
                track_no: None,
                version_label: None,
                artwork_url: song
                    .pointer("/album/picUrl")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_fixture() {
        let value = serde_json::json!({"result":{"songs":[{"id":1,"name":"晴天","duration":269000,"artists":[{"name":"周杰伦"}],"album":{"name":"叶惠美","picUrl":"https://img"}}]}});
        let tracks = super::parse_search(&value).expect("解析网易 fixture");
        assert_eq!(tracks[0].artists, ["周杰伦"]);
    }
}
