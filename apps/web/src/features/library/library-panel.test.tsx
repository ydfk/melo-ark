import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { LibraryPanel } from "./library-panel";

vi.mock("@/features/tasks/job-activity-context", () => ({
  useJobActivity: () => ({ latestJob: () => undefined, registerJob: vi.fn() }),
}));

vi.mock("@/lib/api/methods/library", () => ({
  createLibrary: vi.fn(() => ({ send: vi.fn() })),
  createScrapeJob: vi.fn(() => ({ send: vi.fn() })),
  getDirectories: vi.fn(() => ({ send: vi.fn().mockResolvedValue({ directories: [] }) })),
  getTracks: vi.fn(() => ({
    send: vi.fn().mockResolvedValue({ items: [], page: 1, perPage: 50, total: 0 }),
  })),
  preflightLibraryPath: vi.fn(() => ({ send: vi.fn() })),
  scanLibrary: vi.fn(() => ({ send: vi.fn() })),
}));

describe("LibraryPanel", () => {
  it("添加曲库只显示来源和整理后目录", async () => {
    const user = userEvent.setup();
    render(<LibraryPanel libraries={[]} onChanged={vi.fn()} />);

    expect(screen.queryByRole("button", { name: "添加曲库" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "添加第一个曲库" }));

    expect(screen.getByText("来源目录")).toBeInTheDocument();
    expect(screen.getByText("整理后目录")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "选择目录" })).toHaveLength(2);
    expect(screen.queryByText("用途")).not.toBeInTheDocument();
    expect(screen.queryByText("允许后续写入")).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "自动处理新增音乐" })).toBeChecked();
  });
});
