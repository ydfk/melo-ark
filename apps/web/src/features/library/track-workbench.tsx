import { ArrowRight, FileAudio, Link2, RotateCcw, Save, Trash2, WandSparkles } from "lucide-react";
import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
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
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { ApiError } from "@/lib/api";
import { OperationPreview } from "@/features/library/operation-preview";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import {
  applyOrganizer,
  applyTrash,
  applyTags,
  getTrack,
  getTrackFiles,
  getRuntimeSettings,
  previewOrganizer,
  previewTrash,
  restoreTrash,
  previewTags,
  undoOrganizer,
  undoTags,
} from "@/lib/api/methods/library";
import type { LibraryGroup, MediaFile, Operation, TagField, TrackDetail } from "@/lib/api/types";
import { formatBytes, formatDuration } from "@/lib/format";

const defaultTemplate = "{artist}/{album}/{track:02} - {title}.{ext}";

const ScraperWorkspace = lazy(() =>
  import("@/features/library/scraper-workspace").then((module) => ({
    default: module.ScraperWorkspace,
  }))
);
const LyricsCenter = lazy(() =>
  import("@/features/library/lyrics-center").then((module) => ({ default: module.LyricsCenter }))
);
const TrackArtworkPanel = lazy(() =>
  import("@/features/library/track-artwork-panel").then((module) => ({
    default: module.TrackArtworkPanel,
  }))
);
const TrackHistoryPanel = lazy(() =>
  import("@/features/library/track-history-panel").then((module) => ({
    default: module.TrackHistoryPanel,
  }))
);

type TrackWorkbenchProps = {
  trackId?: string;
  open: boolean;
  libraries: LibraryGroup[];
  onOpenChange: (open: boolean) => void;
  onChanged: () => Promise<void>;
};

type EditValues = {
  title: string;
  artists: string;
  album: string;
  albumArtist: string;
  trackNo: string;
  discNo: string;
  year: string;
  genre: string;
};

