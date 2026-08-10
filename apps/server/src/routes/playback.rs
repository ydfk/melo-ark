use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::auth::require_user_id;
use crate::{
    auth::{create_media_token, verify_media_token},
    error::AppError,
    playback::{
        self, CreatePlaylistRequest, PlaybackHistory, Playlist, ScrobbleRequest, TranscodeQuery,
        UpdatePlaylistRequest,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/media/{id}/stream", get(stream))
        .route("/api/media/{id}/play-token", get(play_token))
        .route("/api/media/{id}/transcode", get(transcode))
        .route("/api/artwork/{id}", get(artwork))
        .route("/api/playback/scrobble", post(scrobble))
        .route("/api/playback/history", get(history))
        .route("/api/favorites", get(favorites))
        .route("/api/favorites/{id}", put(star).delete(unstar))
        .route("/api/playlists", get(playlists).post(create_playlist))
        .route(
            "/api/playlists/{id}",
            get(playlist).put(update_playlist).delete(delete_playlist),
        )
}

#[derive(Debug, Deserialize)]
struct PlayQuery {
    token: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayTokenResponse {
    pub token: String,
    pub expires_in: i64,
}

#[utoipa::path(get,path="/api/media/{id}/play-token",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=200,body=PlayTokenResponse)))]
async fn play_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PlayTokenResponse>, AppError> {
    require_user_id(&headers, &state)?;
    playback::load_media(&state, id).await?;
    Ok(Json(PlayTokenResponse {
        token: create_media_token(id, &state.jwt)?,
        expires_in: 600,
    }))
}

#[utoipa::path(get,path="/api/artwork/{id}",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=200),(status=404)))]
async fn artwork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<PlayQuery>,
) -> Result<axum::response::Response, AppError> {
    if !headers.contains_key(axum::http::header::AUTHORIZATION)
        && !query
            .token
            .as_deref()
            .is_some_and(|token| verify_media_token(token, id, &state.jwt))
    {
        return Err(AppError::Unauthorized("封面凭据无效".to_owned()));
    }
    if headers.contains_key(axum::http::header::AUTHORIZATION) {
        require_user_id(&headers, &state)?;
    }
    playback::artwork(&state, id).await
}
#[utoipa::path(get,path="/api/media/{id}/stream",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=200),(status=206)))]
async fn stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<PlayQuery>,
) -> Result<axum::response::Response, AppError> {
    if !headers.contains_key(axum::http::header::AUTHORIZATION)
        && !query
            .token
            .as_deref()
            .is_some_and(|token| verify_media_token(token, id, &state.jwt))
    {
        return Err(AppError::Unauthorized("播放凭据无效".to_owned()));
    }
    if headers.contains_key(axum::http::header::AUTHORIZATION) {
        require_user_id(&headers, &state)?;
    }
    playback::stream_media(&state, id, &headers).await
}
#[utoipa::path(get,path="/api/media/{id}/transcode",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=200),(status=206)))]
async fn transcode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<TranscodeQuery>,
) -> Result<axum::response::Response, AppError> {
    if !headers.contains_key(axum::http::header::AUTHORIZATION)
        && !query
            .token
            .as_deref()
            .is_some_and(|token| verify_media_token(token, id, &state.jwt))
    {
        return Err(AppError::Unauthorized("播放凭据无效".to_owned()));
    }
    if headers.contains_key(axum::http::header::AUTHORIZATION) {
        require_user_id(&headers, &state)?;
    }
    playback::transcode_media(
        &state,
        id,
        query.profile.as_deref().unwrap_or("opus-192"),
        &headers,
    )
    .await
}
#[utoipa::path(post,path="/api/playback/scrobble",tag="playback",request_body=ScrobbleRequest,security(("bearerAuth"=[])),responses((status=204)))]
async fn scrobble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScrobbleRequest>,
) -> Result<StatusCode, AppError> {
    let user = require_user_id(&headers, &state)?;
    playback::scrobble(&state, user, request).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/playback/history",tag="playback",security(("bearerAuth"=[])),responses((status=200,body=Vec<PlaybackHistory>)))]
async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PlaybackHistory>>, AppError> {
    let user = require_user_id(&headers, &state)?;
    Ok(Json(playback::history(&state, user).await?))
}
#[utoipa::path(get,path="/api/favorites",tag="playback",security(("bearerAuth"=[])),responses((status=200,body=Vec<Uuid>)))]
async fn favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Uuid>>, AppError> {
    let user = require_user_id(&headers, &state)?;
    Ok(Json(playback::favorite_ids(&state, user).await?))
}
#[utoipa::path(put,path="/api/favorites/{id}",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=204)))]
async fn star(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user = require_user_id(&headers, &state)?;
    playback::set_favorite(&state, user, id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(delete,path="/api/favorites/{id}",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=204)))]
async fn unstar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user = require_user_id(&headers, &state)?;
    playback::set_favorite(&state, user, id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/playlists",tag="playback",security(("bearerAuth"=[])),responses((status=200,body=Vec<Playlist>)))]
async fn playlists(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Playlist>>, AppError> {
    let user = require_user_id(&headers, &state)?;
    Ok(Json(playback::list_playlists(&state, user).await?))
}
#[utoipa::path(post,path="/api/playlists",tag="playback",request_body=CreatePlaylistRequest,security(("bearerAuth"=[])),responses((status=201,body=Playlist)))]
async fn create_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreatePlaylistRequest>,
) -> Result<(StatusCode, Json<Playlist>), AppError> {
    let user = require_user_id(&headers, &state)?;
    Ok((
        StatusCode::CREATED,
        Json(playback::create_playlist(&state, user, request).await?),
    ))
}
#[utoipa::path(get,path="/api/playlists/{id}",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=200,body=Playlist)))]
async fn playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Playlist>, AppError> {
    let user = require_user_id(&headers, &state)?;
    Ok(Json(playback::get_playlist(&state, user, id).await?))
}
#[utoipa::path(put,path="/api/playlists/{id}",tag="playback",params(("id"=Uuid,Path)),request_body=UpdatePlaylistRequest,security(("bearerAuth"=[])),responses((status=200,body=Playlist)))]
async fn update_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdatePlaylistRequest>,
) -> Result<Json<Playlist>, AppError> {
    let user = require_user_id(&headers, &state)?;
    Ok(Json(
        playback::update_playlist(&state, user, id, request).await?,
    ))
}
#[utoipa::path(delete,path="/api/playlists/{id}",tag="playback",params(("id"=Uuid,Path)),security(("bearerAuth"=[])),responses((status=204)))]
async fn delete_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user = require_user_id(&headers, &state)?;
    playback::delete_playlist(&state, user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
