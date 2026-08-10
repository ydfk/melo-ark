mod auth;
mod catalog;
mod response;

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Path, State},
    http::{Request, Response},
    routing::any,
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use self::auth::{Params, first};
use crate::{error::AppError, lyrics, playback, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/rest/{method}", any(handle))
}

async fn handle(
    State(state): State<AppState>,
    Path(method): Path<String>,
    request: Request<Body>,
) -> Result<Response<Body>, AppError> {
    let method = method.strip_suffix(".view").unwrap_or(&method).to_owned();
    let query = request.uri().query().map(str::to_owned);
    let headers = request.headers().clone();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(AppError::internal)?;
    let params = auth::parse_params(query.as_deref(), &body);
    let user = match auth::authenticate(&state, &params).await {
        Ok(user) => user,
        Err(error) => return response::failed(&params, 40, error.to_string()),
    };
    let result = dispatch(&state, user, &method, &params, &headers).await;
    match result {
        Ok(response) => Ok(response),
        Err(error) => response::failed(&params, 10, error.to_string()),
    }
}

async fn dispatch(
    state: &AppState,
    user: Uuid,
    method: &str,
    params: &Params,
    headers: &axum::http::HeaderMap,
) -> Result<Response<Body>, AppError> {
    match method {
        "ping" => response::empty(params),
        "getLicense" => response::ok(
            params,
            single(
                "license",
                json!({"valid":true,"email":"MeloArk local admin","licenseExpires":"2099-12-31T00:00:00Z"}),
            ),
        ),
        "getOpenSubsonicExtensions" => response::ok(
            params,
            single(
                "openSubsonicExtensions",
                json!({"openSubsonicExtension":[{"name":"songLyrics","versions":[1]},{"name":"formPost","versions":[1]}]}),
            ),
        ),
        "getMusicFolders" => response::ok(params, catalog::music_folders(state).await?),
        "getIndexes" => response::ok(params, catalog::indexes(state, user).await?),
        "getArtists" => response::ok(params, catalog::artists(state, user).await?),
        "getArtist" => response::ok(
            params,
            catalog::artist(state, required_uuid(params, "id", "ar:")?, user).await?,
        ),
        "getAlbum" => response::ok(
            params,
            catalog::album(state, required_uuid(params, "id", "al:")?, user).await?,
        ),
        "getSong" => response::ok(
            params,
            catalog::song(state, required_track(params, "id")?, user).await?,
        ),
        "getMusicDirectory" => response::ok(
            params,
            catalog::music_directory(state, user, required(params, "id")?).await?,
        ),
        "getAlbumList2" => response::ok(
            params,
            catalog::album_list(
                state,
                user,
                first(params, "type").unwrap_or("newest"),
                number(params, "size", 10),
                number(params, "offset", 0),
            )
            .await?,
        ),
        "getRandomSongs" => response::ok(
            params,
            catalog::random_songs(state, user, number(params, "size", 10)).await?,
        ),
        "getStarred2" => response::ok(params, catalog::starred(state, user).await?),
        "search3" => response::ok(
            params,
            catalog::search(
                state,
                user,
                first(params, "query").unwrap_or(""),
                number(params, "artistCount", 20),
                number(params, "albumCount", 20),
                number(params, "songCount", 50),
            )
            .await?,
        ),
        "stream" | "download" => stream(state, params, headers, method == "stream").await,
        "getCoverArt" => cover_art(state, params).await,
        "star" => {
            for id in values(params, "id") {
                if let Some(id) = catalog::parse_track_id(id) {
                    playback::set_favorite(state, user, id, true).await?;
                }
            }
            response::empty(params)
        }
        "unstar" => {
            for id in values(params, "id") {
                if let Some(id) = catalog::parse_track_id(id) {
                    playback::set_favorite(state, user, id, false).await?;
                }
            }
            response::empty(params)
        }
        "scrobble" => {
            let track_id = required_track(params, "id")?;
            let submission = first(params, "submission").is_some_and(|value| value == "true");
            playback::scrobble(
                state,
                user,
                playback::ScrobbleRequest {
                    track_id,
                    media_file_id: None,
                    completed: submission,
                    position_sec: None,
                    client: first(params, "c").map(str::to_owned),
                },
            )
            .await?;
            response::empty(params)
        }
        "getNowPlaying" => now_playing(state, user, params).await,
        "getPlaylists" => playlists(state, user, params).await,
        "getPlaylist" => playlist(state, user, required_uuid(params, "id", "")?, params).await,
        "createPlaylist" => create_playlist(state, user, params).await,
        "updatePlaylist" => update_playlist(state, user, params).await,
        "deletePlaylist" => {
            playback::delete_playlist(state, user, required_uuid(params, "id", "")?).await?;
            response::empty(params)
        }
        "getLyricsBySongId" => lyrics_by_song(state, required_track(params, "id")?, params).await,
        _ => response::failed(params, 0, format!("MeloArk 尚未实现 {method}")),
    }
}

