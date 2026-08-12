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
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { LibraryRootCard } from "@/features/library/library-root-card";
import { DirectoryTreePicker } from "@/features/library/directory-tree-picker";
import { TrackCatalog } from "@/features/library/track-catalog";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
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
  path: z.string().trim().min(1, "请选择目录"),
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
  const { latestJob, registerJob } = useJobActivity();
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
      path: "",
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
      toast.success("曲库已添加");
      form.reset({ path: "", role: "source", watchEnabled: false, writable: false });
      setDialogOpen(false);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法添加曲库");
    }
  });

  async function startScan(library: LibraryRoot) {
    try {
      registerJob(await scanLibrary(library.id).send());
      toast.success(`已开始扫描「${library.path}」`);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建扫描任务");
    }
  }

  async function startBatchScrape() {
    try {
      registerJob(await createScrapeJob([...selectedTrackIds]).send());
      toast.success(`已创建 ${selectedTrackIds.size} 首曲目的元数据匹配任务`);
      setSelectedTrackIds(new Set());
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建元数据匹配任务");
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
          <h1 className="font-display text-3xl font-semibold tracking-tight">曲库</h1>
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
              <DialogTitle>添加曲库</DialogTitle>
              <DialogDescription>选择音乐目录并设置用途。</DialogDescription>
            </DialogHeader>
            <form id="library-form" onSubmit={submit}>
              <FieldGroup>
                <Controller
                  control={form.control}
                  name="path"
                  render={({ field }) => (
                    <Field data-invalid={Boolean(form.formState.errors.path)}>
                      <FieldLabel>目录</FieldLabel>
                      <DirectoryTreePicker value={field.value} onChange={field.onChange} />
                      <FieldError errors={[form.formState.errors.path]} />
                    </Field>
                  )}
                />
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
                        <FieldLabel htmlFor="watch-enabled">文件监听</FieldLabel>
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
          <AlertTitle>尚未添加曲库</AlertTitle>
          <AlertDescription>添加一个音乐目录即可开始扫描。</AlertDescription>
        </Alert>
      )}

      <InlineJobStatus job={latestJob("workspace", "library", "scrape")} />

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
