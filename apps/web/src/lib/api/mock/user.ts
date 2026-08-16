import { defineMock } from "@alova/mock";

const now = "2026-07-29T08:00:00Z";
let mockUser = {
  id: "019fc04f-3bb0-7c26-a9b2-c91cfc102042",
  username: "admin",
  passwordChangeRequired: false,
  createdAt: now,
  updatedAt: now,
};

let runtimeSettings = {
  scanWorkers: 2,
  reconcileIntervalSec: 21600,
  watchDebounceSec: 5,
  sourceCacheTtlSec: 86400,
  sourceRetryAttempts: 2,
  sourceCircuitBreakerFailures: 3,
  sourceCircuitBreakerCooldownSec: 300,
  analysisWorkers: 1,
  fingerprintThreshold: 0.88,
  aiEnabled: false,
  aiBaseUrl: "https://api.openai.com",
  aiModel: "",
  aiTimeoutSec: 30,
  transcodeWorkers: 2,
  transcodeCacheMaxBytes: 10737418240,
  organizerTemplate: "{artist}/{album}/{track:02} - {title}.{ext}",
  organizerCrossPlatformSafe: true,
};

let setupRequired = true;

export default defineMock({
  "[GET]/health": {
    status: "ok",
    service: "mock-backend",
    version: "1.0.0",
  },
  "[GET]/auth/setup-status": () => ({ setupRequired }),
  "[POST]/auth/setup": ({ data }) => {
    setupRequired = false;
    return {
      status: 201,
      statusText: "Created",
      body: {
        ...mockUser,
        username: data?.username ?? mockUser.username,
      },
    };
  },
  "[POST]/auth/login": { token: "mock-jwt-token", passwordChangeRequired: false },
  "[GET]/auth/profile": () => mockUser,
  "[PATCH]/auth/profile": ({ data }) => {
    mockUser = { ...mockUser, username: data?.username ?? mockUser.username, updatedAt: now };
    return { user: mockUser, token: "mock-jwt-token" };
  },
  "[GET]/settings": () => ({
    values: runtimeSettings,
    aiApiKeyConfigured: false,
    lockedByEnvironment: [],
    restartRequiredFields: ["scanWorkers", "analysisWorkers", "transcodeWorkers"],
    infrastructure: {
      host: "127.0.0.1",
      port: 31000,
      databasePath: "/data/meloark.db",
      ffmpegPath: "ffmpeg",
      fpcalcPath: "fpcalc",
      transcodeCacheDir: "/data/cache/transcode",
      platform: "linux/amd64",
    },
  }),
  "[PATCH]/settings": ({ data }) => {
    runtimeSettings = data?.values ?? runtimeSettings;
    return {
      values: runtimeSettings,
      aiApiKeyConfigured: Boolean(data?.aiApiKey),
      lockedByEnvironment: [],
      restartRequiredFields: ["scanWorkers", "analysisWorkers", "transcodeWorkers"],
      infrastructure: {
        host: "127.0.0.1",
        port: 31000,
        databasePath: "/data/meloark.db",
        ffmpegPath: "ffmpeg",
        fpcalcPath: "fpcalc",
        transcodeCacheDir: "/data/cache/transcode",
        platform: "linux/amd64",
      },
    };
  },
  "[GET]/filesystem/directories": {
    currentPath: "/",
    directories: [
      { name: "data", path: "/data", readable: true },
      { name: "music", path: "/music", readable: true },
    ],
  },
  "[GET]/dashboard/stats": {
    libraryCount: 0,
    artistCount: 0,
    albumCount: 0,
    trackCount: 0,
    mediaFileCount: 0,
    availableManagedFileCount: 0,
    pendingReviewCount: 0,
    totalBytes: 0,
    missingTagCount: 0,
    runningJobCount: 0,
  },
  "[GET]/libraries": [],
  "[GET]/jobs": [],
  "[GET]/tracks": { items: [], page: 1, perPage: 50, total: 0 },
});
