use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;

use super::{
    MetadataProvider, ProviderCapabilities, ProviderError, ProviderTrack, TrackQuery, as_i64,
    request_error, search_text,
};
use crate::state::AppState;

pub struct QqProvider;

#[async_trait]
impl MetadataProvider for QqProvider {
    fn id(&self) -> &'static str {
        "qq"
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
        let body = json!({
            "comm": {"ct": 24, "cv": 0},
            "req": {"method": "DoSearchForQQMusicDesktop", "module": "music.search.SearchCgiService", "param": {"query": search_text(query), "page_num": 1, "num_per_page": 10, "search_type": 0}}
        });
        let response = state
            .http
            .post(format!("{base_url}/cgi-bin/musicu.fcg"))
            .json(&body)
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
        .pointer("/req/data/body/song/list")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::InvalidResponse("缺少 req.data.body.song.list".to_owned()))?;
    Ok(songs
        .iter()
        .filter_map(|song| {
            let album_mid = song.pointer("/album/mid").and_then(Value::as_str);
            Some(ProviderTrack {
                id: song["mid"].as_str()?.to_owned(),
                title: song["title"].as_str()?.to_owned(),
                artists: song["singer"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|artist| artist["name"].as_str().map(str::to_owned))
                    .collect(),
                album: song
                    .pointer("/album/name")
                    .and_then(Value::as_str)
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned),
                duration_ms: as_i64(&song["interval"]).map(|seconds| seconds * 1000),
                year: song["time_public"]
                    .as_str()
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse().ok()),
                track_no: as_i64(&song["index_album"]),
                version_label: song["subtitle"]
                    .as_str()
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned),
                artwork_url: album_mid.map(|mid| {
                    format!("https://y.qq.com/music/photo_new/T002R500x500M000{mid}.jpg")
                }),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_fixture() {
        let value = serde_json::json!({"req":{"data":{"body":{"song":{"list":[{"mid":"qq1","title":"晴天","interval":269,"index_album":3,"time_public":"2003-07-31","singer":[{"name":"周杰伦"}],"album":{"name":"叶惠美","mid":"album1"}}]}}}}});
        let tracks = super::parse_search(&value).expect("解析 QQ fixture");
        assert_eq!(tracks[0].track_no, Some(3));
    }
}
