import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { DashboardStats, Job } from "@/lib/api/types";

import { ManagementPipeline } from "./management-pipeline";

let jobs: Job[] = [];
const openLogs = vi.fn();

vi.mock("@/features/tasks/job-activity-context", () => ({
  useJobActivity: () => ({ jobs, openLogs }),
}));

const stats = {
  pendingReviewCount: 12,
  availableManagedFileCount: 86,
} as DashboardStats;

describe("ManagementPipeline", () => {
  beforeEach(() => {
    jobs = [];
    openLogs.mockClear();
  });

  test("用真实数量串联管理阶段并支持导航", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(
      <ManagementPipeline
        stats={stats}
        libraries={[
          {
            organizedLibraryId: "managed-1",
            organizedPath: "/music/organized",
            status: "ready",
            sources: [
              {
                id: "source-1",
                sourcePath: "/music/source",
                scanEnabled: true,
                watchEnabled: false,
                autoIngestEnabled: true,
                excludePatterns: [],
                createdAt: "2026-08-16T10:00:00Z",
                updatedAt: "2026-08-16T10:00:00Z",
              },
            ],
          },
        ]}
        activeTab="library"
        onNavigate={onNavigate}
      />
    );

    expect(screen.getByText("12 项等待确认")).toBeInTheDocument();
    expect(screen.getByText("86 个整理文件可播放")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /人工确认/ }));
    expect(onNavigate).toHaveBeenCalledWith("reviews");
  });

  test("显示处理阶段进度并可直接打开日志", async () => {
    const user = userEvent.setup();
    jobs = [
      {
        id: "ingest-1",
        kind: "ingest",
        status: "running",
        phase: "processing",
        phaseProcessedItems: 8,
        phaseTotalItems: 20,
        totalItems: 20,
        processedItems: 8,
        successItems: 8,
        skippedItems: 0,
        failedItems: 0,
        createdAt: "2026-08-16T10:00:00Z",
        updatedAt: "2026-08-16T10:01:00Z",
      },
    ];
    render(
      <ManagementPipeline stats={stats} libraries={[]} activeTab="tasks" onNavigate={vi.fn()} />
    );

    expect(screen.getByText("匹配元数据并分析 · 8/20")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "查看信息完善日志" }));
    expect(openLogs).toHaveBeenCalledWith("ingest-1");
  });
});
