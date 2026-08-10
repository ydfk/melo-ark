import { zodResolver } from "@hookform/resolvers/zod";
import { FolderSearch, Plus } from "lucide-react";
import { lazy, Suspense, useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { toast } from "sonner";
import { z } from "zod";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { LibraryRootCard } from "@/features/library/library-root-card";
import { TrackCatalog } from "@/features/library/track-catalog";
import { ApiError } from "@/lib/api";
import {
  createLibrary,
  createScrapeJob,
  getTracks,
  preflightLibraryPath,
  scanLibrary,
} from "@/lib/api/methods/library";
import type { LibraryRoot, TrackFilter, TrackList } from "@/lib/api/types";
import { usePlaybackStore, type QueueTrack } from "@/store/playback-store";

const librarySchema = z.object({
  name: z.string().trim().min(1, "请输入曲库名称").max(80, "名称最多 80 个字符"),
  path: z.string().trim().startsWith("/", "请填写容器内绝对路径，例如 /music/source"),
  role: z.enum(["source", "managed", "both"]),
  watchEnabled: z.boolean(),
  writable: z.boolean(),
});

const TrackWorkbench = lazy(() =>
  import("@/features/library/track-workbench").then((module) => ({
    default: module.TrackWorkbench,
  }))
);
const BatchTagDialog = lazy(() =>
  import("@/features/library/batch-tag-dialog").then((module) => ({
    default: module.BatchTagDialog,
  }))
);

type LibraryForm = z.infer<typeof librarySchema>;

type LibraryPanelProps = {
  libraries: LibraryRoot[];
  refreshKey: number;
  onChanged: () => Promise<void>;
};

export function LibraryPanel({ libraries, refreshKey, onChanged }: LibraryPanelProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [tracks, setTracks] = useState<TrackList>();
  const [search, setSearch] = useState("");
  const [committedSearch, setCommittedSearch] = useState("");
  const [trackFilter, setTrackFilter] = useState<TrackFilter>();
  const [page, setPage] = useState(1);
  const [loadingTracks, setLoadingTracks] = useState(false);
  const [selectedTrackId, setSelectedTrackId] = useState<string>();
  const [selectedTrackIds, setSelectedTrackIds] = useState<Set<string>>(new Set());
  const [batchOpen, setBatchOpen] = useState(false);
  const form = useForm<LibraryForm>({
    resolver: zodResolver(librarySchema),
    defaultValues: {
      name: "",
      path: "/music/source",
      role: "source",
      watchEnabled: false,
      writable: false,
    },
  });

  useEffect(() => {
    setLoadingTracks(true);
    getTracks(page, 50, committedSearch, trackFilter)
      .send()
      .then(setTracks)
      .catch(() => toast.error("加载曲目失败"))
      .finally(() => setLoadingTracks(false));
  }, [committedSearch, page, refreshKey, trackFilter]);

  const submit = form.handleSubmit(async (values) => {
    try {
      const status = await preflightLibraryPath(values.path).send();
      await createLibrary({
        ...values,
        path: status.canonicalPath,
        scanEnabled: true,
      }).send();
      toast.success("Library Root 已添加");
      form.reset();
      setDialogOpen(false);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法添加 Library Root");
    }
  });

  async function startScan(library: LibraryRoot) {
    try {
      await scanLibrary(library.id).send();
      toast.success(`已开始扫描「${library.name}」`);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建扫描任务");
    }
  }

  async function startBatchScrape() {
    try {
      await createScrapeJob([...selectedTrackIds]).send();
      toast.success(`已创建 ${selectedTrackIds.size} 首曲目的批量刮削任务`);
      setSelectedTrackIds(new Set());
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建批量刮削任务");
    }
  }

  function playTrack(trackId: string) {
    const queue: QueueTrack[] =
      tracks?.items.map((track) => ({
        id: track.id,
        mediaId: track.mediaId,
        title: track.title,
        artist: track.artist,
        album: track.album,
        durationMs: track.durationMs,
      })) ?? [];
    const track = queue.find((item) => item.id === trackId);
    if (track) usePlaybackStore.getState().play(track, queue);
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.2em] text-primary">Library Roots</p>
          <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">曲库与扫描</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            路径必须是 Docker 已挂载的容器内目录。基础扫描不会计算完整 Hash 或音频指纹。
          </p>
        </div>
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          <DialogTrigger asChild>
            <Button>
              <Plus data-icon="inline-start" />
              添加曲库
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>添加 Library Root</DialogTitle>
              <DialogDescription>
                填写容器内绝对路径。保存前会 canonicalize 并检查可读写状态。
              </DialogDescription>
            </DialogHeader>
            <form id="library-form" onSubmit={submit}>
              <FieldGroup>
                <Field data-invalid={Boolean(form.formState.errors.name)}>
                  <FieldLabel htmlFor="library-name">名称</FieldLabel>
                  <Input
                    id="library-name"
                    {...form.register("name")}
                    aria-invalid={Boolean(form.formState.errors.name)}
                  />
                  <FieldError errors={[form.formState.errors.name]} />
                </Field>
                <Field data-invalid={Boolean(form.formState.errors.path)}>
                  <FieldLabel htmlFor="library-path">容器内路径</FieldLabel>
                  <Input
                    id="library-path"
                    className="font-mono"
                    {...form.register("path")}
                    aria-invalid={Boolean(form.formState.errors.path)}
                  />
                  <FieldDescription>
                    例如 /music/source，而不是宿主机的 /mnt/nas/music。
                  </FieldDescription>
                  <FieldError errors={[form.formState.errors.path]} />
                </Field>
                <Controller
                  control={form.control}
                  name="role"
                  render={({ field }) => (
                    <Field>
                      <FieldLabel>用途</FieldLabel>
                      <ToggleGroup
                        type="single"
                        value={field.value}
                        onValueChange={(value) => value && field.onChange(value)}
                      >
                        <ToggleGroupItem value="source">来源</ToggleGroupItem>
                        <ToggleGroupItem value="managed">已整理</ToggleGroupItem>
                        <ToggleGroupItem value="both">两者</ToggleGroupItem>
                      </ToggleGroup>
                    </Field>
                  )}
                />
                <Controller
                  control={form.control}
                  name="watchEnabled"
                  render={({ field }) => (
                    <Field orientation="horizontal">
                      <div className="flex-1">
                        <FieldLabel htmlFor="watch-enabled">文件变化 Watch</FieldLabel>
                        <FieldDescription>
                          NAS Watch 不保证可靠，仍会定期 reconcile。
                        </FieldDescription>
                      </div>
                      <Switch
                        id="watch-enabled"
                        checked={field.value}
                        onCheckedChange={field.onChange}
                      />
                    </Field>
                  )}
                />
                <Controller
                  control={form.control}
                  name="writable"
                  render={({ field }) => (
                    <Field orientation="horizontal">
                      <div className="flex-1">
                        <FieldLabel htmlFor="library-writable">允许后续写入</FieldLabel>
                        <FieldDescription>只声明能力，不会在扫描时修改文件。</FieldDescription>
                      </div>
                      <Switch
                        id="library-writable"
                        checked={field.value}
                        onCheckedChange={field.onChange}
                      />
                    </Field>
                  )}
                />
              </FieldGroup>
            </form>
            <DialogFooter>
              <Button variant="outline" onClick={() => setDialogOpen(false)}>
                取消
              </Button>
              <Button form="library-form" type="submit" disabled={form.formState.isSubmitting}>
                {form.formState.isSubmitting ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <FolderSearch data-icon="inline-start" />
                )}
                预检并添加
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>

      {libraries.length ? (
        <section className="grid gap-4 lg:grid-cols-2">
          {libraries.map((library) => (
            <LibraryRootCard
              key={library.id}
              library={library}
              onScan={startScan}
              onChanged={onChanged}
            />
          ))}
        </section>
      ) : (
        <Alert>
          <FolderSearch />
          <AlertTitle>还没有 Library Root</AlertTitle>
          <AlertDescription>先在 Compose 挂载音乐目录，再添加对应的容器内路径。</AlertDescription>
        </Alert>
      )}

      <TrackCatalog
        tracks={tracks}
        loading={loadingTracks}
        search={search}
        filter={trackFilter}
        page={page}
        selected={selectedTrackIds}
        onSearchChange={setSearch}
        onSearch={() => {
          setPage(1);
          setCommittedSearch(search.trim());
        }}
        onFilterChange={(value) => {
          setPage(1);
          setTrackFilter(value);
          setSelectedTrackIds(new Set());
        }}
        onPageChange={setPage}
        onSelectionChange={setSelectedTrackIds}
        onOpenTrack={setSelectedTrackId}
        onPlayTrack={playTrack}
        onBatchTag={() => setBatchOpen(true)}
        onBatchScrape={() => void startBatchScrape()}
      />
      <Suspense fallback={null}>
        <TrackWorkbench
          trackId={selectedTrackId}
          open={Boolean(selectedTrackId)}
          libraries={libraries}
          onOpenChange={(open) => !open && setSelectedTrackId(undefined)}
          onChanged={onChanged}
        />
        <BatchTagDialog
          trackIds={[...selectedTrackIds]}
          open={batchOpen}
          onOpenChange={setBatchOpen}
          onChanged={onChanged}
        />
      </Suspense>
    </div>
  );
}
