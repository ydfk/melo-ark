import { alovaInstance } from "@/lib/api";
import type {
  CreateLibraryRequest,
  UpdateLibraryRequest,
  DashboardStats,
  Job,
  LibraryRoot,
  PathPreflight,
  TrackList,
  TrackFilter,
  TrackDetail,
  MediaFile,
  Operation,
  TrashEntry,
  TrashPurge,
  TagField,
  TagTransform,
  ScrapeSearchResponse,
  ScrapeCandidate,
  LyricsRecord,
  LyricsSearchResponse,
  ProviderSetting,
  DuplicateGroup,
  AiStatus,
  AiRecommendation,
  PlayTokenResponse,
  PlaybackHistory,
  TrackOperationHistory,
  Playlist,
  DirectoryListing,
  RuntimeSettings,
  EditableSettings,
} from "@/lib/api/types";

export const getDashboardStats = () => alovaInstance.Get<DashboardStats>("/dashboard/stats");

export const getLibraries = () => alovaInstance.Get<LibraryRoot[]>("/libraries");

export const createLibrary = (request: CreateLibraryRequest) =>
  alovaInstance.Post<LibraryRoot>("/libraries", request);

export const updateLibrary = (id: string, request: UpdateLibraryRequest) =>
  alovaInstance.Patch<LibraryRoot>(`/libraries/${id}`, request);

export const deleteLibrary = (id: string) => alovaInstance.Delete<void>(`/libraries/${id}`);

export const preflightLibraryPath = (path: string) =>
  alovaInstance.Post<PathPreflight>("/libraries/preflight", { path });

export const getDirectories = (path: string) =>
  alovaInstance.Get<DirectoryListing>("/filesystem/directories", { params: { path } });

export const getRuntimeSettings = () => alovaInstance.Get<RuntimeSettings>("/settings");

export const updateRuntimeSettings = (request: {
  values: EditableSettings;
  aiApiKey?: string;
  clearAiApiKey?: boolean;
}) => alovaInstance.Patch<RuntimeSettings>("/settings", request);

export const scanLibrary = (id: string) => alovaInstance.Post<Job>(`/libraries/${id}/scan`);

export const getTracks = (page: number, perPage: number, search: string, filter?: TrackFilter) =>
  alovaInstance.Get<TrackList>("/tracks", {
    params: { page, perPage, search: search || undefined, filter },
  });

export const getTrack = (id: string) => alovaInstance.Get<TrackDetail>(`/tracks/${id}`);

export const getTrackFiles = (id: string) => alovaInstance.Get<MediaFile[]>(`/tracks/${id}/files`);

export const getTrackOperations = (id: string) =>
  alovaInstance.Get<TrackOperationHistory[]>(`/tracks/${id}/operations`);

export const previewTags = (request: {
  mediaIds: string[];
  set?: Partial<{
    title: string;
    artists: string[];
    album: string;
    albumArtist: string;
    trackNo: number;
    discNo: number;
    year: number;
    genre: string;
    coverDataBase64: string;
  }>;
  clear?: TagField[];
  transforms?: TagTransform[];
}) => alovaInstance.Post<Operation>("/tags/preview", request);

export const applyTags = (operationId: string) =>
  alovaInstance.Post<Operation>("/tags/apply", { operationId, confirmation: "APPLY" });

export const undoTags = (operationId: string) =>
  alovaInstance.Post<Operation>("/tags/undo", { operationId, confirmation: "UNDO" });

export const previewOrganizer = (request: {
  mediaIds: string[];
  targetLibraryId: string;
  template: string;
  crossPlatformSafe: boolean;
}) => alovaInstance.Post<Operation>("/organizer/preview", request);

export const applyOrganizer = (operationId: string) =>
  alovaInstance.Post<Operation>("/organizer/apply", { operationId, confirmation: "APPLY" });

export const undoOrganizer = (operationId: string) =>
  alovaInstance.Post<Operation>("/organizer/undo", { operationId, confirmation: "UNDO" });

export const previewTrash = (mediaIds: string[]) =>
  alovaInstance.Post<Operation>("/trash/preview", { mediaIds });

export const applyTrash = (operationId: string) =>
  alovaInstance.Post<Operation>("/trash/apply", { operationId, confirmation: "TRASH" });

export const restoreTrash = (operationId: string) =>
  alovaInstance.Post<Operation>("/trash/restore", { operationId, confirmation: "RESTORE" });

