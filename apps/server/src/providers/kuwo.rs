use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::{
    MetadataProvider, ProviderCapabilities, ProviderError, ProviderTrack, TrackQuery, as_i64,
    request_error, search_text,
};
use crate::state::AppState;

pub struct KuwoProvider;

#[async_trait]
impl MetadataProvider for KuwoProvider {
    fn id(&self) -> &'static str {
        "kuwo"
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
                "{}/api/www/search/searchMusicBykeyWord",
                base_url.trim_end_matches('/')
            ))
            .query(&[
                ("key", search_text(query)),
                ("pn", "1".to_owned()),
                ("rn", "10".to_owned()),
                ("httpsStatus", "1".to_owned()),
            ])
            .header("Referer", "https://www.kuwo.cn/")
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
        .pointer("/data/list")
        .or_else(|| value.pointer("/data/musicList"))
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::InvalidResponse("缺少 data.list".to_owned()))?;
    Ok(songs
        .iter()
        .filter_map(|song| {
            let id = song["rid"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| as_i64(&song["rid"]).map(|value| value.to_string()))?;
            let duration = as_i64(&song["duration"]);
            Some(ProviderTrack {
                id,
                title: song["name"].as_str()?.to_owned(),
                artists: split_artists(song["artist"].as_str().unwrap_or("未知艺术家")),
                album: optional_text(&song["album"]),
                duration_ms: duration
                    .map(|value| if value < 10_000 { value * 1_000 } else { value }),
                year: song["releaseDate"]
                    .as_str()
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse().ok()),
                track_no: as_i64(&song["track"]),
                version_label: optional_text(&song["songTimeMinutes"]),
                artwork_url: optional_text(&song["pic"]),
            })
        })
        .collect())
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

#[cfg(test)]
mod tests {
    #[test]
    fn parses_fixture() {
        let value = serde_json::json!({"data":{"list":[{"rid":123,"name":"晴天","artist":"周杰伦","album":"叶惠美","duration":269,"releaseDate":"2003-07-31","pic":"https://img"}]}});
        let tracks = super::parse_search(&value).expect("解析 Kuwo fixture");
        assert_eq!(tracks[0].id, "123");
        assert_eq!(tracks[0].duration_ms, Some(269_000));
    }
}
