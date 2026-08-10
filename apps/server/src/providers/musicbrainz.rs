use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

use super::{
    MetadataProvider, ProviderCapabilities, ProviderError, ProviderTrack, TrackQuery, request_error,
};
use crate::state::AppState;

pub struct MusicBrainzProvider;

#[async_trait]
impl MetadataProvider for MusicBrainzProvider {
    fn id(&self) -> &'static str {
        "musicbrainz"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            metadata: true,
            artwork: true,
            lyrics: false,
        }
    }

    async fn search_track(
        &self,
        state: &AppState,
        base_url: &str,
        query: &TrackQuery,
        timeout: Duration,
    ) -> Result<Vec<ProviderTrack>, ProviderError> {
        let search = format!(
            "recording:{} AND artist:{}",
            query.title,
            query.artists.join(" ")
        );
        let response = state
            .http
            .get(format!("{base_url}/ws/2/recording"))
            .query(&[
                ("query", search),
                ("fmt", "json".to_owned()),
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
    let recordings = value["recordings"]
        .as_array()
        .ok_or_else(|| ProviderError::InvalidResponse("缺少 recordings".to_owned()))?;
    Ok(recordings
        .iter()
        .filter_map(|recording| {
            let id = recording["id"].as_str()?.to_owned();
            let title = recording["title"].as_str()?.to_owned();
            let artists = recording["artist-credit"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|credit| credit["name"].as_str().map(str::to_owned))
                .collect();
            let release = recording["releases"]
                .as_array()
                .and_then(|items| items.first());
            Some(ProviderTrack {
                id,
                title,
                artists,
                album: release
                    .and_then(|item| item["title"].as_str())
                    .map(str::to_owned),
                duration_ms: recording["length"].as_i64(),
                year: release
                    .and_then(|item| item["date"].as_str())
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse().ok()),
                track_no: None,
                version_label: None,
                artwork_url: release
                    .and_then(|item| item["id"].as_str())
                    .map(|release_id| {
                        format!("https://coverartarchive.org/release/{release_id}/front-500")
                    }),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_fixture() {
        let value = serde_json::json!({"recordings":[{"id":"mb-1","title":"晴天","length":269000,"artist-credit":[{"name":"周杰伦"}],"releases":[{"id":"release-1","title":"叶惠美","date":"2003-07-31"}]}]});
        let tracks = super::parse_search(&value).expect("解析 MusicBrainz fixture");
        assert_eq!(tracks[0].album.as_deref(), Some("叶惠美"));
        assert_eq!(tracks[0].year, Some(2003));
    }
}
