export type HealthResponse = {
  status: "ok";
  service: string;
  version: string;
};

export type Credentials = {
  username: string;
  password: string;
};

export type SetupStatusResponse = {
  setupRequired: boolean;
};

export type UserResponse = {
  id: string;
  username: string;
  createdAt: string;
  updatedAt: string;
};

export type TokenResponse = {
  token: string;
};

export type Problem = {
  status: number;
  title: string;
  detail: string;
  errors?: Array<{
    message: string;
    location?: string;
    value?: unknown;
  }>;
};

export type DashboardStats = {
  libraryCount: number;
  artistCount: number;
  albumCount: number;
  trackCount: number;
  mediaFileCount: number;
  totalBytes: number;
  missingTagCount: number;
  missingLyricsCount: number;
  missingCoverCount: number;
  possibleDuplicateCount: number;
  exactDuplicateCount: number;
  runningJobCount: number;
  recentScanAt?: string;
  formatDistribution: Array<{ extension: string; count: number; totalBytes: number }>;
  recentAdded: Array<{
    id: string;
    mediaId: string;
    title: string;
    artist: string;
    album: string;
    hasArtwork: boolean;
    createdAt: string;
  }>;
  recentPlayed: Array<{
    trackId: string;
    title: string;
    artist: string;
    client: string;
    playedAt: string;
  }>;
};

export type LibraryRoot = {
  id: string;
  name: string;
  path: string;
  scanEnabled: boolean;
  watchEnabled: boolean;
  writable: boolean;
  role: "source" | "managed" | "both";
  excludePatterns: string[];
  lastScanAt?: string;
  createdAt: string;
  updatedAt: string;
};

export type CreateLibraryRequest = {
  name: string;
  path: string;
  scanEnabled: boolean;
  watchEnabled: boolean;
  writable: boolean;
  role: LibraryRoot["role"];
  excludePatterns?: string[];
};

export type UpdateLibraryRequest = Partial<CreateLibraryRequest>;

export type PathPreflight = {
  canonicalPath: string;
  exists: boolean;
  directory: boolean;
  readable: boolean;
  writable: boolean;
  deviceId: string;
};

export type JobStatus =
  | "queued"
  | "running"
  | "paused"
  | "cancel_requested"
  | "cancelled"
  | "completed"
  | "completed_with_errors"
  | "failed"
  | "interrupted";

export type Job = {
  id: string;
  kind: string;
  status: JobStatus;
  libraryId?: string;
  totalItems: number;
  processedItems: number;
  successItems: number;
  skippedItems: number;
  failedItems: number;
  currentItem?: string;
  errorMessage?: string;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
  updatedAt: string;
  itemsPerSecond?: number | null;
  etaSeconds?: number | null;
};

export type JobEvent = {
  event: "job.updated";
  job: Job;
};

export type Track = {
  id: string;
  mediaId: string;
  title: string;
  artist: string;
  album: string;
  year?: number;
  durationMs?: number;
  variantCount: number;
  totalBytes: number;
  codec?: string;
  extension: string;
  sampleRate?: number;
  bitDepth?: number;
  qualityScore?: number;
  hasLyrics: boolean;
  hasArtwork: boolean;
  tagHealth: "complete" | "missing";
  path: string;
};

export type TrackFilter = "missing_lyrics" | "missing_cover" | "missing_tags" | "duplicates";

export type TrackList = {
  items: Track[];
  page: number;
  perPage: number;
  total: number;
};

export type TrackDetail = {
  id: string;
  title: string;
  artists: string;
  album: string;
  albumArtist?: string;
  trackNo?: number;
  discNo?: number;
  year?: number;
  genre?: string;
  durationMs?: number;
  versionLabel?: string;
};

export type MediaFile = {
  id: string;
  libraryId: string;
  libraryName: string;
  path: string;
  extension: string;
  fileSize: number;
  deviceId: string;
  inode: string;
  hardlinkCount: number;
  codec?: string;
  durationMs?: number;
  bitrate?: number;
  sampleRate?: number;
  bitDepth?: number;
  hasArtwork: boolean;
  metadataWritable: boolean;
  libraryWritable: boolean;
};

export type TagField =
  | "title"
  | "artists"
  | "album"
  | "albumArtist"
  | "trackNo"
  | "discNo"
  | "year"
  | "genre"
  | "cover";

export type TagTransform =
  | { kind: "trim"; fields: TagField[] }
  | { kind: "findReplace"; fields: TagField[]; find: string; replacement: string }
  | { kind: "regexReplace"; fields: TagField[]; pattern: string; replacement: string }
  | { kind: "traditionalToSimplified"; fields: TagField[] }
  | { kind: "normalizePunctuation"; fields: TagField[] }
  | { kind: "filenameToTags" };

