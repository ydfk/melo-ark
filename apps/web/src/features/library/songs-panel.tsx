import { lazy, Suspense, useEffect, useState } from "react";
import { toast } from "sonner";

import { TrackCatalog } from "@/features/library/track-catalog";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { ApiError } from "@/lib/api";
import { createScrapeJob, getManagedMediaFiles } from "@/lib/api/methods/library";
import type { LibraryGroup, ManagedMediaFilePage, TrackFilter } from "@/lib/api/types";
import { usePlaybackStore, type QueueTrack } from "@/store/playback-store";

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

type SongsPanelProps = {
  libraries: LibraryGroup[];
  refreshKey: number;
  onChanged: () => Promise<void>;
};

export function SongsPanel({ libraries, refreshKey, onChanged }: SongsPanelProps) {
  const { latestJob, registerJob } = useJobActivity();
  const [tracks, setTracks] = useState<ManagedMediaFilePage>();
  const [search, setSearch] = useState("");
  const [committedSearch, setCommittedSearch] = useState("");
  const [trackFilter, setTrackFilter] = useState<TrackFilter>();
  const [page, setPage] = useState(1);
  const [perPage, setPerPage] = useState(50);
  const [loadingTracks, setLoadingTracks] = useState(false);
  const [selectedTrackId, setSelectedTrackId] = useState<string>();
  const [selectedTrackIds, setSelectedTrackIds] = useState<Set<string>>(new Set());
  const [batchOpen, setBatchOpen] = useState(false);

  useEffect(() => {
    setLoadingTracks(true);
    getManagedMediaFiles(page, perPage, committedSearch, trackFilter)
      .send()
      .then((result) => {
        const lastPage = Math.max(1, Math.ceil(result.total / result.perPage));
        if (page > lastPage) {
          setPage(lastPage);
          return;
        }
        setTracks(result);
      })
      .catch(() => toast.error("加载歌曲失败"))
      .finally(() => setLoadingTracks(false));
  }, [committedSearch, page, perPage, refreshKey, trackFilter]);

  async function startBatchScrape() {
    const trackIds = [
      ...new Set(
        tracks?.items
          .filter((file) => selectedTrackIds.has(file.mediaId))
          .map((file) => file.trackId) ?? []
      ),
    ];
    if (!trackIds.length) return;
    try {
      registerJob(await createScrapeJob(trackIds).send());
      toast.success(`已创建 ${trackIds.length} 首歌曲的元数据匹配任务`);
      setSelectedTrackIds(new Set());
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建元数据匹配任务");
    }
  }

  function playTrack(mediaId: string) {
    const queue: QueueTrack[] =
      tracks?.items.map((track) => ({
        id: track.trackId,
        mediaId: track.mediaId,
        title: track.title,
        artist: track.artist,
        album: track.album,
        durationMs: track.durationMs,
      })) ?? [];
    const track = queue.find((item) => item.mediaId === mediaId);
    if (track) usePlaybackStore.getState().play(track, queue);
  }

  return (
    <div className="flex flex-col gap-6">
      <div>
        <p className="font-mono text-xs uppercase tracking-[0.24em] text-primary">Managed Music</p>
        <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">歌曲列表</h1>
      </div>

      <InlineJobStatus job={latestJob("workspace", "library", "scrape")} />

      <TrackCatalog
        tracks={tracks}
        loading={loadingTracks}
        search={search}
        filter={trackFilter}
        page={page}
        perPage={perPage}
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
        onPageChange={(value) => {
          setPage(value);
          setSelectedTrackIds(new Set());
        }}
        onPerPageChange={(value) => {
          setPage(1);
          setPerPage(value);
          setSelectedTrackIds(new Set());
        }}
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
          mediaIds={[...selectedTrackIds]}
          open={batchOpen}
          onOpenChange={setBatchOpen}
          onChanged={onChanged}
        />
      </Suspense>
    </div>
  );
}
