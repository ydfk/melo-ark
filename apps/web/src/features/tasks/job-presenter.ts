import type { Job, JobStatus } from "@/lib/api/types";

export const jobStatusLabels: Record<JobStatus, string> = {
  queued: "排队中",
  running: "执行中",
  paused: "已暂停",
  cancel_requested: "正在取消",
  cancelled: "已取消",
  completed: "已完成",
  completed_with_errors: "部分失败",
  failed: "失败",
  interrupted: "待恢复",
};

export const jobKindLabels: Record<string, string> = {
  scan: "扫描目录",
  tag_edit: "写入标签",
  organize: "整理文件",
  ingest: "整理新增音乐",
  review_batch: "处理待处理项",
  trash: "移入回收站",
  scrape: "搜索元数据",
  analyze: "重复分析",
  lyrics: "写入歌词",
};

export const jobPhaseLabels: Record<string, string> = {
  discovering: "发现音乐文件",
  scanning: "读取文件并更新索引",
  linking: "创建整理硬链接",
  indexing: "更新整理目录索引",
  processing: "匹配元数据并分析",
};

export function jobTitle(job: Job) {
  if (job.kind === "scan") return job.targetPath ? "更新整理目录索引" : "扫描来源目录";
  return jobKindLabels[job.kind] ?? job.kind;
}

export function jobDescription(job: Job) {
  if (job.kind === "ingest" && job.sourcePath && job.targetPath)
    return `${job.sourcePath} → ${job.targetPath}`;
  if (job.kind === "scan") return job.targetPath ?? job.sourcePath ?? "更新音乐文件索引";
  const count = job.totalItems ? `${job.totalItems} 项` : "当前操作";
  const descriptions: Record<string, string> = {
    tag_edit: `向 ${count}写入标签`,
    organize: `整理 ${count}的文件路径`,
    review_batch: `处理 ${count}待确认项目`,
    trash: `将 ${count}移入回收站`,
    scrape: `为 ${count}搜索元数据候选`,
    analyze: `分析 ${count}的重复与质量信息`,
    lyrics: `为 ${count}写入歌词`,
  };
  return descriptions[job.kind];
}

export function jobProgress(job: Job) {
  const total = job.phaseTotalItems ?? job.totalItems;
  const processed = job.phase
    ? (job.phaseProcessedItems ?? job.processedItems)
    : job.processedItems;
  return total ? Math.min(100, (processed / total) * 100) : 0;
}

export function jobProgressText(job: Job) {
  const label = job.phase ? jobPhaseLabels[job.phase] : undefined;
  const processed = job.phase
    ? (job.phaseProcessedItems ?? job.processedItems)
    : job.processedItems;
  const total = job.phaseTotalItems ?? (job.phase ? undefined : job.totalItems);
  const counts = total === undefined ? `已处理 ${processed}` : `${processed}/${total}`;
  return label ? `${label} · ${counts}` : counts;
}

export function jobCurrentStatusText(job: Job) {
  if (job.status === "completed" || job.status === "completed_with_errors") return "处理完成";
  if (job.status === "failed") return "任务失败";
  if (job.status === "cancelled") return "已取消";
  if (job.status === "paused") return "已暂停";
  if (job.status === "queued" || job.status === "interrupted") return "等待执行";
  return job.phase ? (jobPhaseLabels[job.phase] ?? "正在处理") : "正在处理";
}