export type OperationItem = {
  id: string;
  mediaFileId?: string;
  sourcePath?: string;
  targetPath?: string;
  status: string;
  diffs: Array<{ field: string; before?: string; after?: string }>;
  errorMessage?: string;
  preflight?: {
    sameFilesystem: boolean;
    targetExists: boolean;
    sameInode: boolean;
    pathConflict: boolean;
    canApply: boolean;
  };
};

export type Operation = {
  id: string;
  kind: "tag_edit" | "organize" | "trash";
  status: string;
  items: OperationItem[];
};

export type TrashEntry = {
  operationId: string;
  status: string;
  createdAt: string;
  finishedAt?: string;
  itemCount: number;
  totalBytes: number;
  purgeId?: string;
  purgeStatus?: string;
};

export type TrashPurge = {
  id: string;
  trashOperationId: string;
  status: "previewed" | "running" | "completed" | "completed_with_errors";
  totalItems: number;
  totalBytes: number;
  createdAt: string;
  confirmedAt?: string;
  finishedAt?: string;
  items: Array<{
    id: string;
    path: string;
    expectedSize: number;
    status: "previewed" | "success" | "failed";
    errorMessage?: string;
  }>;
};

export type ProviderSetting = {
  providerId: string;
  displayName: string;
  kind: "metadata" | "lyrics" | "both";
  enabled: boolean;
  priority: number;
  maturity: "stable" | "beta";
  baseUrl?: string;
  timeoutMs: number;
  rateLimitMs: number;
  consecutiveFailures: number;
  circuitOpenUntil?: string;
  lastSuccessAt?: string;
  lastError?: string;
  capabilities?: { metadata: boolean; artwork: boolean; lyrics: boolean };
};

export type ScrapeCandidate = {
  id: string;
  trackId: string;
  providerId: string;
  providerItemId: string;
  title: string;
  artistsJson: string[];
  album?: string;
  durationMs?: number;
  year?: number;
  trackNo?: number;
  versionLabel?: string;
  artworkUrl?: string;
  score: number;
  confidence: "high" | "review" | "low";
  differencesJson: string[];
};

export type ScrapeSearchResponse = {
  candidates: ScrapeCandidate[];
  failures: Array<{ providerId: string; code: string; message: string }>;
};

export type LyricsRecord = {
  id: string;
  trackId: string;
  mediaFileId?: string;
  providerId?: string;
  providerItemId?: string;
  format: "plain" | "lrc";
  language?: string;
  content: string;
  translatedContent?: string;
  synced: boolean;
  coveragePercent: number;
  qualityScore: number;
  storage: "candidate" | "external" | "embedded" | "both";
  externalPath?: string;
  active: boolean;
};

export type LyricsSearchResponse = {
  candidates: LyricsRecord[];
  failures: Array<{ providerId: string; message: string }>;
};

export type DuplicateMember = {
  mediaFileId: string;
  trackId: string;
  title: string;
  versionLabel?: string;
  artist: string;
  path: string;
  extension: string;
  fileSize: number;
  deviceId: string;
  inode: string;
  codec?: string;
  bitrate?: number;
  sampleRate?: number;
  bitDepth?: number;
  durationMs?: number;
  hasArtwork: boolean;
  similarity?: number;
  qualityScore: number;
  recommendedKeep: boolean;
};

export type DuplicateGroup = {
  id: string;
  kind:
    | "hardlink_alias"
    | "binary_exact"
    | "audio_duplicate"
    | "quality_variant"
    | "possible_duplicate";
  confidence: number;
  reclaimableBytes: number;
  reason: string;
  members: DuplicateMember[];
};

export type AiStatus = {
  enabled: boolean;
  baseUrl: string;
  model: string;
  apiKeyConfigured: boolean;
  uploadsAudio: false;
};

export type AiRecommendation = { id: string; relation: string; confidence: number; reason: string };

export type PlayTokenResponse = { token: string; expiresIn: number };

export type PlaybackHistory = {
  id: string;
  trackId: string;
  title: string;
  artist: string;
  client: string;
  playedAt: string;
  completed: boolean;
};

export type TrackOperationHistory = {
  id: string;
  operationId: string;
  kind: string;
  action: string;
  status: string;
  sourcePath?: string;
  targetPath?: string;
  errorMessage?: string;
  createdAt: string;
  confirmedAt?: string;
  finishedAt?: string;
  updatedAt: string;
};

export type Playlist = {
  id: string;
  name: string;
  comment?: string;
  songCount: number;
  durationSec: number;
  createdAt: string;
  updatedAt: string;
};
