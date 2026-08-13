import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

    expect(screen.getByText("3/3")).toBeInTheDocument();
    expect(screen.getByText(/成功 3/)).toBeInTheDocument();
    expect(screen.getByText("已完成")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "查看日志" })).toBeInTheDocument();
  });

  test("folds legacy ingest jobs by parent scan", async () => {
    const user = userEvent.setup();
    const base = {
      kind: "ingest",
      status: "completed" as const,
      parentJobId: "scan-1",
      totalItems: 1,
      processedItems: 1,
      successItems: 1,
      skippedItems: 0,
      failedItems: 0,
      createdAt: "2026-08-10T10:00:00Z",
      updatedAt: "2026-08-10T10:00:00Z",
    };
    render(
      <TasksPanel
        jobs={[
          { ...base, id: "ingest-1" },
          { ...base, id: "ingest-2" },
        ]}
        onChanged={vi.fn()}
      />
    );

    expect(screen.getByText("历史新增音乐接入 · 2 个任务")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查看日志" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "展开历史任务" }));
    expect(screen.getAllByRole("button", { name: "查看日志" })).toHaveLength(2);
  });
});
