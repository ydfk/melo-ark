import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ManagementMiniPlayer } from "./management-mini-player";

const togglePlaying = vi.fn();
const openPlayer = vi.fn();

vi.mock("@/features/player/player-context", () => ({
  usePlayer: () => ({
    current: {
      id: "track-1",
      mediaId: "media-1",
      title: "夜曲",
      artist: "周杰伦",
      album: "十一月的萧邦",
    },
    playing: false,
    position: 30,
    duration: 240,
    volume: 0.8,
    artworkUrl: undefined,
    seek: vi.fn(),
    setVolume: vi.fn(),
    playPrevious: vi.fn(),
    playNext: vi.fn(),
    togglePlaying,
  }),
}));

describe("ManagementMiniPlayer", () => {
  it("复用播放上下文并提供基础控制", () => {
    const { container } = render(<ManagementMiniPlayer onOpenPlayer={openPlayer} />);

    expect(screen.getAllByText("夜曲").length).toBeGreaterThan(0);
    expect(screen.getByText(/周杰伦 · 十一月的萧邦/)).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("button", { name: "播放" })[0]);
    expect(togglePlaying).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "打开完整播放器" }));
    expect(openPlayer).toHaveBeenCalledOnce();
    expect(container.querySelector("audio")).toBeNull();
  });
});
