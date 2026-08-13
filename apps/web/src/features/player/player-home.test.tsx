import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { AuthProvider } from "@/features/auth/auth-context";
import { LoginDialog } from "@/features/auth/login-dialog";

import { PlayerProvider } from "./player-context";
import { PlayerHome } from "./player-home";

const { tracksMock, albumsMock } = vi.hoisted(() => ({
  tracksMock: vi.fn(),
  albumsMock: vi.fn(),
}));

vi.mock("@/lib/api/methods/catalog", async () => {
  const actual = await vi.importActual<object>("@/lib/api/methods/catalog");
  return {
    ...actual,
    getCatalogTracks: () => ({ send: tracksMock }),
    getCatalogAlbums: () => ({ send: albumsMock }),
  };
});

vi.mock("@/lib/api/methods/library", () => ({
  getFavorites: () => ({ send: vi.fn().mockResolvedValue([]) }),
  starTrack: () => ({ send: vi.fn() }),
  unstarTrack: () => ({ send: vi.fn() }),
  scrobble: () => ({ send: vi.fn() }),
  getPlaylists: () => ({ send: vi.fn().mockResolvedValue([]) }),
  createPlaylist: () => ({ send: vi.fn() }),
}));

vi.mock("@/lib/api/methods/user", () => ({
  getProfile: () => ({ send: vi.fn() }),
  login: () => ({ send: vi.fn() }),
  updateProfile: () => ({ send: vi.fn() }),
}));

describe("PlayerHome", () => {
  beforeEach(() => {
    localStorage.clear();
    tracksMock.mockResolvedValue({
      items: [
        {
          id: "track-1",
          mediaId: "media-1",
          title: "夜航星",
          artist: "MeloArk",
          album: "私人唱片",
          durationMs: 210000,
          hasLyrics: true,
          hasArtwork: false,
        },
      ],
      page: 1,
      perPage: 48,
      total: 1,
    });
    albumsMock.mockResolvedValue([]);
  });

  test("访客直接进入播放器并能浏览精选音乐", async () => {
    renderPlayer(vi.fn());

    expect(screen.getByRole("heading", { name: "让下一首歌转起来" })).toBeInTheDocument();
    expect(await screen.findByText("夜航星")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录" })).toBeInTheDocument();
  });

  test("访客进入管理时打开登录弹窗而不是离开播放器", async () => {
    const onOpenManagement = vi.fn();
    renderPlayer(onOpenManagement);
    await screen.findByText("夜航星");

    fireEvent.click(screen.getByRole("button", { name: /管理/ }));

    await waitFor(() => expect(screen.getByRole("dialog")).toBeInTheDocument());
    expect(screen.getByText("登录 MeloArk")).toBeInTheDocument();
    expect(onOpenManagement).not.toHaveBeenCalled();
  });
});

function renderPlayer(onOpenManagement: () => void) {
  return render(
    <AuthProvider>
      <PlayerProvider>
        <PlayerHome onOpenManagement={onOpenManagement} />
        <LoginDialog />
      </PlayerProvider>
    </AuthProvider>
  );
}
