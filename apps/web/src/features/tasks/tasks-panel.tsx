import {
  ChevronDown,
  ChevronRight,
  CirclePause,
  CirclePlay,
  FileText,
  RefreshCw,
  Square,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { ApiError } from "@/lib/api";
import { cancelJob, pauseJob, resumeJob, retryFailedJob } from "@/lib/api/methods/jobs";
import type { Job, JobStatus } from "@/lib/api/types";
import { formatDate } from "@/lib/format";
import { useJobActivity } from "./job-activity-context";
import { jobKindLabels, jobProgress, jobStatusLabels } from "./job-presenter";

type TasksPanelProps = {
  jobs?: Job[];
  onChanged: () => Promise<void>;
};

type TaskGroup = {
  key: string;
  jobs: Job[];
  summary: Job;
};

export function TasksPanel({ jobs: providedJobs, onChanged }: TasksPanelProps) {
  const { jobs: activityJobs, openLogs, registerJob } = useJobActivity();
  const jobs = providedJobs ?? activityJobs;
  const groups = useMemo(() => groupJobs(jobs), [jobs]);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  async function run(action: () => { send: () => Promise<Job> }, success: string) {
    try {
      registerJob(await action().send());
      toast.success(success);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "任务操作失败");
    }
  }

  function toggleGroup(key: string) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  return (
    <div className="flex flex-col gap-6">
      <div>
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-primary">Persistent Jobs</p>
        <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">任务中心</h1>
      </div>

      {groups.length ? (
        <section className="overflow-hidden rounded-2xl border bg-card/70">
          <div className="hidden border-b bg-muted/30 px-4 py-2 text-xs font-medium text-muted-foreground lg:grid lg:grid-cols-[minmax(180px,1fr)_110px_minmax(220px,1.2fr)_minmax(140px,0.8fr)_auto] lg:gap-4">
            <span>任务</span>
            <span>状态</span>
            <span>进度</span>
            <span>当前项目</span>
            <span className="text-right">操作</span>
          </div>
          {groups.map((group) => {
            const isLegacyGroup = group.jobs.length > 1;
            const isExpanded = expanded.has(group.key);
            return (
              <div key={group.key} className="border-b last:border-b-0">
                <TaskRow
                  job={group.summary}
                  label={
                    isLegacyGroup ? `历史新增音乐接入 · ${group.jobs.length} 个任务` : undefined
                  }
                  leading={
                    isLegacyGroup ? (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-8"
                        aria-label={isExpanded ? "收起历史任务" : "展开历史任务"}
                        onClick={() => toggleGroup(group.key)}
                      >
                        {isExpanded ? <ChevronDown /> : <ChevronRight />}
                      </Button>
                    ) : null
                  }
                  actions={
                    isLegacyGroup ? null : (
                      <TaskActions job={group.summary} openLogs={openLogs} run={run} />
                    )
                  }
                />
                {isLegacyGroup && isExpanded ? (
                  <div className="border-t bg-muted/15 pl-4 sm:pl-10">
                    {group.jobs.map((job) => (
                      <TaskRow
                        key={job.id}
                        job={job}
                        compact
                        actions={<TaskActions job={job} openLogs={openLogs} run={run} />}
                      />
                    ))}
                  </div>
                ) : null}
              </div>
            );
          })}
        </section>
      ) : (
        <Alert>
          <RefreshCw />
          <AlertTitle>还没有任务</AlertTitle>
          <AlertDescription>扫描曲库后，任务进度会显示在这里。</AlertDescription>
        </Alert>
      )}
    </div>
  );
}