export function TrackWorkbench({
  trackId,
  open,
  libraries,
  onOpenChange,
  onChanged,
}: TrackWorkbenchProps) {
  const { latestJob } = useJobActivity();
  const [track, setTrack] = useState<TrackDetail>();
  const [files, setFiles] = useState<MediaFile[]>([]);
  const [values, setValues] = useState<EditValues>();
  const [operation, setOperation] = useState<Operation>();
  const [busy, setBusy] = useState(false);
  const [simplify, setSimplify] = useState(false);
  const [coverDataBase64, setCoverDataBase64] = useState<string>();
  const [targetLibraryId, setTargetLibraryId] = useState("");
  const [template, setTemplate] = useState(defaultTemplate);
  const [crossPlatformSafe, setCrossPlatformSafe] = useState(true);

  const writableFiles = useMemo(
    () => files.filter((file) => file.libraryWritable && file.available),
    [files]
  );
  const organizerTargets = libraries.flatMap((library) =>
    library.organizedLibraryId && library.organizedPath
      ? [{ id: library.organizedLibraryId, path: library.organizedPath }]
      : []
  );

  useEffect(() => {
    if (!open || !trackId) return;
    setOperation(undefined);
    Promise.all([
      getTrack(trackId).send(),
      getTrackFiles(trackId).send(),
      getRuntimeSettings()
        .send()
        .catch(() => undefined),
    ])
      .then(([nextTrack, nextFiles, runtimeSettings]) => {
        setTrack(nextTrack);
        setFiles(nextFiles);
        if (runtimeSettings) {
          setTemplate(runtimeSettings.values.organizerTemplate);
          setCrossPlatformSafe(runtimeSettings.values.organizerCrossPlatformSafe);
        }
        setValues({
          title: nextTrack.title,
          artists: nextTrack.artists,
          album: nextTrack.album,
          albumArtist: nextTrack.albumArtist ?? "",
          trackNo: nextTrack.trackNo?.toString() ?? "",
          discNo: nextTrack.discNo?.toString() ?? "",
          year: nextTrack.year?.toString() ?? "",
          genre: nextTrack.genre ?? "",
        });
      })
      .catch(showError);
  }, [open, trackId]);

  async function previewTagChanges() {
    if (!values || !writableFiles.length) return;
    setBusy(true);
    try {
      const clear: TagField[] = [];
      if (!values.albumArtist.trim()) clear.push("albumArtist" as const);
      if (!values.trackNo) clear.push("trackNo" as const);
      if (!values.discNo) clear.push("discNo" as const);
      if (!values.year) clear.push("year" as const);
      if (!values.genre.trim()) clear.push("genre" as const);
      const next = await previewTags({
        mediaIds: writableFiles.map((file) => file.id),
        set: {
          title: values.title,
          artists: splitArtists(values.artists),
          album: values.album,
          ...(values.albumArtist.trim() ? { albumArtist: values.albumArtist.trim() } : {}),
          ...(toNumber(values.trackNo) ? { trackNo: toNumber(values.trackNo) } : {}),
          ...(toNumber(values.discNo) ? { discNo: toNumber(values.discNo) } : {}),
          ...(toNumber(values.year) ? { year: toNumber(values.year) } : {}),
          ...(values.genre.trim() ? { genre: values.genre.trim() } : {}),
          ...(coverDataBase64 ? { coverDataBase64 } : {}),
        },
        clear,
        transforms: simplify
          ? [
              {
                kind: "traditionalToSimplified",
                fields: ["title", "artists", "album", "albumArtist", "genre"] as TagField[],
              },
            ]
          : [],
      }).send();
      setOperation(next);
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function confirmTags() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await applyTags(operation.id).send());
      toast.success("Tag 已写入并保存快照");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function revertTags() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await undoTags(operation.id).send());
      toast.success("Tag 修改已撤销");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function previewOrganize() {
    if (!targetLibraryId || !files.length) return;
    setBusy(true);
    try {
      setOperation(
        await previewOrganizer({
          mediaIds: files.map((file) => file.id),
          targetLibraryId,
          template,
          crossPlatformSafe,
        }).send()
      );
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function confirmOrganizer() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await applyOrganizer(operation.id).send());
      toast.success("硬链接整理已执行");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function revertOrganizer() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await undoOrganizer(operation.id).send());
      toast.success("整理操作已撤销");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function prepareTrash(file: MediaFile) {
    setBusy(true);
    try {
      setOperation(await previewTrash([file.id]).send());
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
      toast.success("文件已移入曲库回收站");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function restoreFromTrash() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await restoreTrash(operation.id).send());
      toast.success("文件已恢复原路径");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-3xl">
        <SheetHeader className="pr-8">
          <SheetTitle className="font-display text-2xl">{track?.title ?? "曲目详情"}</SheetTitle>
          <SheetDescription>
            {track ? `${track.artists} · ${track.album}` : "正在读取逻辑曲目与物理文件…"}
          </SheetDescription>
        </SheetHeader>

        <Tabs defaultValue="overview" className="mt-6">
          <TabsList className="w-full justify-start overflow-x-auto">
            <TabsTrigger value="overview">概览</TabsTrigger>
            <TabsTrigger value="tags">Tag</TabsTrigger>
            <TabsTrigger value="files">文件</TabsTrigger>
            <TabsTrigger value="organizer">整理</TabsTrigger>
            <TabsTrigger value="scraper">刮削</TabsTrigger>
            <TabsTrigger value="lyrics">歌词</TabsTrigger>
            <TabsTrigger value="artwork">Artwork</TabsTrigger>
            <TabsTrigger value="history">History</TabsTrigger>
          </TabsList>

          <TabsContent value="overview" className="mt-5 grid gap-4 sm:grid-cols-2">
            <Metric label="时长" value={formatDuration(track?.durationMs)} />
            <Metric label="物理变体" value={`${files.length}`} />
            <Metric label="年份" value={track?.year?.toString() ?? "未填写"} />
            <Metric label="流派" value={track?.genre ?? "未填写"} />
          </TabsContent>

          <TabsContent value="tags" className="mt-5 space-y-5">
            {!writableFiles.length ? (
              <Alert variant="destructive">
                <AlertTitle>当前文件不可写</AlertTitle>
                <AlertDescription>当前没有可修改的整理文件。</AlertDescription>
              </Alert>
            ) : null}
            {values ? (
              <FieldGroup className="grid gap-4 sm:grid-cols-2">
                <EditField
                  label="标题"
                  value={values.title}
                  onChange={(title) => setValues({ ...values, title })}
                />
                <EditField
                  label="歌手"
                  value={values.artists}
                  onChange={(artists) => setValues({ ...values, artists })}
                  description="多个歌手用 ; 分隔"
                />
                <EditField
                  label="专辑"
                  value={values.album}
                  onChange={(album) => setValues({ ...values, album })}
                />
                <EditField
                  label="专辑歌手"
                  value={values.albumArtist}
                  onChange={(albumArtist) => setValues({ ...values, albumArtist })}
                />
                <EditField
                  label="曲序"
                  type="number"
                  value={values.trackNo}
                  onChange={(trackNo) => setValues({ ...values, trackNo })}
                />
                <EditField
                  label="碟序"
                  type="number"
                  value={values.discNo}
                  onChange={(discNo) => setValues({ ...values, discNo })}
                />
                <EditField
                  label="年份"
                  type="number"
                  value={values.year}
                  onChange={(year) => setValues({ ...values, year })}
                />
                <EditField
                  label="流派"
                  value={values.genre}
                  onChange={(genre) => setValues({ ...values, genre })}
                />
              </FieldGroup>
            ) : null}
            <Field orientation="horizontal" className="rounded-xl border p-4">
              <div className="flex-1">
                <FieldLabel htmlFor="simplify-tags">繁体转简体</FieldLabel>
                <FieldDescription>仅在预览副本中转换，不会修改原始 Tag。</FieldDescription>
              </div>
              <Switch id="simplify-tags" checked={simplify} onCheckedChange={setSimplify} />
            </Field>
            <OperationPreview operation={operation?.kind === "tag_edit" ? operation : undefined} />
            <InlineJobStatus
              job={
                operation?.kind === "tag_edit"
                  ? latestJob("operation", operation.id, "tag_edit")
                  : undefined
              }
            />
            <div className="flex flex-wrap justify-end gap-2">
              {operation?.kind === "tag_edit" && operation.status === "completed" ? (
                <Button variant="outline" onClick={() => void revertTags()} disabled={busy}>
                  <RotateCcw data-icon="inline-start" />
                  撤销
                </Button>
              ) : null}
              <Button
                variant="outline"
                onClick={() => void previewTagChanges()}
                disabled={busy || !writableFiles.length}
              >
                <WandSparkles data-icon="inline-start" />
                生成变更预览
              </Button>
              {operation?.kind === "tag_edit" && operation.status === "previewed" ? (
                <Button onClick={() => void confirmTags()} disabled={busy}>
                  {busy ? <Spinner data-icon="inline-start" /> : <Save data-icon="inline-start" />}
                  确认写入
                </Button>
              ) : null}
            </div>
          </TabsContent>

          <TabsContent value="files" className="mt-5 space-y-3">
            {files.map((file) => (
              <div key={file.id} className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <p className="truncate font-medium">{file.path}</p>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {file.libraryPath} · dev {file.deviceId} · inode {file.inode}
                    </p>
                  </div>
                  <div className="flex gap-2">
                    {!file.available ? <Badge variant="destructive">文件不可用</Badge> : null}
                    <Badge variant="secondary">{file.extension.toUpperCase()}</Badge>
                  </div>
                </div>
                <div className="mt-3 flex flex-wrap gap-2 text-xs text-muted-foreground">
                  <span>{formatBytes(file.fileSize)}</span>
                  <span>·</span>
                  <span>{file.bitDepth ? `${file.bitDepth} bit` : "位深未知"}</span>
                  <span>·</span>
                  <span>{file.sampleRate ? `${file.sampleRate / 1000} kHz` : "采样率未知"}</span>
                  <span>·</span>
                  <span>{file.hardlinkCount} 个硬链接</span>
                </div>
                <div className="mt-3 flex justify-end">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={!file.libraryWritable || !file.available || busy}
                    onClick={() => void prepareTrash(file)}
                  >
                    <Trash2 data-icon="inline-start" />
                    移入回收站…
                  </Button>
                </div>
              </div>
            ))}
            <OperationPreview operation={operation?.kind === "trash" ? operation : undefined} />
            <InlineJobStatus
              job={
                operation?.kind === "trash"
                  ? latestJob("operation", operation.id, "trash")
                  : undefined
              }
            />
            {operation?.kind === "trash" ? (
              <div className="flex justify-end gap-2">
                {operation.status === "completed" ? (
                  <Button variant="outline" onClick={() => void restoreFromTrash()} disabled={busy}>
                    <RotateCcw data-icon="inline-start" />
                    恢复原路径
                  </Button>
                ) : null}
                {operation.status === "previewed" ? (
                  <Button variant="destructive" onClick={() => void confirmTrash()} disabled={busy}>
                    <Trash2 data-icon="inline-start" />
                    确认移入回收站
                  </Button>
                ) : null}
              </div>
            ) : null}
          </TabsContent>

          <TabsContent value="organizer" className="mt-5 space-y-5">
            <Alert>
              <Link2 />
              <AlertTitle>默认只创建硬链接</AlertTitle>
              <AlertDescription>不能跨文件系统，目标冲突时不会覆盖。</AlertDescription>
            </Alert>
            <Field>
              <FieldLabel>目标曲库</FieldLabel>
              <Select value={targetLibraryId} onValueChange={setTargetLibraryId}>
                <SelectTrigger>
                  <SelectValue placeholder="选择可写的已整理曲库" />
                </SelectTrigger>
                <SelectContent>
                  {organizerTargets.map((library) => (
                    <SelectItem key={library.id} value={library.id}>
                      {library.path}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field>
              <FieldLabel htmlFor="organizer-template">路径模板</FieldLabel>
              <Textarea
                id="organizer-template"
                className="min-h-24 font-mono"
                value={template}
                onChange={(event) => setTemplate(event.target.value)}
              />
              <FieldDescription>
                支持 artist、album、track:02、title、ext、year、quality 等变量。
              </FieldDescription>
            </Field>
            <Field orientation="horizontal" className="rounded-xl border p-4">
              <div className="flex-1">
                <FieldLabel htmlFor="cross-platform">跨平台安全文件名</FieldLabel>
                <FieldDescription>替换 Windows/macOS/Linux 不兼容字符。</FieldDescription>
              </div>
              <Switch
                id="cross-platform"
                checked={crossPlatformSafe}
                onCheckedChange={setCrossPlatformSafe}
              />
            </Field>
            <OperationPreview operation={operation?.kind === "organize" ? operation : undefined} />
            <InlineJobStatus
              job={
                operation?.kind === "organize"
                  ? latestJob("operation", operation.id, "organize")
                  : undefined
              }
            />
            <div className="flex flex-wrap justify-end gap-2">
              {operation?.kind === "organize" && operation.status === "completed" ? (
                <Button variant="outline" onClick={() => void revertOrganizer()} disabled={busy}>
                  <RotateCcw data-icon="inline-start" />
                  撤销整理
                </Button>
              ) : null}
              <Button
                variant="outline"
                onClick={() => void previewOrganize()}
                disabled={busy || !targetLibraryId}
              >
                <FileAudio data-icon="inline-start" />
                Dry Run
              </Button>
              {operation?.kind === "organize" && operation.status === "previewed" ? (
                <Button
                  onClick={() => void confirmOrganizer()}
                  disabled={
                    busy ||
                    operation.items.some((item) => item.preflight && !item.preflight.canApply)
                  }
                >
                  <ArrowRight data-icon="inline-start" />
                  确认创建硬链接
                </Button>
              ) : null}
            </div>
          </TabsContent>

          <TabsContent value="scraper" className="mt-5">
            {trackId ? (
              <Suspense
                fallback={
                  <p className="py-10 text-center text-sm text-muted-foreground">
                    正在装载刮削工作台…
                  </p>
                }
              >
                <ScraperWorkspace trackId={trackId} onChanged={onChanged} />
              </Suspense>
            ) : null}
          </TabsContent>

          <TabsContent value="lyrics" className="mt-5">
            {trackId ? (
              <Suspense
                fallback={
                  <p className="py-10 text-center text-sm text-muted-foreground">
                    正在装载歌词中心…
                  </p>
                }
              >
                <LyricsCenter trackId={trackId} files={files} onChanged={onChanged} />
              </Suspense>
            ) : null}
          </TabsContent>

          <TabsContent value="artwork" className="mt-5">
            <Suspense
              fallback={
                <p className="py-10 text-center text-sm text-muted-foreground">正在装载封面…</p>
              }
            >
              <TrackArtworkPanel
                files={files}
                pending={Boolean(coverDataBase64)}
                onChange={setCoverDataBase64}
              />
            </Suspense>
          </TabsContent>

          <TabsContent value="history" className="mt-5">
            {trackId ? (
              <Suspense
                fallback={
                  <p className="py-10 text-center text-sm text-muted-foreground">正在装载历史…</p>
                }
              >
                <TrackHistoryPanel trackId={trackId} />
              </Suspense>
            ) : null}
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}

function EditField({
  label,
  value,
  onChange,
  description,
  type = "text",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  description?: string;
  type?: "text" | "number";
}) {
  const id = `track-${label}`;
  return (
    <Field>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input
        id={id}
        type={type}
        min={type === "number" ? 0 : undefined}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {description ? <FieldDescription>{description}</FieldDescription> : null}
    </Field>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border bg-card/60 p-4">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-2 font-display text-2xl font-semibold">{value}</p>
    </div>
  );
}

function splitArtists(value: string) {
  return value
    .split(/[;、&]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function toNumber(value: string) {
  const parsed = Number(value);
  return value && Number.isInteger(parsed) && parsed >= 0 ? parsed : undefined;
}

function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "操作失败，请检查服务日志");
}
