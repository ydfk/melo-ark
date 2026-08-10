use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::{
    MetadataProvider, ProviderCapabilities, ProviderError, ProviderTrack, TrackQuery, as_i64,
    request_error, search_text,
};
use crate::state::AppState;

pub struct MiguProvider;

#[async_trait]
impl MetadataProvider for MiguProvider {
    fn id(&self) -> &'static str {
        "migu"
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
            .get(format!(
                "{}/v3/api/search/audio",
                base_url.trim_end_matches('/')
            ))
            .query(&[
                ("q", search_text(query)),
                ("pageNo", "1".to_owned()),
                ("pageSize", "10".to_owned()),
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
        .pointer("/data/songs")
        .or_else(|| value.pointer("/data/items"))
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::InvalidResponse("缺少 data.songs".to_owned()))?;
    Ok(songs
        .iter()
        .filter_map(|song| {
            let id = ["copyrightId", "contentId", "id"]
                .iter()
                .find_map(|key| value_as_id(&song[*key]))?;
            let title = song["songName"]
                .as_str()
                .or_else(|| song["name"].as_str())?
                .to_owned();
            let singer = song["singerName"]
                .as_str()
                .or_else(|| song["singer"].as_str())
                .unwrap_or("未知艺术家");
            let duration = as_i64(&song["duration"]).or_else(|| as_i64(&song["durationMs"]));
            Some(ProviderTrack {
                id,
                title,
                artists: split_artists(singer),
                album: optional_text(&song["albumName"]).or_else(|| optional_text(&song["album"])),
                duration_ms: duration
                    .map(|value| if value < 10_000 { value * 1_000 } else { value }),
                year: song["publishDate"]
                    .as_str()
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse().ok()),
                track_no: as_i64(&song["trackNo"]),
                version_label: optional_text(&song["version"]),
                artwork_url: artwork_url(song),
            })
        })
        .collect())
}

fn value_as_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| as_i64(value).map(|value| value.to_string()))
}

fn optional_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn split_artists(value: &str) -> Vec<String> {
    value
        .split(['&', '、', '/'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn artwork_url(song: &Value) -> Option<String> {
    optional_text(&song["largePic"])
        .or_else(|| optional_text(&song["albumPic"]))
        .or_else(|| {
            song["albumImgs"]
                .as_array()
                .and_then(|images| images.first())
                .and_then(|image| optional_text(&image["img"]))
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_fixture() {
        let value = serde_json::json!({"data":{"songs":[{"copyrightId":"migu-1","songName":"晴天","singerName":"周杰伦","albumName":"叶惠美","durationMs":269000,"publishDate":"2003-07-31","largePic":"https://img"}]}});
        let tracks = super::parse_search(&value).expect("解析 Migu fixture");
        assert_eq!(tracks[0].album.as_deref(), Some("叶惠美"));
        assert_eq!(tracks[0].year, Some(2003));
    }
}