async fn stream(
    state: &AppState,
    params: &Params,
    headers: &axum::http::HeaderMap,
    allow_transcode: bool,
) -> Result<Response<Body>, AppError> {
    let id = required(params, "id")?;
    let media_id = if let Some(id) = catalog::parse_media_id(id.strip_prefix("mf:").unwrap_or("")) {
        id
    } else {
        playback::best_media_id(
            state,
            catalog::parse_track_id(id)
                .ok_or_else(|| AppError::BadRequest("歌曲 ID 无效".to_owned()))?,
        )
        .await?
    };
    let max_bitrate = number(params, "maxBitRate", 0);
    if allow_transcode && max_bitrate > 0 {
        let profile = if max_bitrate <= 192 {
            "opus-192"
        } else if max_bitrate <= 256 {
            "aac-256"
        } else {
            "mp3-320"
        };
        playback::transcode_media(state, media_id, profile, headers).await
    } else {
        playback::stream_media(state, media_id, headers).await
    }
}

async fn cover_art(state: &AppState, params: &Params) -> Result<Response<Body>, AppError> {
    let id = required(params, "id")?;
    let media_id = if let Some(id) = id.strip_prefix("mf:").and_then(|value| value.parse().ok()) {
        id
    } else if let Some(album_id) = id
        .strip_prefix("al:")
        .and_then(|value| value.parse::<Uuid>().ok())
    {
        sqlx::query_scalar::<_,Uuid>("SELECT mf.id FROM media_files mf JOIN tracks t ON t.id=mf.track_id WHERE t.album_id=? ORDER BY mf.file_size DESC LIMIT 1").bind(album_id).fetch_optional(&state.pool).await.map_err(AppError::internal)?.ok_or_else(||AppError::NotFound("封面不存在".to_owned()))?
    } else {
        return Err(AppError::BadRequest("CoverArt ID 无效".to_owned()));
    };
    playback::artwork(state, media_id).await
}

async fn playlists(
    state: &AppState,
    user: Uuid,
    params: &Params,
) -> Result<Response<Body>, AppError> {
    let items=playback::list_playlists(state,user).await?.into_iter().map(|item|json!({"id":item.id,"name":item.name,"comment":item.comment,"songCount":item.song_count,"duration":item.duration_sec,"created":item.created_at,"changed":item.updated_at,"owner":"admin","public":false})).collect::<Vec<_>>();
    response::ok(params, single("playlists", json!({"playlist":items})))
}
async fn playlist(
    state: &AppState,
    user: Uuid,
    id: Uuid,
    params: &Params,
) -> Result<Response<Body>, AppError> {
    let item = playback::get_playlist(state, user, id).await?;
    let tracks = playback::playlist_tracks(state, user, id).await?;
    let mut entries = Vec::new();
    for id in tracks {
        entries.push(catalog::song(state, id, user).await?["song"].clone());
    }
    response::ok(
        params,
        single(
            "playlist",
            json!({"id":item.id,"name":item.name,"comment":item.comment,"songCount":item.song_count,"duration":item.duration_sec,"created":item.created_at,"changed":item.updated_at,"owner":"admin","public":false,"entry":entries}),
        ),
    )
}
async fn create_playlist(
    state: &AppState,
    user: Uuid,
    params: &Params,
) -> Result<Response<Body>, AppError> {
    let tracks = values(params, "songId")
        .iter()
        .filter_map(|id| catalog::parse_track_id(id))
        .collect();
    let item = playback::create_playlist(
        state,
        user,
        playback::CreatePlaylistRequest {
            name: first(params, "name").unwrap_or("新播放列表").to_owned(),
            comment: None,
            track_ids: tracks,
        },
    )
    .await?;
    playlist(state, user, item.id, params).await
}
async fn update_playlist(
    state: &AppState,
    user: Uuid,
    params: &Params,
) -> Result<Response<Body>, AppError> {
    let id = required_uuid(params, "playlistId", "")?;
    let mut tracks = playback::playlist_tracks(state, user, id).await?;
    let mut indexes: Vec<usize> = values(params, "songIndexToRemove")
        .iter()
        .filter_map(|item| item.parse().ok())
        .collect();
    indexes.sort_unstable_by(|a, b| b.cmp(a));
    for index in indexes {
        if index < tracks.len() {
            tracks.remove(index);
        }
    }
    tracks.extend(
        values(params, "songIdToAdd")
            .iter()
            .filter_map(|id| catalog::parse_track_id(id)),
    );
    let item = playback::update_playlist(
        state,
        user,
        id,
        playback::UpdatePlaylistRequest {
            name: first(params, "name").map(str::to_owned),
            comment: first(params, "comment").map(str::to_owned),
            track_ids: Some(tracks),
        },
    )
    .await?;
    playlist(state, user, item.id, params).await
}

