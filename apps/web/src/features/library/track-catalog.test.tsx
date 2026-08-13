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

    await user.click(screen.getByRole("combobox", { name: "每页歌曲数量" }));
    await user.click(screen.getByRole("option", { name: "每页 100 首" }));
    expect(onPerPageChange).toHaveBeenCalledWith(100);
  });
});
