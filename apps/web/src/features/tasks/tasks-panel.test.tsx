import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { TasksPanel } from "./tasks-panel";

describe("TasksPanel", () => {
  test("renders completed jobs when optional metrics are null", () => {
    render(
      <TasksPanel
        jobs={[
          {
            id: "job-1",
            kind: "scan",
            status: "completed",
            totalItems: 3,
            processedItems: 3,
            successItems: 3,
            skippedItems: 0,
            failedItems: 0,
            createdAt: "2026-08-10T10:00:00Z",
            startedAt: "2026-08-10T10:00:00Z",
            finishedAt: "2026-08-10T10:00:00Z",
            updatedAt: "2026-08-10T10:00:00Z",
            itemsPerSecond: null,
            etaSeconds: null,
          },
        ]}
        onChanged={vi.fn()}
      />
    );

    expect(screen.getByText("等待采样")).toBeInTheDocument();
    expect(screen.getByText("已完成", { selector: "span" })).toBeInTheDocument();
  });
});
