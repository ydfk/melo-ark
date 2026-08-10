import { CirclePause, CirclePlay, RefreshCw, Square } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldLabel } from "@/components/ui/field";
import { Progress } from "@/components/ui/progress";
import { ApiError } from "@/lib/api";
import { cancelJob, pauseJob, resumeJob, retryFailedJob } from "@/lib/api/methods/jobs";
import type { Job, JobStatus } from "@/lib/api/types";
import { formatDate } from "@/lib/format";

type TasksPanelProps = {
  jobs: Job[];
  onChanged: () => Promise<void>;
};

const statusLabels: Record<JobStatus, string> = {
  queued: "排队中",
  running: "执行中",
  paused: "已暂停",
  cancel_requested: "正在取消",
  cancelled: "已取消",
  completed: "已完成",
  completed_with_errors: "部分失败",
  failed: "失败",
  interrupted: "启动后待恢复",
};

const kindLabels: Record<string, string> = {
  scan: "曲库扫描",
  tag_edit: "Tag 写入",
  organize: "Hardlink 整理",
  trash: "移入回收站",
  scrape: "元数据刮削",
  analyze: "Hash 与指纹分析",
  lyrics: "歌词写入",
};

export function TasksPanel({ jobs, onChanged }: TasksPanelProps) {
  async function run(action: () => { send: () => Promise<Job> }, success: string) {
    try {
      await action().send();
      toast.success(success);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "任务操作失败");
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <div>
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-primary">Persistent Jobs</p>
        <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">任务中心</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          状态和逐文件结果保存在 SQLite；容器重启后，运行中任务会变为待恢复，不会重复已完成项。
        </p>
      </div>

      {jobs.length ? (
        <section className="grid gap-4 xl:grid-cols-2">
          {jobs.map((job) => {
            const progress = job.totalItems
              ? Math.min(100, (job.processedItems / job.totalItems) * 100)
              : 0;
            return (
              <Card key={job.id}>
                <CardHeader>
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <CardTitle>{kindLabels[job.kind] ?? job.kind}</CardTitle>
                      <CardDescription>{formatDate(job.createdAt)}</CardDescription>
                    </div>
                    <Badge variant={job.status === "failed" ? "destructive" : "secondary"}>
                      {statusLabels[job.status]}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent className="flex flex-col gap-4">
                  <Field>
                    <FieldLabel className="justify-between">
                      <span>处理进度</span>
                      <span className="font-mono text-xs text-muted-foreground">
                        {job.processedItems}/{job.totalItems}
                      </span>
                    </FieldLabel>
                    <Progress value={progress} />
                  </Field>
                  <div className="grid grid-cols-3 gap-3 text-center text-xs">
                    <TaskCount label="成功" value={job.successItems} />
                    <TaskCount label="跳过" value={job.skippedItems} />
                    <TaskCount label="失败" value={job.failedItems} />
                  </div>
                  <div className="grid grid-cols-2 gap-3 text-xs">
                    <TaskMetric
                      label="处理速度"
                      value={
                        job.itemsPerSecond == null
                          ? "等待采样"
                          : `${formatRate(job.itemsPerSecond)} 项/秒`
                      }
                    />
                    <TaskMetric label="预计剩余" value={formatEta(job.etaSeconds, job.status)} />
                  </div>
                  <p className="truncate font-mono text-xs text-muted-foreground">
                    {job.currentItem ?? job.errorMessage ?? "没有正在处理的文件"}
                  </p>
                </CardContent>
                <CardFooter className="gap-2">
                  {job.status === "running" || job.status === "queued" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => void run(() => pauseJob(job.id), "任务已暂停")}
                    >
                      <CirclePause data-icon="inline-start" />
                      暂停
                    </Button>
                  ) : null}
                  {job.status === "paused" || job.status === "interrupted" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => void run(() => resumeJob(job.id), "任务已恢复")}
                    >
                      <CirclePlay data-icon="inline-start" />
                      恢复
                    </Button>
                  ) : null}
                  {["queued", "running", "paused", "interrupted"].includes(job.status) ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => void run(() => cancelJob(job.id), "已请求取消任务")}
                    >
                      <Square data-icon="inline-start" />
                      取消
                    </Button>
                  ) : null}
                  {job.status === "completed_with_errors" || job.status === "failed" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => void run(() => retryFailedJob(job.id), "失败项已重新入队")}
                    >
                      <RefreshCw data-icon="inline-start" />
                      重试失败项
                    </Button>
                  ) : null}
                </CardFooter>
              </Card>
            );
          })}
        </section>
      ) : (
        <Alert>
          <RefreshCw />
          <AlertTitle>还没有任务</AlertTitle>
          <AlertDescription>从曲库页发起首次扫描后，进度与逐项结果会显示在这里。</AlertDescription>
        </Alert>
      )}
    </div>
  );
}

function TaskCount({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border bg-muted/30 px-3 py-2">
      <p className="font-mono text-lg text-foreground">{value}</p>
      <p className="text-muted-foreground">{label}</p>
    </div>
  );
}

function TaskMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between rounded-lg border bg-muted/20 px-3 py-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono text-foreground">{value}</span>
    </div>
  );
}

function formatRate(rate: number) {
  return rate >= 10 ? rate.toFixed(1) : rate.toFixed(2);
}

function formatEta(seconds: number | null | undefined, status: JobStatus) {
  if (status === "completed" || status === "completed_with_errors") return "已完成";
  if (status !== "running" || seconds == null) return "—";
  if (seconds < 60) return `${seconds} 秒`;
  if (seconds < 3_600) return `${Math.ceil(seconds / 60)} 分钟`;
  return `${Math.ceil(seconds / 3_600)} 小时`;
}
