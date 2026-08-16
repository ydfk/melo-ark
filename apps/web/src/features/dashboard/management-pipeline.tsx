import {
  ArrowRight,
  CircleAlert,
  FileCheck2,
  FolderInput,
  ListChecks,
  ScanSearch,
  ScrollText,
  Sparkles,
  WandSparkles,
} from "lucide-react";

import type { DashboardTab } from "@/components/dashboard-command-palette";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { jobProgressText } from "@/features/tasks/job-presenter";
import type { DashboardStats, Job, LibraryGroup } from "@/lib/api/types";

type StageTone = "idle" | "running" | "attention" | "error" | "ready";

type PipelineStage = {
  id: string;
  label: string;
  detail: string;
  tone: StageTone;
  tab: DashboardTab;
  icon: typeof FolderInput;
  job?: Job;
};

const activeStatuses = new Set(["queued", "running", "paused", "cancel_requested", "interrupted"]);

export function ManagementPipeline({
  stats,
  libraries,
  activeTab,
  onNavigate,
}: {
  stats?: DashboardStats;
  libraries: LibraryGroup[];
  activeTab: DashboardTab;
  onNavigate: (tab: DashboardTab) => void;
}) {
  const { jobs, openLogs } = useJobActivity();
  const stages = buildStages(stats, libraries, jobs);

  return (
    <section className="mt-5 overflow-hidden rounded-2xl border bg-card/65 shadow-sm backdrop-blur-md">
      <div className="flex items-center justify-between gap-3 border-b px-4 py-3">
        <div>
          <p className="text-sm font-semibold">音乐处理轴线</p>
          <p className="text-xs text-muted-foreground">从来源目录到可播放文件的完整状态</p>
        </div>
        <Badge variant="outline" className="hidden font-mono text-[10px] tracking-wider sm:flex">
          SOURCE → MANAGED
        </Badge>
      </div>
      <div className="overflow-x-auto [scrollbar-width:thin]">
        <div className="grid min-w-[1040px] grid-cols-[repeat(6,minmax(0,1fr))]">
          {stages.map((stage, index) => (
            <div
              key={stage.id}
              className="group relative border-r last:border-r-0"
              data-active={activeTab === stage.tab || undefined}
            >
              <button
                type="button"
                className="flex h-full min-h-32 w-full flex-col items-start gap-3 px-4 py-4 text-left transition-colors hover:bg-muted/50 group-data-[active]:bg-primary/5"
                onClick={() => onNavigate(stage.tab)}
              >
                <div className="flex w-full items-center justify-between gap-2">
                  <span className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
                    <span className="font-mono text-[10px]">
                      {String(index + 1).padStart(2, "0")}
                    </span>
                    <stage.icon className="size-4 text-primary" />
                  </span>
                  <StageBadge tone={stage.tone} />
                </div>
                <div className="min-w-0">
                  <p className="font-medium">{stage.label}</p>
                  <p className="mt-1 line-clamp-2 text-xs leading-5 text-muted-foreground">
                    {stage.detail}
                  </p>
                </div>
              </button>
              {stage.job ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="absolute bottom-2 right-2 size-7 opacity-70 hover:opacity-100"
                  onClick={() => openLogs(stage.job!.id)}
                  aria-label={`查看${stage.label}日志`}
                >
                  <ScrollText />
                </Button>
              ) : null}
              {index < stages.length - 1 ? (
                <ArrowRight className="pointer-events-none absolute -right-2.5 top-1/2 z-10 size-5 -translate-y-1/2 rounded-full border bg-background p-0.5 text-muted-foreground" />
              ) : null}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function buildStages(
  stats: DashboardStats | undefined,
  libraries: LibraryGroup[],
  jobs: Job[]
): PipelineStage[] {
  const sources = libraries.flatMap((library) => library.sources);
  const unconfigured = libraries
    .filter((library) => library.status === "needsTarget")
    .reduce((count, library) => count + library.sources.length, 0);
  const scans = jobs.filter((job) => job.kind === "scan" && !job.internal && !job.targetPath);
  const ingests = jobs.filter((job) => job.kind === "ingest" && !job.internal);
  const enrichment = jobs.filter(
    (job) =>
      (!job.internal && ["scrape", "analyze", "lyrics", "tag_edit"].includes(job.kind)) ||
      (job.kind === "ingest" && job.phase === "processing")
  );
  const reviewJobs = jobs.filter((job) => job.kind === "review_batch" && !job.internal);
  const scanState = summarizeJobs(scans);
  const ingestState = summarizeJobs(ingests.filter((job) => job.phase !== "processing"));
  const enrichmentState = summarizeJobs(enrichment);
  const reviewState = summarizeJobs(reviewJobs);
  const pendingReviews = stats?.pendingReviewCount ?? 0;
  const managedFiles = stats?.availableManagedFileCount ?? 0;

  return [
    {
      id: "intake",
      label: "曲库接入",
      detail: !sources.length
        ? "尚未添加来源目录"
        : unconfigured
          ? `${sources.length} 个来源 · ${unconfigured} 个待配置`
          : `${sources.length} 个来源已连接整理目录`,
      tone: !sources.length || unconfigured ? "attention" : "ready",
      tab: "library",
      icon: FolderInput,
    },
    {
      id: "scan",
      label: "扫描新增",
      detail: jobDetail(
        scanState,
        sources.length ? `${sources.length} 个来源可扫描` : "等待曲库接入"
      ),
      tone: jobTone(scanState, sources.length > 0),
      tab: "library",
      icon: ScanSearch,
      job: scanState.latest,
    },
    {
      id: "organize",
      label: "硬链接与索引",
      detail: jobDetail(ingestState, "等待扫描发现新增音乐"),
      tone: jobTone(ingestState, managedFiles > 0),
      tab: "tasks",
      icon: WandSparkles,
      job: ingestState.latest,
    },
    {
      id: "enrich",
      label: "信息完善",
      detail: jobDetail(
        enrichmentState,
        managedFiles ? "封面、歌词、标签与质量分析" : "等待整理文件"
      ),
      tone: jobTone(enrichmentState, managedFiles > 0),
      tab: "tasks",
      icon: Sparkles,
      job: enrichmentState.latest,
    },
    {
      id: "review",
      label: "人工确认",
      detail: reviewState.active.length
        ? jobDetail(reviewState, "正在处理已确认项目")
        : pendingReviews
          ? `${pendingReviews} 项等待确认`
          : "当前没有待处理项目",
      tone: reviewState.failed
        ? "error"
        : reviewState.active.length
          ? "running"
          : pendingReviews
            ? "attention"
            : "ready",
      tab: "reviews",
      icon: ListChecks,
      job: reviewState.latest,
    },
    {
      id: "ready",
      label: "完成入库",
      detail: managedFiles ? `${managedFiles} 个整理文件可播放` : "尚无可播放的整理文件",
      tone: managedFiles ? "ready" : "idle",
      tab: "songs",
      icon: FileCheck2,
    },
  ];
}

function summarizeJobs(jobs: Job[]) {
  const sorted = [...jobs].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  const active = sorted.filter((job) => activeStatuses.has(job.status));
  const latest = active[0] ?? sorted[0];
  return {
    active,
    latest,
    failed: !active.length && latest?.status === "failed",
  };
}

function jobDetail(state: ReturnType<typeof summarizeJobs>, fallback: string) {
  if (state.active.length > 1)
    return `${state.active.length} 个任务进行中 · ${jobProgressText(state.active[0])}`;
  if (state.latest && (state.active.length || state.failed)) return jobProgressText(state.latest);
  if (state.latest?.status === "completed" || state.latest?.status === "completed_with_errors")
    return `最近完成 ${state.latest.successItems} 项`;
  return fallback;
}

function jobTone(state: ReturnType<typeof summarizeJobs>, ready: boolean): StageTone {
  if (state.failed) return "error";
  if (state.active.length) return "running";
  return ready ? "ready" : "idle";
}

function StageBadge({ tone }: { tone: StageTone }) {
  const labels: Record<StageTone, string> = {
    idle: "待执行",
    running: "运行中",
    attention: "需处理",
    error: "异常",
    ready: "已就绪",
  };
  const styles: Record<StageTone, string> = {
    idle: "border-border text-muted-foreground",
    running: "border-primary/30 bg-primary/10 text-primary",
    attention: "border-amber-500/30 bg-amber-500/10 text-amber-500",
    error: "border-destructive/30 bg-destructive/10 text-destructive",
    ready: "border-emerald-500/30 bg-emerald-500/10 text-emerald-500",
  };
  return (
    <Badge variant="outline" className={styles[tone]}>
      {tone === "error" ? <CircleAlert /> : null}
      {labels[tone]}
    </Badge>
  );
}
