import { Bot, DatabaseZap, Fingerprint, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { OperationPreview } from "@/features/library/operation-preview";
import { CoverArtwork } from "@/features/library/cover-artwork";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { ApiError } from "@/lib/api";
import {
  analyzeDuplicates,
  applyTrash,
  explainDuplicate,
  getAiStatus,
  getDuplicateGroups,
  previewTrash,
} from "@/lib/api/methods/library";
import type { AiStatus, DuplicateGroup, Operation } from "@/lib/api/types";
import { formatBytes, formatDuration } from "@/lib/format";

const kinds: Array<{ value: "all" | DuplicateGroup["kind"]; label: string }> = [
  { value: "all", label: "全部" },
  { value: "hardlink_alias", label: "硬链接别名" },
  { value: "binary_exact", label: "Binary Exact" },
  { value: "audio_duplicate", label: "Audio Duplicate" },
  { value: "quality_variant", label: "Quality Variant" },
  { value: "possible_duplicate", label: "Possible" },
];

export function DuplicatePanel({ onChanged }: { onChanged: () => Promise<void> }) {
  const { latestJob, registerJob } = useJobActivity();
  const [kind, setKind] = useState<(typeof kinds)[number]["value"]>("all");
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [operation, setOperation] = useState<Operation>();
  const [busy, setBusy] = useState(false);
  const [aiStatus, setAiStatus] = useState<AiStatus>();
  const refresh = async (nextKind = kind, nextPage = page) => {
    const result = await getDuplicateGroups(
      nextKind === "all" ? undefined : nextKind,
      nextPage
    ).send();
    setGroups(result.items);
    setTotal(result.total);
    setPage(result.page);
  };
  useEffect(() => {
    void refresh();
    void getAiStatus().send().then(setAiStatus);
  }, []);

  async function analyze() {
    setBusy(true);
    try {
      registerJob(await analyzeDuplicates().send());
      toast.success("已创建 Hash + Fingerprint 分析任务");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }
  async function prepareTrash() {
    if (!selected.size) return;
    setBusy(true);
    try {
      setOperation(await previewTrash([...selected]).send());
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }
  async function confirmTrash() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await applyTrash(operation.id).send());
      setSelected(new Set());
      toast.success("选中文件已移入对应曲库的回收站");
      await onChanged();
      await refresh();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }
  async function askAi(group: DuplicateGroup) {
    try {
      const result = await explainDuplicate(group.id).send();
      toast.info(`${result.relation} · ${Math.round(result.confidence * 100)}%：${result.reason}`, {
        duration: 10_000,
      });
    } catch (error) {
      showError(error);
    }
  }

  return (
    <div className="space-y-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.2em] text-primary">Duplicate Lab</p>
          <h2 className="mt-2 font-display text-3xl font-semibold">重复与质量分析</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            每个重复概念独立展示；Quality Score 是技术规格分，不代表听感。
          </p>
        </div>
        <Button onClick={() => void analyze()} disabled={busy}>
          <Fingerprint data-icon="inline-start" />
          分析全部物理文件
        </Button>
      </div>
      <InlineJobStatus job={latestJob("workspace", "duplicates", "analyze")} />
      <Alert>
        <ShieldCheck />
        <AlertTitle>分析永远不会删除文件</AlertTitle>
        <AlertDescription>
          硬链接别名不计为空间浪费。清理前会显示预览 与显式确认；推荐保留项仅供参考。
        </AlertDescription>
      </Alert>
      <Tabs
        value={kind}
        onValueChange={(value) => {
          const next = value as typeof kind;
          setKind(next);
          setPage(1);
          setSelected(new Set());
          void refresh(next, 1);
        }}
      >
        <TabsList className="h-auto w-full justify-start overflow-x-auto">
          {kinds.map((item) => (
            <TabsTrigger key={item.value} value={item.value}>
              {item.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
      {selected.size ? (
        <div className="flex items-center justify-between rounded-xl border bg-destructive/5 px-4 py-3">
          <span className="text-sm">已选择 {selected.size} 个物理文件</span>
          <Button variant="destructive" size="sm" onClick={() => void prepareTrash()}>
            <Trash2 data-icon="inline-start" />
            生成 Trash Preview
          </Button>
        </div>
      ) : null}
      <div className="space-y-4">
        {groups.map((group) => (
          <Card key={group.id} className="overflow-hidden bg-card/70">
            <CardHeader>
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <CardTitle className="flex flex-wrap items-center gap-2">
                    <DatabaseZap className="size-4" />
                    {labelKind(group.kind)}
                    <Badge variant="secondary">置信度 {group.confidence}%</Badge>
                    {group.reclaimableBytes ? (
                      <Badge variant="destructive">
                        可回收约 {formatBytes(group.reclaimableBytes)}
                      </Badge>
                    ) : null}
                  </CardTitle>
                  <CardDescription className="mt-2">{group.reason}</CardDescription>
                </div>
                {aiStatus?.enabled && group.kind === "possible_duplicate" ? (
                  <Button variant="outline" size="sm" onClick={() => void askAi(group)}>
                    <Bot data-icon="inline-start" />
                    AI 解释
                  </Button>
                ) : null}
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              {group.members.map((member) => (
                <div
                  key={member.mediaFileId}
                  className={`grid gap-3 rounded-xl border p-3 sm:grid-cols-[auto_3.5rem_1fr_8rem] ${member.recommendedKeep ? "border-emerald-500/30 bg-emerald-500/5" : ""}`}
                >
                  <Checkbox
                    aria-label={`选择 ${member.path}`}
                    checked={selected.has(member.mediaFileId)}
                    disabled={group.kind === "hardlink_alias" || member.recommendedKeep}
                    onCheckedChange={(value) =>
                      setSelected((current) => {
                        const next = new Set(current);
                        if (value) next.add(member.mediaFileId);
                        else next.delete(member.mediaFileId);
                        return next;
                      })
                    }
                  />
                  <CoverArtwork
                    mediaId={member.mediaFileId}
                    hasArtwork={member.hasArtwork}
                    alt={`${member.title} 封面`}
                    className="size-14 rounded-lg"
                  />
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="truncate font-medium">{member.title}</p>
                      {member.versionLabel ? (
                        <Badge variant="secondary">{member.versionLabel}</Badge>
                      ) : null}
                      {member.recommendedKeep ? <Badge>建议保留</Badge> : null}
                      <Badge variant="outline">{member.extension.toUpperCase()}</Badge>
                    </div>
                    <p className="mt-1 truncate text-xs text-muted-foreground">
                      {member.artist} · {member.path}
                    </p>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {formatDuration(member.durationMs)} · {formatBytes(member.fileSize)} ·{" "}
                      {(member.codec ?? member.extension).toUpperCase()} ·{" "}
                      {member.bitrate ? `${Math.round(member.bitrate / 1000)} kbps · ` : ""}
                      {member.bitDepth ?? "?"} bit /{" "}
                      {member.sampleRate ? `${member.sampleRate / 1000} kHz` : "?"}
                    </p>
                  </div>
                  <div>
                    <div className="flex justify-between text-xs">
                      <span>Quality</span>
                      <strong>{member.qualityScore}</strong>
                    </div>
                    <Progress className="mt-2 h-1.5" value={member.qualityScore} />
                    {member.similarity !== undefined ? (
                      <p className="mt-2 text-right text-xs text-muted-foreground">
                        相似 {Math.round(member.similarity * 100)}%
                      </p>
                    ) : null}
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        ))}
      </div>
      {!groups.length ? (
        <p className="py-14 text-center text-sm text-muted-foreground">
          尚无重复组。分析完成后会按五个维度分别显示。
        </p>
      ) : null}
      {total > 25 ? (
        <div className="flex flex-wrap items-center justify-between gap-3 text-sm text-muted-foreground">
          <span>
            共 {total} 组 · 第 {page}/{Math.ceil(total / 25)} 页
          </span>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={page <= 1}
              onClick={() => void refresh(kind, page - 1)}
            >
              上一页
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={page >= Math.ceil(total / 25)}
              onClick={() => void refresh(kind, page + 1)}
            >
              下一页
            </Button>
          </div>
        </div>
      ) : null}
      <OperationPreview operation={operation} />
      <InlineJobStatus
        job={operation ? latestJob("operation", operation.id, "trash") : undefined}
      />
      {operation?.status === "previewed" ? (
        <div className="flex justify-end">
          <Button variant="destructive" onClick={() => void confirmTrash()}>
            <Trash2 data-icon="inline-start" />
            确认移入回收站
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function labelKind(kind: DuplicateGroup["kind"]) {
  return (
    {
      hardlink_alias: "硬链接别名",
      binary_exact: "Binary Exact",
      audio_duplicate: "Audio Duplicate",
      quality_variant: "Quality Variant",
      possible_duplicate: "Possible Duplicate",
    } as const
  )[kind];
}
function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "重复分析操作失败");
}
