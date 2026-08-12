mod ai;
mod auth;
mod duplicates;
mod filesystem;
mod jobs;
mod libraries;
mod lyrics;
mod organizer;
mod playback;
mod scraper;
mod settings;
mod system;
mod tags;
mod tracks;
mod trash;

use axum::{Router, routing::get};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    ai::{AiDuplicateRequest, AiRecommendation, AiRerankRequest, AiStatus},
    duplicates::{AnalyzeRequest, DuplicateGroup, DuplicateMember},
    error::Problem,
    jobs::{JobEvent, JobLogPage, JobLogResponse, JobResponse},
    library::{
        CapabilityResponse, CreateLibraryRequest, LibraryResponse, PathPreflightRequest,
        PathPreflightResponse, UpdateLibraryRequest,
    },
    lyrics::{
        ApplyLyricsRequest, LyricsFailure, LyricsRecord, LyricsSearchRequest, LyricsSearchResponse,
        LyricsWriteMode,
    },
    model::UserResponse,
    organizer::{OrganizerApplyRequest, OrganizerPreviewRequest, OrganizerUndoRequest},
    playback::{
        CreatePlaylistRequest, PlaybackHistory, Playlist, ScrobbleRequest, TranscodeQuery,
        UpdatePlaylistRequest,
    },
    runtime_settings::{
        EditableSettings, InfrastructureSettings, SettingsResponse, UpdateSettingsRequest,
    },
    scraper::{
        BatchScrapeRequest, ProviderFailure, ProviderSetting, ScrapeApplyRequest, ScrapeCandidate,
        ScrapeSearchRequest, ScrapeSearchResponse, UpdateProviderRequest,
    },
    state::AppState,
    tag_operations::{
        ApplyOperationRequest, CoverData, OperationItemResponse, OperationResponse, TagDiff,
        TagField, TagPreviewRequest, TagSet, TagTransform, TagValues, UndoOperationRequest,
    },
    trash::{
        TrashApplyRequest, TrashEntryResponse, TrashPreviewRequest, TrashPurgeApplyRequest,
        TrashPurgeItemResponse, TrashPurgePreviewRequest, TrashPurgeResponse,
    },
};

