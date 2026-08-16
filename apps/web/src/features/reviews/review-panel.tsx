import {
  CheckCheck,
  CopyCheck,
  FileQuestion,
  Filter,
  Layers3,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { ReviewPagination } from "@/features/reviews/review-pagination";
import { ReviewPreviewDialog } from "@/features/reviews/review-preview-dialog";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { ApiError } from "@/lib/api";
import {
  applyReviewBatch,
  clearReviewMarks,
  getReviews,
  previewReviewBatch,
  updateReview,
} from "@/lib/api/methods/reviews";
import type {
  ReviewBatchPreview,
  ReviewBatchRule,
  ReviewItem,
  ReviewKind,
  ReviewStatus,
} from "@/lib/api/types";
import { formatDate } from "@/lib/format";

const kindOptions: Array<{ value: "all" | ReviewKind; label: string }> = [
  { value: "all", label: "全部问题" },
  { value: "metadata_candidate", label: "元数据候选" },
  { value: "missing_artwork", label: "缺少封面" },
  { value: "missing_lyrics", label: "缺少歌词" },
  { value: "incomplete_tags", label: "标签不完整" },
  { value: "duplicate", label: "重复文件" },
  { value: "quality_variant", label: "质量版本" },
  { value: "organize_required", label: "需要整理" },
  { value: "hardlink_conflict", label: "硬链接冲突" },
  { value: "not_writable", label: "无法写入" },
  { value: "parse_failed", label: "解析失败" },
  { value: "job_failed", label: "任务失败" },
  { value: "source_missing", label: "来源不可用" },
];

const ruleOptions: Array<{ value: ReviewBatchRule; label: string }> = [
  { value: "high_confidence_metadata", label: "应用高置信元数据" },
  { value: "best_lyrics", label: "使用最佳歌词" },
  { value: "missing_artwork", label: "补充缺失封面" },
  { value: "reorganize", label: "按已确认信息重新整理" },
  { value: "recommended_duplicates", label: "处理推荐的重复版本" },
];

const DuplicatePanel = lazy(() =>
  import("@/features/duplicates/duplicate-panel").then((module) => ({
    default: module.DuplicatePanel,
  }))
);

export function ReviewPanel({ onChanged }: { onChanged: () => Promise<void> }) {
  const { latestJob, registerJob } = useJobActivity();
  const [items, setItems] = useState<ReviewItem[]>([]);
  const [total, setTotal] = useState(0);
  const [markedTotal, setMarkedTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [perPage, setPerPage] = useState(25);
  const [status, setStatus] = useState<ReviewStatus>("pending");
  const [kind, setKind] = useState<"all" | ReviewKind>("all");
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string>();
  const [rule, setRule] = useState<ReviewBatchRule>("high_confidence_metadata");
  const [preview, setPreview] = useState<ReviewBatchPreview>();
  const [applying, setApplying] = useState(false);
  const [duplicatesOpen, setDuplicatesOpen] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const result = await getReviews({
        status,
        kind: kind === "all" ? undefined : kind,
        page,
        perPage,
      }).send();
      const lastPage = Math.max(1, Math.ceil(result.total / result.perPage));
      if (page > lastPage) {
        setPage(lastPage);
        return;
      }
      setItems(result.items);
      setTotal(result.total);
      setMarkedTotal(result.markedTotal);
    } catch (error) {
      showError(error, "无法加载待处理项目");
    } finally {
      setLoading(false);
    }
  }, [kind, page, perPage, status]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function patch(item: ReviewItem, request: { marked?: boolean; status?: ReviewStatus }) {
    setBusyId(item.id);
    try {
      await updateReview(item.id, request).send();
      if (request.status) {
        await refresh();
        await onChanged();
      } else {
        setItems((current) =>
          current.map((entry) => (entry.id === item.id ? { ...entry, ...request } : entry))
        );
        if (request.marked !== undefined && request.marked !== item.marked) {
          setMarkedTotal((current) => Math.max(0, current + (request.marked ? 1 : -1)));
        }
      }
    } catch (error) {
      showError(error, "待处理状态更新失败");
    } finally {
      setBusyId(undefined);
    }
  }

  async function createPreview() {
    if (!markedTotal) return;
    try {
      setPreview(
        await previewReviewBatch({ status, kind: kind === "all" ? undefined : kind }, rule).send()
      );
    } catch (error) {
      showError(error, "无法生成批量处理预览");
    }
  }

  async function applyPreview() {
    if (!preview) return;
    setApplying(true);
    try {
      registerJob(await applyReviewBatch(preview.id).send());
      toast.success("批量处理任务已创建");
      setPreview(undefined);
      await refresh();
      await onChanged();
    } catch (error) {
      showError(error, "批量处理启动失败");
    } finally {
      setApplying(false);
    }
  }

  async function clearMarks() {
    try {
      await clearReviewMarks(status, kind === "all" ? undefined : kind).send();
      await refresh();
    } catch (error) {
      showError(error, "无法清除标记");
    }
  }

  return (
    <div className="space-y-5">
      <div className="flex flex-col justify-between gap-4 xl:flex-row xl:items-end">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.24em] text-primary">Review Queue</p>
          <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">待处理</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            系统负责发现问题，实际修改始终由你确认。
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Select
            value={status}
            onValueChange={(value) => {
              setStatus(value as ReviewStatus);
              setPage(1);
            }}
          >
            <SelectTrigger className="w-32" aria-label="处理状态">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="pending">待处理</SelectItem>
              <SelectItem value="resolved">已解决</SelectItem>
              <SelectItem value="ignored">已忽略</SelectItem>
            </SelectContent>
          </Select>
          <Select
            value={kind}
            onValueChange={(value) => {
              setKind(value as typeof kind);
              setPage(1);
            }}
          >
            <SelectTrigger className="w-44" aria-label="问题类型">
              <Filter />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {kindOptions.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="outline" size="icon" onClick={() => void refresh()} aria-label="刷新">
            <RefreshCw />
          </Button>
        </div>
      </div>

      {status === "pending" ? (
        <Card className="border-primary/20 bg-primary/5">
          <CardContent className="flex flex-col justify-between gap-3 py-4 lg:flex-row lg:items-center">
            <div className="flex items-center gap-3">
              <Layers3 className="size-5 text-primary" />
              <div>
                <p className="font-medium">当前筛选已标记 {markedTotal} 项</p>
                <p className="text-xs text-muted-foreground">
                  选择规则后先预览，不会直接修改文件。
                </p>
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              <Select value={rule} onValueChange={(value) => setRule(value as ReviewBatchRule)}>
                <SelectTrigger className="min-w-56" aria-label="批量规则">
                  <Sparkles />
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ruleOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {markedTotal ? (
                <Button variant="ghost" onClick={() => void clearMarks()}>
                  清除标记
                </Button>
              ) : null}
              <Button disabled={!markedTotal} onClick={() => void createPreview()}>
                生成处理预览
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      <InlineJobStatus job={latestJob("workspace", "reviews", "review_batch")} />

      {loading ? (
        <div className="flex justify-center py-20">
          <Spinner />
        </div>
      ) : (
        <div className="space-y-3">
          {items.map((item) => (
            <ReviewRow key={item.id} item={item} busy={busyId === item.id} onPatch={patch} />
          ))}
        </div>
      )}

      {!loading && !items.length ? (
        <Alert>
          <CheckCheck />
          <AlertTitle>当前没有项目</AlertTitle>
          <AlertDescription>新音乐扫描和分析后，需要判断的内容会集中到这里。</AlertDescription>
        </Alert>
      ) : null}
      {!loading && total ? (
        <ReviewPagination
          page={page}
          perPage={perPage}
          total={total}
          onPageChange={setPage}
          onPerPageChange={(value) => {
            setPerPage(value);
            setPage(1);
          }}
        />
      ) : null}

      {status === "pending" && ["all", "duplicate", "quality_variant"].includes(kind) ? (
        <details
          className="group rounded-2xl border bg-card/60 p-4"
          onToggle={(event) => setDuplicatesOpen(event.currentTarget.open)}
        >
          <summary className="flex cursor-pointer list-none items-center gap-2 font-medium">
            <CopyCheck className="size-4 text-primary" />
            重复与质量详情
            <span className="ml-auto text-xs text-muted-foreground group-open:hidden">展开</span>
          </summary>
          {duplicatesOpen ? (
            <div className="mt-6 border-t pt-6">
              <Suspense fallback={<Spinner />}>
                <DuplicatePanel onChanged={onChanged} />
              </Suspense>
            </div>
          ) : null}
        </details>
      ) : null}

      <ReviewPreviewDialog
        preview={preview}
        applying={applying}
        onClose={() => setPreview(undefined)}
        onApply={() => void applyPreview()}
      />
    </div>
  );
}

function ReviewRow({
  item,
  busy,
  onPatch,
}: {
  item: ReviewItem;
  busy: boolean;
  onPatch: (
    item: ReviewItem,
    request: { marked?: boolean; status?: ReviewStatus }
  ) => Promise<void>;
}) {
  const option = kindOptions.find((candidate) => candidate.value === item.kind);
  return (
    <article className="grid gap-3 rounded-2xl border bg-card/70 p-4 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-center">
      <Checkbox
        checked={item.marked}
        disabled={busy || item.status !== "pending"}
        onCheckedChange={(checked) => void onPatch(item, { marked: checked === true })}
        aria-label={`标记 ${item.title}`}
      />
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <FileQuestion className="size-4 text-primary" />
          <h2 className="truncate font-medium">{item.title}</h2>
          <Badge variant="secondary">{option?.label ?? item.kind}</Badge>
          {item.confidence !== undefined ? (
            <Badge variant="outline">置信度 {formatConfidence(item.confidence)}</Badge>
          ) : null}
        </div>
        <p className="mt-1 text-sm text-muted-foreground">{item.detail}</p>
        <p className="mt-2 text-xs text-muted-foreground">发现于 {formatDate(item.createdAt)}</p>
      </div>
      {item.status === "pending" ? (
        <div className="flex gap-2 sm:justify-end">
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            onClick={() => void onPatch(item, { status: "ignored" })}
          >
            忽略
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => void onPatch(item, { status: "resolved" })}
          >
            标为已解决
          </Button>
        </div>
      ) : (
        <Badge variant="outline">{item.status === "resolved" ? "已解决" : "已忽略"}</Badge>
      )}
    </article>
  );
}

function formatConfidence(value: number) {
  const percentage = value <= 1 ? value * 100 : value;
  return `${Math.round(percentage)}%`;
}

function showError(error: unknown, fallback: string) {
  toast.error(error instanceof ApiError ? error.problem.detail : fallback);
}