function TaskRow({
  job,
  label,
  leading,
  actions,
  compact = false,
}: {
  job: Job;
  label?: string;
  leading?: ReactNode;
  actions: ReactNode;
  compact?: boolean;
}) {
  const progress = jobProgress(job);
  return (
    <div
      className={`grid gap-3 px-4 ${compact ? "py-3" : "py-4"} lg:grid-cols-[minmax(180px,1fr)_110px_minmax(220px,1.2fr)_minmax(140px,0.8fr)_auto] lg:items-center lg:gap-4`}
    >
      <div className="flex min-w-0 items-center gap-2">
        {leading}
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">
            {label ?? jobKindLabels[job.kind] ?? job.kind}
          </p>
          <p className="mt-0.5 text-xs text-muted-foreground">{formatDate(job.createdAt)}</p>
        </div>
      </div>
      <div>
        <Badge variant={job.status === "failed" ? "destructive" : "secondary"}>
          {jobStatusLabels[job.status]}
        </Badge>
      </div>
      <div className="min-w-0 space-y-1.5">
        <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
          <span>
            {job.processedItems}/{job.totalItems}
          </span>
          <span>
            成功 {job.successItems} · 跳过 {job.skippedItems} · 失败 {job.failedItems}
          </span>
        </div>
        <Progress value={progress} className="h-1.5" />
      </div>
      <p className="truncate font-mono text-xs text-muted-foreground">
        {job.currentItem ?? job.errorMessage ?? "—"}
      </p>
      <div className="flex flex-wrap justify-start gap-1 lg:justify-end">{actions}</div>
    </div>
  );
}

function TaskActions({
  job,
  openLogs,
  run,
}: {
  job: Job;
  openLogs: (id: string) => void;
  run: (action: () => { send: () => Promise<Job> }, success: string) => Promise<void>;
}) {
  return (
    <>
      <Button variant="ghost" size="sm" aria-label="查看日志" onClick={() => openLogs(job.id)}>
        <FileText />
        <span className="hidden 2xl:inline">日志</span>
      </Button>
      {job.status === "running" || job.status === "queued" ? (
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          aria-label="暂停任务"
          onClick={() => void run(() => pauseJob(job.id), "任务已暂停")}
        >
          <CirclePause />
        </Button>
      ) : null}
      {job.status === "paused" || job.status === "interrupted" ? (
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          aria-label="恢复任务"
          onClick={() => void run(() => resumeJob(job.id), "任务已恢复")}
        >
          <CirclePlay />
        </Button>
      ) : null}
      {(["queued", "running", "paused", "interrupted"] as JobStatus[]).includes(job.status) ? (
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          aria-label="取消任务"
          onClick={() => void run(() => cancelJob(job.id), "已请求取消任务")}
        >
          <Square />
        </Button>
      ) : null}
      {job.status === "completed_with_errors" || job.status === "failed" ? (
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          aria-label="重试失败项"
          onClick={() => void run(() => retryFailedJob(job.id), "失败项已重新入队")}
        >
          <RefreshCw />
        </Button>
      ) : null}
    </>
  );
}

function groupJobs(jobs: Job[]): TaskGroup[] {
  const groups = new Map<string, Job[]>();
  const order: string[] = [];
  for (const job of jobs) {
    const key = job.kind === "ingest" && job.parentJobId ? `ingest:${job.parentJobId}` : job.id;
    if (!groups.has(key)) order.push(key);
    groups.set(key, [...(groups.get(key) ?? []), job]);
  }
  return order.map((key) => {
    const entries = groups.get(key) ?? [];
    return { key, jobs: entries, summary: summarizeJobs(entries) };
  });
}

function summarizeJobs(jobs: Job[]): Job {
  if (jobs.length === 1) return jobs[0];
  const first = jobs[0];
  return {
    ...first,
    status: summarizeStatus(jobs.map((job) => job.status)),
    totalItems: sum(jobs, "totalItems"),
    processedItems: sum(jobs, "processedItems"),
    successItems: sum(jobs, "successItems"),
    skippedItems: sum(jobs, "skippedItems"),
    failedItems: sum(jobs, "failedItems"),
    currentItem: jobs.find((job) => job.currentItem)?.currentItem,
    errorMessage: jobs.find((job) => job.errorMessage)?.errorMessage,
  };
}

function summarizeStatus(statuses: JobStatus[]): JobStatus {
  const priority: JobStatus[] = [
    "running",
    "cancel_requested",
    "paused",
    "queued",
    "interrupted",
    "failed",
    "completed_with_errors",
    "cancelled",
    "completed",
  ];
  return priority.find((status) => statuses.includes(status)) ?? "completed";
}

function sum(
  jobs: Job[],
  key: "totalItems" | "processedItems" | "successItems" | "skippedItems" | "failedItems"
) {
  return jobs.reduce((total, job) => total + job[key], 0);
}