use self::{
    auth::{
        Credentials, SetupStatusResponse, TokenResponse, UpdateProfileRequest,
        UpdateProfileResponse,
    },
    filesystem::{DirectoryEntry, DirectoryListing},
    system::{
        DashboardRecentPlay, DashboardRecentTrack, DashboardStatsResponse, FormatDistribution,
        HealthResponse,
    },
    tracks::{
        MediaFileResponse, TrackDetailResponse, TrackListResponse, TrackOperationHistoryResponse,
        TrackResponse,
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        system::health,
        system::dashboard_stats,
        auth::setup_status,
        auth::setup,
        auth::login,
        auth::profile,
        auth::update_profile,
        filesystem::list_directories,
        settings::get_settings,
        settings::update_settings,
        libraries::list,
        libraries::create,
        libraries::update,
        libraries::delete,
        libraries::preflight,
        libraries::capabilities,
        libraries::scan,
        jobs::list,
        jobs::get_one,
        jobs::logs,
        jobs::pause,
        jobs::resume,
        jobs::cancel,
        jobs::retry_failed,
        tracks::list,
        tracks::get_one,
        tracks::files,
        tracks::operations,
        tags::preview,
        tags::apply,
        tags::retry_failed,
        tags::undo,
        tags::get_operation,
        organizer::preview,
        organizer::apply,
        organizer::retry_failed,
        organizer::undo,
        trash::preview,
        trash::apply,
        trash::restore,
        trash::list,
        trash::preview_purge,
        trash::apply_purge,
        scraper::list_providers,
        scraper::update_provider,
        scraper::search,
        scraper::create_job,
        scraper::candidates,
        scraper::apply,
        lyrics::list,
        lyrics::search,
        lyrics::apply,
        duplicates::analyze,
        duplicates::groups,
        duplicates::group,
        duplicates::rebuild,
        ai::status,
        ai::explain,
        ai::rerank,
        playback::stream,
        playback::transcode,
        playback::scrobble,
        playback::history,
        playback::play_token,
        playback::artwork,
        playback::favorites,
        playback::star,
        playback::unstar,
        playback::playlists,
        playback::create_playlist,
        playback::playlist,
        playback::update_playlist,
        playback::delete_playlist
    ),
    components(schemas(
        Credentials,
        TokenResponse,
        UpdateProfileRequest,
        UpdateProfileResponse,
        SetupStatusResponse,
        HealthResponse,
        DashboardStatsResponse,
        FormatDistribution,
        DashboardRecentTrack,
        DashboardRecentPlay,
        UserResponse,
        DirectoryEntry,
        DirectoryListing,
        EditableSettings,
        InfrastructureSettings,
        SettingsResponse,
        UpdateSettingsRequest,
        Problem,
        LibraryResponse,
        CreateLibraryRequest,
        UpdateLibraryRequest,
        PathPreflightRequest,
        PathPreflightResponse,
        CapabilityResponse,
        JobResponse,
        JobLogResponse,
        JobLogPage,
        JobEvent,
        TrackResponse,
        TrackListResponse,
        TrackDetailResponse,
        MediaFileResponse,
        TrackOperationHistoryResponse,
        TagValues,
        CoverData,
        TagSet,
        TagField,
        TagTransform,
        TagPreviewRequest,
        ApplyOperationRequest,
        UndoOperationRequest,
        TagDiff,
        OperationItemResponse,
        OperationResponse,
        OrganizerPreviewRequest,
        OrganizerApplyRequest,
        OrganizerUndoRequest,
        TrashPreviewRequest,
        TrashApplyRequest,
        TrashEntryResponse,
        TrashPurgePreviewRequest,
        TrashPurgeApplyRequest,
        TrashPurgeItemResponse,
        TrashPurgeResponse,
        ProviderSetting,
        ProviderFailure,
        UpdateProviderRequest,
        ScrapeSearchRequest,
        ScrapeSearchResponse,
        ScrapeCandidate,
        ScrapeApplyRequest,
        BatchScrapeRequest,
        LyricsRecord,
        LyricsFailure,
        LyricsSearchRequest,
        LyricsSearchResponse,
        ApplyLyricsRequest,
        LyricsWriteMode,
        AnalyzeRequest,
        DuplicateGroup,
        DuplicateMember,
        AiStatus,
        AiDuplicateRequest,
        AiRerankRequest,
        AiRecommendation,
        TranscodeQuery,
        ScrobbleRequest,
        PlaybackHistory,
        CreatePlaylistRequest,
        UpdatePlaylistRequest,
        Playlist,
        playback::PlayTokenResponse
    )),
    tags(
        (name = "system", description = "服务与曲库健康状态"),
        (name = "auth", description = "单管理员认证"),
        (name = "settings", description = "运行设置与部署信息"),
        (name = "libraries", description = "曲库根目录"),
        (name = "jobs", description = "持久化任务"),
        (name = "tracks", description = "逻辑歌曲与物理文件索引"),
        (name = "tags", description = "Tag 预览、写入与撤销"),
        (name = "organizer", description = "Hardlink Dry Run 与执行"),
        (name = "trash", description = "曲库回收站"),
        (name = "operations", description = "持久化操作日志"),
        (name = "providers", description = "在线数据源健康状态、优先级与启停"),
        (name = "scraper", description = "多源候选、置信度评分与应用预览"),
        (name = "lyrics", description = "歌词候选、质量评分与显式写入"),
        (name = "duplicates", description = "分层重复分析与 Quality Score"),
        (name = "ai", description = "可选 AI 元数据建议；不上传原始音频"),
        (name = "playback", description = "HTTP Range、转码、播放历史、收藏与播放列表")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearerAuth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(system::router())
        .merge(ai::router())
        .merge(duplicates::router())
        .merge(crate::opensubsonic::router())
        .merge(playback::router())
        .merge(auth::router())
        .merge(filesystem::router())
        .merge(settings::router())
        .merge(libraries::router())
        .merge(organizer::router())
        .merge(scraper::router())
        .merge(jobs::router())
        .merge(tags::router())
        .merge(tracks::router())
        .merge(trash::router())
        .merge(lyrics::router())
        .route("/api/events", get(jobs::events))
        .route("/openapi.yaml", get(openapi_yaml))
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}

async fn openapi_yaml() -> Result<String, crate::error::AppError> {
    yaml_serde::to_string(&ApiDoc::openapi()).map_err(crate::error::AppError::internal)
}