export const getTrashEntries = () => alovaInstance.Get<TrashEntry[]>("/trash");

export const previewTrashPurge = (trashOperationId: string) =>
  alovaInstance.Post<TrashPurge>("/trash/purge/preview", { trashOperationId });

export const applyTrashPurge = (purgeId: string) =>
  alovaInstance.Post<TrashPurge>("/trash/purge/apply", {
    purgeId,
    confirmation: "PURGE_PERMANENTLY",
  });

export const getProviders = () => alovaInstance.Get<ProviderSetting[]>("/providers");

export const updateProvider = (
  id: string,
  request: {
    enabled?: boolean;
    priority?: number;
    baseUrl?: string;
    timeoutMs?: number;
    rateLimitMs?: number;
  }
) => alovaInstance.Patch<ProviderSetting>(`/providers/${id}`, request);

export const searchScrapeCandidates = (trackId: string, providerIds: string[] = []) =>
  alovaInstance.Post<ScrapeSearchResponse>("/scrape/search", { trackId, providerIds });

export const getScrapeCandidates = (trackId: string) =>
  alovaInstance.Get<ScrapeCandidate[]>(`/tracks/${trackId}/scrape-candidates`);

export const previewScrapeCandidate = (candidate: ScrapeCandidate, includeArtwork: boolean) =>
  alovaInstance.Post<Operation>("/scrape/apply", {
    candidateId: candidate.id,
    confirmation:
      candidate.score >= 95
        ? "APPLY"
        : candidate.score >= 80
          ? "APPLY_REVIEWED"
          : "APPLY_LOW_CONFIDENCE",
    includeArtwork,
  });

export const createScrapeJob = (trackIds: string[], providerIds: string[] = []) =>
  alovaInstance.Post<Job>("/scrape/jobs", { trackIds, providerIds });

export const getLyrics = (trackId: string) =>
  alovaInstance.Get<LyricsRecord[]>(`/tracks/${trackId}/lyrics`);

export const searchLyrics = (trackId: string) =>
  alovaInstance.Post<LyricsSearchResponse>("/lyrics/search", { trackId });

export const applyLyrics = (request: {
  jobId: string;
  lyricsId: string;
  mediaFileId: string;
  mode: "external" | "embedded" | "both";
  replaceExisting: boolean;
}) =>
  alovaInstance.Post<LyricsRecord>("/lyrics/apply", {
    ...request,
    confirmation: "USE_LYRICS",
  });

export const analyzeDuplicates = (mediaIds: string[] = []) =>
  alovaInstance.Post<Job>("/duplicates/analyze", {
    mediaIds,
    calculateHash: true,
    calculateFingerprint: true,
  });

export const getDuplicateGroups = (kind?: DuplicateGroup["kind"]) =>
  alovaInstance.Get<DuplicateGroup[]>("/duplicates/groups", { params: { kind } });

export const getAiStatus = () => alovaInstance.Get<AiStatus>("/ai/status");

export const explainDuplicate = (groupId: string) =>
  alovaInstance.Post<AiRecommendation>("/ai/duplicates/explain", {
    groupId,
    confirmation: "SEND_METADATA",
  });

export const getPlayToken = (mediaId: string) =>
  alovaInstance.Get<PlayTokenResponse>(`/media/${mediaId}/play-token`);

export const scrobble = (request: {
  trackId: string;
  mediaFileId: string;
  completed: boolean;
  positionSec?: number;
}) => alovaInstance.Post<void>("/playback/scrobble", { ...request, client: "meloark-web" });

export const getFavorites = () => alovaInstance.Get<string[]>("/favorites");
export const starTrack = (trackId: string) => alovaInstance.Put<void>(`/favorites/${trackId}`);
export const unstarTrack = (trackId: string) => alovaInstance.Delete<void>(`/favorites/${trackId}`);

export const getPlaybackHistory = () => alovaInstance.Get<PlaybackHistory[]>("/playback/history");

export const getPlaylists = () => alovaInstance.Get<Playlist[]>("/playlists");

export const createPlaylist = (request: { name: string; comment?: string; trackIds: string[] }) =>
  alovaInstance.Post<Playlist>("/playlists", request);

export const deletePlaylist = (id: string) => alovaInstance.Delete<void>(`/playlists/${id}`);
