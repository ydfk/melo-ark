import type { JobStatus } from "@/lib/api/types";

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
  scan: "曲库扫描",
  tag_edit: "标签写入",
  organize: "文件整理",
  ingest: "新增音乐接入",
  review_batch: "批量处理",
  trash: "移入回收站",
  scrape: "元数据匹配",
  analyze: "重复文件分析",
  lyrics: "歌词写入",
};

export function jobProgress(job: { processedItems: number; totalItems: number }) {
  return job.totalItems ? Math.min(100, (job.processedItems / job.totalItems) * 100) : 0;
}
