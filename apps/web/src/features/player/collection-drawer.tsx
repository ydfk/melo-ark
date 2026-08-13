import { Album, Clock3, Disc3, ListMusic, LogIn, Play, Plus, Search, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAuth } from "@/features/auth/auth-context";
import { useIsMobile } from "@/hooks/use-mobile";
import { getCatalogTracks, type CatalogAlbum, type CatalogTrack } from "@/lib/api/methods/catalog";
import { createPlaylist, getPlaylists } from "@/lib/api/methods/library";
import type { Playlist } from "@/lib/api/types";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

import { CatalogArtwork } from "./catalog-artwork";
import { usePlayer } from "./player-context";

type CollectionDrawerProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  featured: CatalogTrack[];
  albums: CatalogAlbum[];
};

export function CollectionDrawer({ open, onOpenChange, featured, albums }: CollectionDrawerProps) {
  const mobile = useIsMobile();
  const player = usePlayer();
  const { isAuthenticated, requireAuth } = useAuth();
  const [activeTab, setActiveTab] = useState("search");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState(featured);
  const [searching, setSearching] = useState(false);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [playlistName, setPlaylistName] = useState("");
  const [savingPlaylist, setSavingPlaylist] = useState(false);

  useEffect(() => {
    if (!query.trim()) setResults(featured);
  }, [featured, query]);

  useEffect(() => {
    if (!open || !isAuthenticated) return;
    void getPlaylists()
      .send()
      .then(setPlaylists)
      .catch(() => undefined);
  }, [isAuthenticated, open]);

  async function searchCatalog(search = query) {
    setSearching(true);
    try {
      const page = await getCatalogTracks(1, 80, search).send();
      setResults(page.items);
    } catch {
      toast.error("无法搜索曲库");
    } finally {
      setSearching(false);
    }
  }

  async function saveQueue() {
    if (!playlistName.trim() || !player.queue.length) return;
    setSavingPlaylist(true);
    try {
      const playlist = await createPlaylist({
        name: playlistName.trim(),
        trackIds: player.queue.map((track) => track.id),
      }).send();
      setPlaylists((current) => [playlist, ...current]);
      setPlaylistName("");
      toast.success("当前队列已保存为歌单");
    } catch {
      toast.error("保存歌单失败");
    } finally {
      setSavingPlaylist(false);
    }
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side={mobile ? "bottom" : "right"}
        className={cn(
          "border-white/10 bg-[oklch(0.12_0.022_270/0.97)] p-0 text-white backdrop-blur-2xl",
          mobile ? "h-[84dvh] rounded-t-[2rem]" : "w-[min(36rem,92vw)] max-w-none sm:max-w-none"
        )}
      >
        <SheetHeader className="border-b border-white/8 px-5 py-5 pr-12 text-left sm:px-7">
          <SheetTitle className="font-display text-xl text-white">播放收藏</SheetTitle>
          <SheetDescription className="text-white/45">
            搜索音乐、浏览专辑或调整队列
          </SheetDescription>
        </SheetHeader>
        <Tabs
          value={activeTab}
          onValueChange={setActiveTab}
          className="flex h-[calc(100%-89px)] min-h-0 flex-col"
        >
          <TabsList className="mx-4 mt-4 grid h-10 grid-cols-4 bg-white/5 sm:mx-7">
            <TabsTrigger value="search">
              <Search />
              <span className="hidden sm:inline">搜索</span>
            </TabsTrigger>
            <TabsTrigger value="albums">
              <Album />
              <span className="hidden sm:inline">专辑</span>
            </TabsTrigger>
            <TabsTrigger value="queue">
              <ListMusic />
              <span className="hidden sm:inline">队列</span>
            </TabsTrigger>
            <TabsTrigger value="playlists">
              <Disc3 />
              <span className="hidden sm:inline">歌单</span>
            </TabsTrigger>
          </TabsList>

          <TabsContent value="search" className="min-h-0 flex-1 overflow-y-auto px-4 pb-8 sm:px-7">
            <form
              className="sticky top-0 z-10 flex gap-2 bg-[oklch(0.12_0.022_270/0.96)] py-4"
              onSubmit={(event) => {
                event.preventDefault();
                void searchCatalog();
              }}
            >
              <Input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="歌名、艺术家或专辑"
                aria-label="搜索音乐"
                className="h-11 rounded-xl border-white/10 bg-white/5"
              />
              <Button className="size-11 rounded-xl" size="icon" disabled={searching}>
                {searching ? <Spinner /> : <Search />}
                <span className="sr-only">搜索</span>
              </Button>
            </form>
            <TrackList
              tracks={results}
              currentId={player.current?.id}
              onPlay={(track) => {
                player.playTrack(track, results);
                onOpenChange(false);
              }}
            />
          </TabsContent>

          <TabsContent value="albums" className="min-h-0 flex-1 overflow-y-auto px-4 py-5 sm:px-7">
            <div className="grid grid-cols-2 gap-4">
              {albums.map((album) => (
                <button
                  key={album.id}
                  className="group min-w-0 text-left"
                  onClick={() => {
                    setQuery(album.title);
                    void searchCatalog(album.title);
                    setActiveTab("search");
                  }}
                >
                  <CatalogArtwork
                    mediaId={album.coverMediaId}
                    alt={album.title}
                    className="aspect-square w-full rounded-2xl shadow-lg shadow-black/25 transition duration-300 group-hover:-translate-y-1 group-hover:shadow-cyan-950/40"
                  />
                  <p className="mt-2 truncate text-sm font-medium text-white/90">{album.title}</p>
                  <p className="truncate text-xs text-white/38">
                    {album.artist} · {album.trackCount} 首
                  </p>
                </button>
              ))}
            </div>
            {!albums.length ? <EmptyMessage text="还没有可浏览的专辑" /> : null}
          </TabsContent>

          <TabsContent value="queue" className="min-h-0 flex-1 overflow-y-auto px-4 py-5 sm:px-7">
            <div className="mb-3 flex items-center justify-between">
              <p className="text-sm text-white/50">{player.queue.length} 首音乐</p>
              {player.queue.length ? (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-white/45 hover:bg-white/8 hover:text-white"
                  onClick={player.clearQueue}
                >
                  清空
                </Button>
              ) : null}
            </div>
            <div className="space-y-1">
              {player.queue.map((track, index) => (
                <div
                  key={`${track.id}-${index}`}
                  className={cn(
                    "group flex items-center gap-3 rounded-xl px-3 py-3",
                    index === player.currentIndex ? "bg-cyan-200/10" : "hover:bg-white/5"
                  )}
                >
                  <button
                    className="flex size-8 shrink-0 items-center justify-center rounded-full text-white/55 hover:bg-white/10 hover:text-white"
                    onClick={() => player.playQueueItem(index)}
                    aria-label={`播放 ${track.title}`}
                  >
                    {index === player.currentIndex && player.playing ? (
                      <span className="playing-bars" aria-hidden="true">
                        <i />
                        <i />
                        <i />
                      </span>
                    ) : (
                      <Play className="size-3.5 fill-current" />
                    )}
                  </button>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm text-white/85">{track.title}</p>
                    <p className="truncate text-xs text-white/35">{track.artist}</p>
                  </div>
                  <span className="text-[11px] text-white/30">
                    {formatDuration(track.durationMs)}
                  </span>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-8 text-white/30 opacity-0 hover:bg-white/8 hover:text-white group-hover:opacity-100 focus-visible:opacity-100"
                    onClick={() => player.removeFromQueue(index)}
                    aria-label={`将 ${track.title} 移出队列`}
                  >
                    <Trash2 />
                  </Button>
                </div>
              ))}
            </div>
            {!player.queue.length ? <EmptyMessage text="从搜索结果中选择音乐即可加入队列" /> : null}
          </TabsContent>

          <TabsContent
            value="playlists"
            className="min-h-0 flex-1 overflow-y-auto px-4 py-5 sm:px-7"
          >
            {isAuthenticated ? (
              <>
                <form
                  className="flex gap-2"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void saveQueue();
                  }}
                >
                  <Input
                    value={playlistName}
                    onChange={(event) => setPlaylistName(event.target.value)}
                    placeholder={player.queue.length ? "给当前队列命名" : "播放队列为空"}
                    aria-label="歌单名称"
                    disabled={!player.queue.length}
                    className="h-11 rounded-xl border-white/10 bg-white/5"
                  />
                  <Button
                    className="h-11 rounded-xl"
                    disabled={!player.queue.length || !playlistName.trim() || savingPlaylist}
                  >
                    {savingPlaylist ? <Spinner /> : <Plus />}
                    保存
                  </Button>
                </form>
                <div className="mt-5 space-y-2">
                  {playlists.map((playlist) => (
                    <div
                      key={playlist.id}
                      className="flex items-center gap-3 rounded-xl bg-white/5 p-3"
                    >
                      <span className="grid size-10 place-items-center rounded-xl bg-cyan-200/10 text-cyan-100">
                        <ListMusic className="size-4" />
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm text-white/85">{playlist.name}</p>
                        <p className="text-xs text-white/35">
                          {playlist.songCount} 首 · {formatDuration(playlist.durationSec * 1000)}
                        </p>
                      </div>
                    </div>
                  ))}
                  {!playlists.length ? <EmptyMessage text="还没有保存的歌单" /> : null}
                </div>
              </>
            ) : (
              <div className="flex min-h-72 flex-col items-center justify-center text-center">
                <span className="grid size-14 place-items-center rounded-2xl bg-white/5 text-cyan-100">
                  <LogIn />
                </span>
                <p className="mt-5 font-medium">登录后保存自己的歌单</p>
                <p className="mt-2 text-sm text-white/40">当前播放队列会继续保留</p>
                <Button className="mt-5 rounded-full px-6" onClick={() => requireAuth()}>
                  登录
                </Button>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}

function TrackList({
  tracks,
  currentId,
  onPlay,
}: {
  tracks: CatalogTrack[];
  currentId?: string;
  onPlay: (track: CatalogTrack) => void;
}) {
  return (
    <div className="space-y-1">
      {tracks.map((track) => (
        <button
          key={track.id}
          className={cn(
            "group flex w-full items-center gap-3 rounded-xl p-2 text-left transition hover:bg-white/5",
            currentId === track.id && "bg-cyan-200/8"
          )}
          onClick={() => onPlay(track)}
        >
          <CatalogArtwork
            mediaId={track.artworkMediaId ?? (track.hasArtwork ? track.mediaId : undefined)}
            alt={track.title}
            className="size-12 shrink-0 rounded-xl"
          />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm text-white/88">{track.title}</span>
            <span className="block truncate text-xs text-white/35">
              {track.artist} · {track.album}
            </span>
          </span>
          <span className="text-[11px] text-white/28">{formatDuration(track.durationMs)}</span>
          <span className="grid size-8 place-items-center rounded-full bg-white text-slate-950 opacity-0 transition group-hover:opacity-100 group-focus-visible:opacity-100">
            <Play className="size-3 fill-current" />
          </span>
        </button>
      ))}
      {!tracks.length ? <EmptyMessage text="没有找到匹配的音乐" /> : null}
    </div>
  );
}

function EmptyMessage({ text }: { text: string }) {
  return (
    <div className="flex min-h-48 flex-col items-center justify-center text-center text-white/32">
      <Clock3 className="mb-3 size-6" />
      <p className="text-sm">{text}</p>
    </div>
  );
}
