import { AlertCircle, CircleCheck, Info, LoaderCircle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { getJobLogs } from "@/lib/api/methods/jobs";
import type { Job, JobLog } from "@/lib/api/types";
import { formatDate } from "@/lib/format";

import { jobDescription, jobProgressText, jobStatusLabels, jobTitle } from "./job-presenter";

type LogLevel = "all" | JobLog["level"];

export function JobLogSheet({
  job,
  liveLogs,
  open,
  onOpenChange,
}: {
  job?: Job;
  liveLogs: JobLog[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [logs, setLogs] = useState<JobLog[]>([]);
  const [nextBefore, setNextBefore] = useState<number>();
  const [level, setLevel] = useState<LogLevel>("all");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!job || !open) return;
    setLoading(true);
    getJobLogs(job.id, undefined, level === "all" ? undefined : level)
      .send()
      .then((page) => {
        setLogs([...page.items].reverse());
        setNextBefore(page.nextBefore);
      })
      .finally(() => setLoading(false));
  }, [job?.id, level, open]);

  const visibleLiveLogs = useMemo(
    () => liveLogs.filter((item) => level === "all" || item.level === level),
    [level, liveLogs]
  );
  const mergedLogs = useMemo(() => {
    const byId = new Map([...logs, ...visibleLiveLogs].map((item) => [item.id, item]));
    return [...byId.values()].sort((left, right) => left.id - right.id);
  }, [logs, visibleLiveLogs]);

  async function loadOlder() {
    if (!job || !nextBefore) return;
    setLoading(true);
    try {
      const page = await getJobLogs(job.id, nextBefore, level === "all" ? undefined : level).send();
      setLogs((current) => [...page.items.reverse(), ...current]);
      setNextBefore(page.nextBefore);
    } finally {
      setLoading(false);
    }
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="flex w-[94vw] flex-col sm:max-w-2xl">
        <SheetHeader className="pr-8">
          <div className="flex flex-wrap items-center gap-2">
            <SheetTitle>{job ? jobTitle(job) : "任务日志"}</SheetTitle>
            {job ? <Badge variant="secondary">{jobStatusLabels[job.status]}</Badge> : null}
          </div>
          <SheetDescription>
            {job
              ? [jobDescription(job), jobProgressText(job), formatDate(job.createdAt)]
                  .filter(Boolean)
                  .join(" · ")
              : ""}
          </SheetDescription>
        </SheetHeader>
        <div className="flex items-center justify-between gap-3 border-y py-3">
          <span className="text-sm text-muted-foreground">{mergedLogs.length} 条日志</span>
          <Select value={level} onValueChange={(value) => setLevel(value as LogLevel)}>
            <SelectTrigger className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部级别</SelectItem>
              <SelectItem value="info">信息</SelectItem>
              <SelectItem value="warn">警告</SelectItem>
              <SelectItem value="error">错误</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <ScrollArea className="min-h-0 flex-1 pr-4">
          <div className="space-y-1 py-4">
            {nextBefore ? (
              <Button
                className="mb-3 w-full"
                variant="ghost"
                size="sm"
                disabled={loading}
                onClick={() => void loadOlder()}
              >
                {loading ? <LoaderCircle className="animate-spin" /> : null}
                加载更早日志
              </Button>
            ) : null}
            {mergedLogs.map((log) => (
              <LogRow key={log.id} log={log} />
            ))}
            {!loading && !mergedLogs.length ? (
              <p className="py-16 text-center text-sm text-muted-foreground">暂无日志</p>
            ) : null}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  );
}

function LogRow({ log }: { log: JobLog }) {
  const Icon = log.level === "error" ? AlertCircle : log.level === "warn" ? Info : CircleCheck;
  return (
    <div className="grid grid-cols-[18px_minmax(0,1fr)] gap-3 rounded-lg px-2 py-2.5 hover:bg-muted/40">
      <Icon
        className={
          log.level === "error" ? "mt-0.5 size-4 text-destructive" : "mt-0.5 size-4 text-primary"
        }
      />
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
          <span>{formatDate(log.createdAt)}</span>
          <span>{jobLogEventLabels[log.eventType] ?? log.eventType}</span>
          {log.attempt ? <span>第 {log.attempt} 次</span> : null}
        </div>
        <p className="mt-1 text-sm">{log.message}</p>
        {log.itemKey ? (
          <p className="mt-1 break-all font-mono text-xs text-muted-foreground">{log.itemKey}</p>
        ) : null}
      </div>
    </div>
  );
}

const jobLogEventLabels: Record<string, string> = {
  queued: "已排队",
  started: "已开始",
  item_started: "开始处理文件",
  hardlink_created: "已创建硬链接",
  hardlink_exists: "硬链接已存在",
  index_started: "开始更新整理索引",
  index_completed: "整理索引已更新",
  success: "处理成功",
  skipped: "已跳过",
  failed: "处理失败",
  paused: "已暂停",
  resumed: "已恢复",
  cancelled: "已取消",
  retry: "正在重试",
  completed: "已完成",
  completed_with_errors: "完成但有失败项",
};
