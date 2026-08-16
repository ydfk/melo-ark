import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { ReviewPanel } from "./review-panel";

const send = vi.fn();
const getReviews = vi.fn((_query?: unknown) => ({ send }));

vi.mock("@/lib/api/methods/reviews", () => ({
  getReviews: (query: unknown) => getReviews(query),
  updateReview: vi.fn(),
  clearReviewMarks: vi.fn(),
  previewReviewBatch: vi.fn(),
  getReviewBatchPreviewItems: vi.fn(),
  applyReviewBatch: vi.fn(),
}));

describe("ReviewPanel", () => {
  beforeEach(() => {
    send.mockResolvedValue({
      total: 1,
      page: 1,
      perPage: 25,
      markedTotal: 0,
      items: [
        {
          id: "review-1",
          kind: "missing_lyrics",
          status: "pending",
          marked: false,
          title: "夜曲",
          detail: "没有可用歌词",
          createdAt: "2026-08-12T10:00:00Z",
          updatedAt: "2026-08-12T10:00:00Z",
        },
      ],
    });
  });

  test("集中展示需要用户判断的项目", async () => {
    render(<ReviewPanel onChanged={vi.fn()} />);

    expect(await screen.findByText("夜曲")).toBeInTheDocument();
    expect(screen.getByText("缺少歌词")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "生成处理预览" })).toBeDisabled();
  });

  test("只渲染当前页并可切换到下一页", async () => {
    const user = userEvent.setup();
    send
      .mockResolvedValueOnce({
        total: 60,
        page: 1,
        perPage: 25,
        markedTotal: 0,
        items: [
          {
            id: "review-1",
            kind: "missing_lyrics",
            status: "pending",
            marked: false,
            title: "第一页歌曲",
            detail: "没有歌词",
            createdAt: "2026-08-12T10:00:00Z",
            updatedAt: "2026-08-12T10:00:00Z",
          },
        ],
      })
      .mockResolvedValueOnce({
        total: 60,
        page: 2,
        perPage: 25,
        markedTotal: 0,
        items: [
          {
            id: "review-26",
            kind: "missing_lyrics",
            status: "pending",
            marked: false,
            title: "第二页歌曲",
            detail: "没有歌词",
            createdAt: "2026-08-12T10:00:00Z",
            updatedAt: "2026-08-12T10:00:00Z",
          },
        ],
      });
    render(<ReviewPanel onChanged={vi.fn()} />);

    expect(await screen.findByText("第一页歌曲")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(await screen.findByText("第二页歌曲")).toBeInTheDocument();
    expect(getReviews).toHaveBeenLastCalledWith(expect.objectContaining({ page: 2, perPage: 25 }));
  });
});
