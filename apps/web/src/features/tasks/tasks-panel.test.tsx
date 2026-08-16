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

  test("shows current ingest phase and hides internal index scans", () => {
    render(
      <TasksPanel
        jobs={[
          {
            id: "ingest-1",
            kind: "ingest",
            status: "running",
            totalItems: 1180,
            processedItems: 0,
            successItems: 20,
            skippedItems: 0,
            failedItems: 1,
            sourcePath: "/music/source",
            targetPath: "/music/organized",
            phase: "linking",
            phaseProcessedItems: 21,
            phaseTotalItems: 1180,
            createdAt: "2026-08-16T10:00:00Z",
            updatedAt: "2026-08-16T10:01:00Z",
          },
          {
            id: "internal-scan-1",
            kind: "scan",
            status: "running",
            internal: true,
            targetPath: "/music/organized",
            phase: "scanning",
            phaseProcessedItems: 300,
            phaseTotalItems: 1180,
            totalItems: 1180,
            processedItems: 300,
            successItems: 300,
            skippedItems: 0,
            failedItems: 0,
            createdAt: "2026-08-16T10:01:00Z",
            updatedAt: "2026-08-16T10:02:00Z",
          },
        ]}
        onChanged={vi.fn()}
      />
    );

    expect(screen.getByText("整理新增音乐")).toBeInTheDocument();
    expect(screen.getByText("/music/source → /music/organized")).toBeInTheDocument();
    expect(screen.getByText("创建整理硬链接 · 21/1180")).toBeInTheDocument();
    expect(screen.queryByText("更新整理目录索引")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "查看日志" })).toHaveLength(1);
  });
});
