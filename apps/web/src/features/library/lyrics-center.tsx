import { Languages, Save, Search } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { ApiError } from "@/lib/api";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { applyLyrics, getLyrics, searchLyrics } from "@/lib/api/methods/library";
import type { LyricsRecord, MediaFile } from "@/lib/api/types";

export function LyricsCenter({
  trackId,
  files,
  onChanged,
}: {
  trackId: string;
  files: MediaFile[];
  onChanged: () => Promise<void>;
}) {
  const { latestJob } = useJobActivity();
  const [items, setItems] = useState<LyricsRecord[]>([]);
  const [selected, setSelected] = useState<LyricsRecord>();
  const [mediaId, setMediaId] = useState(files[0]?.id ?? "");
  const [mode, setMode] = useState<"external" | "embedded" | "both">("external");
  const [replaceExisting, setReplaceExisting] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getLyrics(trackId)
      .send()
      .then((next) => {
        setItems(next);
        setSelected(next[0]);
      })
      .catch(() => undefined);
  }, [trackId]);
  useEffect(() => {
    if (!mediaId && files[0]) setMediaId(files[0].id);
  }, [files, mediaId]);

  async function searchAll() {
    setBusy(true);
    try {
      const response = await searchLyrics(trackId).send();
      setItems(response.candidates);
      setSelected(response.candidates[0]);
      if (response.failures.length) toast.warning(`${response.failures.length} 个歌词源不可用`);
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    if (!selected || !mediaId) return;
    setBusy(true);
    try {
      const jobId = crypto.randomUUID();
      const saved = await applyLyrics({
        jobId,
        lyricsId: selected.id,
        mediaFileId: mediaId,
        mode,
        replaceExisting,
      }).send();
      setSelected(saved);
      setItems(await getLyrics(trackId).send());
      toast.success("歌词已按选定策略写入");
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
        <Languages />
        <AlertTitle>不会静默覆盖已有歌词</AlertTitle>
        <AlertDescription>
          外置 LRC 与内嵌歌词都需要明确选择替换；双语相同时间戳会原样保留。
        </AlertDescription>
      </Alert>
      <div className="flex justify-end">
        <Button variant="outline" onClick={() => void searchAll()} disabled={busy}>
          {busy ? <Spinner data-icon="inline-start" /> : <Search data-icon="inline-start" />}
          搜索本地与远程歌词
        </Button>
      </div>
      <div className="grid gap-3 sm:grid-cols-3">
        {items.map((item) => (
          <button
            type="button"
            key={item.id}
            onClick={() => setSelected(item)}
            className={`rounded-xl border p-3 text-left transition ${selected?.id === item.id ? "border-primary bg-primary/5" : "bg-card/60 hover:border-primary/40"}`}
          >
            <div className="flex flex-wrap gap-1">
              <Badge variant="secondary">{item.qualityScore} 分</Badge>
              {item.synced ? <Badge variant="outline">同步</Badge> : null}
              {item.active ? <Badge>使用中</Badge> : null}
            </div>
            <p className="mt-2 text-sm font-medium">{item.providerId ?? "本地"}</p>
            <p className="mt-1 text-xs text-muted-foreground">
              覆盖率 {item.coveragePercent}% · {item.storage}
            </p>
          </button>
        ))}
      </div>
      {selected ? (
        <Textarea
          className="min-h-72 font-mono text-xs"
          value={selected.content}
          readOnly
          aria-label="歌词预览"
        />
      ) : (
        <p className="py-10 text-center text-sm text-muted-foreground">尚无歌词候选</p>
      )}
      <div className="grid gap-3 sm:grid-cols-2">
        <Select value={mediaId} onValueChange={setMediaId}>
          <SelectTrigger>
            <SelectValue placeholder="写入哪个物理文件" />
          </SelectTrigger>
          <SelectContent>
            {files
              .filter((file) => file.libraryWritable)
              .map((file) => (
                <SelectItem key={file.id} value={file.id}>
                  {file.path}
                </SelectItem>
              ))}
          </SelectContent>
        </Select>
        <Select value={mode} onValueChange={(value) => setMode(value as typeof mode)}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="external">仅外置 LRC</SelectItem>
            <SelectItem value="embedded">仅内嵌</SelectItem>
            <SelectItem value="both">外置 + 内嵌</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={replaceExisting}
            onCheckedChange={(value) => setReplaceExisting(value === true)}
          />
          明确替换已有歌词
        </label>
        <Button onClick={() => void save()} disabled={busy || !selected || !mediaId}>
          <Save data-icon="inline-start" />
          按策略写入
        </Button>
      </div>
      <InlineJobStatus job={latestJob("track", trackId, "lyrics")} />
    </div>
  );
}

function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "歌词操作失败");
}