async fn lyrics_by_song(
    state: &AppState,
    track_id: Uuid,
    params: &Params,
) -> Result<Response<Body>, AppError> {
    let records = lyrics::list(state, track_id).await?;
    let structured=records.into_iter().filter(|item|item.active||item.storage=="candidate").map(|item|{let lines=lyrics::parse_lrc(&item.content).unwrap_or_default();json!({"displayArtist":"","displayTitle":"","lang":item.language.unwrap_or_else(||"und".to_owned()),"synced":item.synced,"line":lines.into_iter().map(|line|json!({"start":line.timestamp_ms,"value":[line.text]})).collect::<Vec<_>>()})}).collect::<Vec<_>>();
    response::ok(
        params,
        single("lyricsList", json!({"structuredLyrics":structured})),
    )
}
async fn now_playing(
    state: &AppState,
    user: Uuid,
    params: &Params,
) -> Result<Response<Body>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, i64, String)>(
        "SELECT track_id,position_sec,client FROM now_playing WHERE user_id=? AND updated_at>? ",
    )
    .bind(user)
    .bind(chrono::Utc::now() - chrono::Duration::minutes(30))
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut entries = Vec::new();
    for (track_id, position, client) in rows {
        let mut song = catalog::song(state, track_id, user).await?["song"].clone();
        if let Some(object) = song.as_object_mut() {
            object.insert("username".to_owned(), json!("admin"));
            object.insert("minutesAgo".to_owned(), json!(0));
            object.insert("playerName".to_owned(), json!(client));
            object.insert("playerId".to_owned(), json!("meloark"));
            object.insert("position".to_owned(), json!(position * 1000));
        }
        entries.push(song);
    }
    response::ok(params, single("nowPlaying", json!({"entry":entries})))
}

fn required<'a>(params: &'a Params, key: &str) -> Result<&'a str, AppError> {
    first(params, key).ok_or_else(|| AppError::BadRequest(format!("缺少参数 {key}")))
}
fn required_track(params: &Params, key: &str) -> Result<Uuid, AppError> {
    catalog::parse_track_id(required(params, key)?)
        .ok_or_else(|| AppError::BadRequest("歌曲 ID 无效".to_owned()))
}
fn required_uuid(params: &Params, key: &str, prefix: &str) -> Result<Uuid, AppError> {
    let value = required(params, key)?;
    value
        .strip_prefix(prefix)
        .unwrap_or(value)
        .parse()
        .map_err(|_| AppError::BadRequest(format!("{key} 无效")))
}
fn values<'a>(params: &'a Params, key: &str) -> Vec<&'a str> {
    params
        .get(key)
        .map(|items| items.iter().map(String::as_str).collect())
        .unwrap_or_default()
}
fn number(params: &Params, key: &str, default: i64) -> i64 {
    first(params, key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
fn single(key: &str, value: Value) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert(key.to_owned(), value);
    map
}
