import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { JobLogSheet } from "./job-log-sheet";

vi.mock("@/lib/api/methods/jobs", () => ({
  getJobLogs: () => ({
    send: async () => ({
      items: [
        {
          id: 1,
          jobId: "job-1",
          level: "info",
          eventType: "success",
          itemKey: "album/song.flac",
          message: "处理成功",
          createdAt: "2026-08-12T10:00:00Z",
        },
      ],
    }),
  }),
}));

describe("JobLogSheet", () => {
  test("renders structured task logs", async () => {
    render(
      <JobLogSheet
        open
        onOpenChange={vi.fn()}
        liveLogs={[]}
        job={{
          id: "job-1",
          kind: "scan",
          status: "completed",
          totalItems: 1,
          processedItems: 1,
          successItems: 1,
          skippedItems: 0,
          failedItems: 0,
          createdAt: "2026-08-12T10:00:00Z",
          updatedAt: "2026-08-12T10:00:00Z",
        }}
      />
    );

    expect(await screen.findByText("处理成功", { selector: "p" })).toBeInTheDocument();
    expect(screen.getByText("处理成功", { selector: "span" })).toBeInTheDocument();
    expect(screen.getByText("album/song.flac")).toBeInTheDocument();
  });
});
