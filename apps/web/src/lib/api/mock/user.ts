import { defineMock } from "@alova/mock";

const now = "2026-07-29T08:00:00Z";
const mockUser = {
  id: "019fc04f-3bb0-7c26-a9b2-c91cfc102042",
  username: "admin",
  createdAt: now,
  updatedAt: now,
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
  "[POST]/auth/login": { token: "mock-jwt-token" },
  "[GET]/auth/profile": mockUser,
  "[GET]/dashboard/stats": {
    libraryCount: 0,
    artistCount: 0,
    albumCount: 0,
    trackCount: 0,
    mediaFileCount: 0,
    totalBytes: 0,
    missingTagCount: 0,
    runningJobCount: 0,
  },
  "[GET]/libraries": [],
  "[GET]/jobs": [],
  "[GET]/tracks": { items: [], page: 1, perPage: 50, total: 0 },
});
