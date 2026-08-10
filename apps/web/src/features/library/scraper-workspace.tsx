import { Check, Image, RefreshCw, Search, ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { OperationPreview } from "@/features/library/operation-preview";
import { ApiError } from "@/lib/api";
import {
  applyTags,
  getScrapeCandidates,
  previewScrapeCandidate,
  searchScrapeCandidates,
} from "@/lib/api/methods/library";
import type { Operation, ScrapeCandidate } from "@/lib/api/types";
import { formatDuration } from "@/lib/format";

export function ScraperWorkspace({
  trackId,
  onChanged,
}: {
  trackId: string;
  onChanged: () => Promise<void>;
}) {
  const [candidates, setCandidates] = useState<ScrapeCandidate[]>([]);
  const [failures, setFailures] = useState<Array<{ providerId: string; message: string }>>([]);
  const [operation, setOperation] = useState<Operation>();
  const [includeArtwork, setIncludeArtwork] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getScrapeCandidates(trackId)
      .send()
      .then(setCandidates)
      .catch(() => undefined);
  }, [trackId]);

  async function searchAll() {
    setBusy(true);
    try {
      const response = await searchScrapeCandidates(trackId).send();
      setCandidates(response.candidates);
      setFailures(response.failures);
      toast.success(`找到 ${response.candidates.length} 个候选`);
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function prepare(candidate: ScrapeCandidate) {
    setBusy(true);
    try {
      setOperation(
        await previewScrapeCandidate(
          candidate,
          includeArtwork && Boolean(candidate.artworkUrl)
        ).send()
      );
      toast.success("已生成 Tag Diff，尚未写入文件");
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await applyTags(operation.id).send());
      toast.success("刮削元数据已写入");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-5">
      <Alert>
        <ShieldAlert />
        <AlertTitle>候选永远先人工审阅</AlertTitle>
        <AlertDescription>
          95 分以上也只进入可接受队列；80–94 分必须显式确认，低于 80 分会显示风险。
        </AlertDescription>
      </Alert>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={includeArtwork}
            onCheckedChange={(value) => setIncludeArtwork(value === true)}
          />
          选中候选时同时下载封面
        </label>
        <Button variant="outline" onClick={() => void searchAll()} disabled={busy}>
          {busy ? <Spinner data-icon="inline-start" /> : <Search data-icon="inline-start" />}
          搜索全部 Provider
        </Button>
      </div>
      {failures.length ? (
        <Alert variant="destructive">
          <RefreshCw />
          <AlertTitle>部分 Provider 已降级</AlertTitle>
          <AlertDescription>
            {failures.map((item) => `${item.providerId}: ${item.message}`).join("；")}
          </AlertDescription>
        </Alert>
      ) : null}
      <div className="space-y-3">
        {candidates.map((candidate) => (
          <article
            key={candidate.id}
            className="grid gap-4 rounded-xl border bg-card/60 p-4 sm:grid-cols-[5rem_1fr_auto]"
          >
            <div className="flex aspect-square items-center justify-center overflow-hidden rounded-lg bg-muted">
              {candidate.artworkUrl ? (
                <img
                  className="size-full object-cover"
                  src={candidate.artworkUrl}
                  alt="候选封面"
                  loading="lazy"
                />
              ) : (
                <Image className="size-6 text-muted-foreground" />
              )}
            </div>
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <Badge
                  variant={
                    candidate.score >= 95
                      ? "default"
                      : candidate.score >= 80
                        ? "secondary"
                        : "destructive"
                  }
                >
                  {candidate.score} 分
                </Badge>
                <Badge variant="outline">{candidate.providerId}</Badge>
                {candidate.versionLabel ? (
                  <Badge variant="outline">{candidate.versionLabel}</Badge>
                ) : null}
              </div>
              <h3 className="mt-2 truncate font-semibold">{candidate.title}</h3>
              <p className="text-sm text-muted-foreground">
                {candidate.artistsJson.join(" / ")} · {candidate.album ?? "专辑未知"}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {formatDuration(candidate.durationMs)} · {candidate.year ?? "年份未知"} · 曲序{" "}
                {candidate.trackNo ?? "—"}
              </p>
              <Progress className="mt-3 h-1.5" value={candidate.score} />
              {candidate.differencesJson.length ? (
                <p className="mt-2 text-xs text-amber-500">
                  差异：{candidate.differencesJson.join("、")}
                </p>
              ) : null}
            </div>
            <Button size="sm" onClick={() => void prepare(candidate)} disabled={busy}>
              选择
            </Button>
          </article>
        ))}
        {!candidates.length ? (
          <p className="py-8 text-center text-sm text-muted-foreground">
            尚无候选。点击搜索后，各 Provider 会独立返回结果或故障状态。
          </p>
        ) : null}
      </div>
      <OperationPreview operation={operation} />
      {operation?.status === "previewed" ? (
        <div className="flex justify-end">
          <Button onClick={() => void confirm()} disabled={busy}>
            <Check data-icon="inline-start" />
            确认写入 Diff
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "刮削操作失败");
}
