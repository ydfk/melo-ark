import { FileText } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { Job } from "@/lib/api/types";

import { useJobActivity } from "./job-activity-context";
import {
  jobCurrentStatusText,
  jobProgress,
  jobProgressText,
  jobStatusLabels,
} from "./job-presenter";

export function InlineJobStatus({
  job,
  label,
  className = "",
}: {
  job?: Job;
  label?: string;
  className?: string;
}) {
  const { openLogs } = useJobActivity();
  if (!job) return null;

  return (
    <div className={`rounded-xl border bg-muted/20 p-3 ${className}`}>
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          {label ? <p className="mb-2 text-sm font-medium">{label}</p> : null}
          <div className="flex items-center gap-2">
            <Badge variant={job.status === "failed" ? "destructive" : "secondary"}>
              {jobStatusLabels[job.status]}
            </Badge>
            <span className="font-mono text-xs text-muted-foreground">{jobProgressText(job)}</span>
          </div>
          <p className="mt-2 truncate text-xs text-muted-foreground">
            {job.currentItem ?? job.errorMessage ?? jobCurrentStatusText(job)}
          </p>
        </div>
        <Button type="button" variant="ghost" size="sm" onClick={() => openLogs(job.id)}>
          <FileText data-icon="inline-start" />
          日志
        </Button>
      </div>
      <Progress className="mt-3 h-1.5" value={jobProgress(job)} />
      <div className="mt-2 flex gap-4 text-xs text-muted-foreground">
        <span>成功 {job.successItems}</span>
        <span>跳过 {job.skippedItems}</span>
        <span className={job.failedItems ? "text-destructive" : undefined}>
          失败 {job.failedItems}
        </span>
      </div>
    </div>
  );
}
