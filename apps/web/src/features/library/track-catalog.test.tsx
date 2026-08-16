import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TrackCatalog } from "./track-catalog";

describe("TrackCatalog", () => {
  it("shows complete server pagination controls", async () => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    const onPerPageChange = vi.fn();
    render(
      <TrackCatalog
        tracks={{ items: [], page: 2, perPage: 50, total: 120 }}
        loading={false}
        search=""
        page={2}
        perPage={50}
        selected={new Set()}
        onSearchChange={vi.fn()}
        onSearch={vi.fn()}
        onFilterChange={vi.fn()}
        onPageChange={onPageChange}
        onPerPageChange={onPerPageChange}
        onSelectionChange={vi.fn()}
        onOpenTrack={vi.fn()}
        onPlayTrack={vi.fn()}
        onBatchTag={vi.fn()}
        onBatchScrape={vi.fn()}
      />
    );

    expect(screen.getByText("第 2 / 3 页")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "首页" }));
    await user.click(screen.getByRole("button", { name: "上一页" }));
    await user.click(screen.getByRole("button", { name: "下一页" }));
    await user.click(screen.getByRole("button", { name: "末页" }));
    expect(onPageChange.mock.calls.map(([page]) => page)).toEqual([1, 1, 3, 3]);

    await user.click(screen.getByRole("combobox", { name: "每页整理文件数量" }));
    await user.click(screen.getByRole("option", { name: "每页 100 个文件" }));
    expect(onPerPageChange).toHaveBeenCalledWith(100);
  });

  it("keeps different organized files of the same track as separate rows", async () => {
    const user = userEvent.setup();
    const onSelectionChange = vi.fn();
    const onOpenTrack = vi.fn();
    const onPlayTrack = vi.fn();
    const common = {
      trackId: "track-1",
      organizedLibraryId: "library-1",
      organizedPath: "/music/organized",
      title: "同一首歌",
      artist: "歌手",
      album: "专辑",
      fileSize: 1024,
      hasLyrics: false,
      hasArtwork: false,
      tagHealth: "complete" as const,
    };

    render(
      <TrackCatalog
        tracks={{
          items: [
            {
              ...common,
              mediaId: "media-flac",
              relativePath: "歌手/专辑/同一首歌.flac",
              path: "/music/organized/歌手/专辑/同一首歌.flac",
              codec: "flac",
              extension: "flac",
            },
            {
              ...common,
              mediaId: "media-wav",
              relativePath: "歌手/专辑/同一首歌.wav",
              path: "/music/organized/歌手/专辑/同一首歌.wav",
              codec: "pcm_s16le",
              extension: "wav",
            },
          ],
          page: 1,
          perPage: 50,
          total: 2,
        }}
        loading={false}
        search=""
        page={1}
        perPage={50}
        selected={new Set()}
        onSearchChange={vi.fn()}
        onSearch={vi.fn()}
        onFilterChange={vi.fn()}
        onPageChange={vi.fn()}
        onPerPageChange={vi.fn()}
        onSelectionChange={onSelectionChange}
        onOpenTrack={onOpenTrack}
        onPlayTrack={onPlayTrack}
        onBatchTag={vi.fn()}
        onBatchScrape={vi.fn()}
      />
    );

    expect(screen.getByText("共 2 个整理文件")).toBeInTheDocument();
    expect(screen.getAllByText("同一首歌")).toHaveLength(2);
    await user.click(screen.getByRole("checkbox", { name: "选择当前页全部整理文件" }));
    expect(onSelectionChange).toHaveBeenCalledWith(new Set(["media-flac", "media-wav"]));
    await user.click(screen.getAllByRole("button", { name: "播放 同一首歌" })[1]);
    expect(onPlayTrack).toHaveBeenCalledWith("media-wav");
    await user.click(screen.getAllByRole("button", { name: "同一首歌" })[0]);
    expect(onOpenTrack).toHaveBeenCalledWith("track-1");
  });
});
