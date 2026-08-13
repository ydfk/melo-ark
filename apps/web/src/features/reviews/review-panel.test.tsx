import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { ReviewPanel } from "./review-panel";

const send = vi.fn();

vi.mock("@/lib/api/methods/reviews", () => ({
  getReviews: () => ({ send }),
  updateReview: vi.fn(),
  previewReviewBatch: vi.fn(),
  applyReviewBatch: vi.fn(),
}));

describe("ReviewPanel", () => {
  beforeEach(() => {
    send.mockResolvedValue({
      total: 1,
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
});
