import {
  ArrowRight,
  Headphones,
  Library,
  ListMusic,
  LogIn,
  LogOut,
  Play,
  RefreshCw,
  Settings2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { BrandMark } from "@/components/brand-mark";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useAuth } from "@/features/auth/auth-context";
import {
  getCatalogAlbums,
  getCatalogTracks,
  type CatalogAlbum,
  type CatalogTrack,
} from "@/lib/api/methods/catalog";
import { getFavorites, starTrack, unstarTrack } from "@/lib/api/methods/library";
import { formatDuration } from "@/lib/format";

import { CatalogArtwork } from "./catalog-artwork";
import { CollectionDrawer } from "./collection-drawer";
import { usePlayer } from "./player-context";
import { PlayerStage } from "./player-stage";

export function PlayerHome({ onOpenManagement }: { onOpenManagement: () => void }) {
  const { status, user, isAuthenticated, requireAuth, openLogin, logout } = useAuth();
  const player = usePlayer();
  const [tracks, setTracks] = useState<CatalogTrack[]>([]);
  const [albums, setAlbums] = useState<CatalogAlbum[]>([]);
  const [favorites, setFavorites] = useState<Set<string>>(new Set());
  const [collectionOpen, setCollectionOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string>();

  const loadCatalog = useCallback(async () => {
    setLoading(true);
    try {
      const [trackPage, albumItems] = await Promise.all([
        getCatalogTracks(1, 48, "").send(),
        getCatalogAlbums(24).send(),
      ]);
      setTracks(trackPage.items);
      setAlbums(albumItems);
      setLoadError(undefined);
    } catch {
      setLoadError("暂时无法读取音乐目录");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  useEffect(() => {
    if (!isAuthenticated) {
      setFavorites(new Set());
      return;
    }
    void getFavorites()
      .send()
      .then((ids) => setFavorites(new Set(ids)))
      .catch(() => undefined);
  }, [isAuthenticated]);

  function openManagement() {
    requireAuth(onOpenManagement);
  }

  function toggleFavorite() {
    const track = player.current;
    if (!track) return;
    requireAuth(async () => {
      const starred = favorites.has(track.id);
      try {
        if (starred) await unstarTrack(track.id).send();
        else await starTrack(track.id).send();
        setFavorites((current) => {
          const next = new Set(current);
          if (starred) next.delete(track.id);
          else next.add(track.id);
          return next;
        });
        toast.success(starred ? "已取消收藏" : "已加入收藏");
      } catch {
        toast.error("收藏状态更新失败");
      }
    });
  }

  return (
    <main className="player-home min-h-[100dvh] overflow-hidden text-white">
      <header className="relative z-30 mx-auto flex w-full max-w-[1800px] items-center justify-between gap-3 px-4 py-4 sm:px-8 sm:py-6">
        <button
          className="flex items-center gap-3 rounded-xl text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-200"
          onClick={() => window.scrollTo({ top: 0, behavior: "smooth" })}
          aria-label="返回播放器顶部"
        >
          <BrandMark className="size-10 rounded-full bg-white text-slate-950 shadow-cyan-300/15" />
          <span>
            <span className="block font-display text-base font-semibold tracking-wide">
              MeloArk
            </span>
            <span className="hidden text-[10px] tracking-[0.22em] text-white/35 uppercase sm:block">
              Personal listening room
            </span>
          </span>
        </button>

        <nav
          className="absolute left-1/2 flex -translate-x-1/2 rounded-full border border-white/10 bg-black/25 p-1 shadow-xl shadow-black/20 backdrop-blur-xl"
          aria-label="模式切换"
        >
          <Button
            className="h-9 rounded-full bg-white px-4 text-slate-950 hover:bg-white"
            size="sm"
          >
            <Headphones />
            播放
          </Button>
          <Button
            variant="ghost"
            className="h-9 rounded-full px-4 text-white/50 hover:bg-white/8 hover:text-white"
            size="sm"
            onClick={openManagement}
          >
            <Settings2 />
            管理
          </Button>
        </nav>

        <div className="flex items-center gap-1.5">
          <Button
            variant="ghost"
            size="icon"
            className="rounded-full text-white/55 hover:bg-white/10 hover:text-white sm:hidden"
            onClick={() => setCollectionOpen(true)}
            aria-label="打开播放收藏"
          >
            <ListMusic />
          </Button>
          {isAuthenticated ? (
            <>
              <Button
                variant="ghost"
                className="hidden rounded-full text-white/65 hover:bg-white/10 hover:text-white sm:inline-flex"
                onClick={openManagement}
              >
                <span className="size-2 rounded-full bg-emerald-300 shadow-[0_0_10px_oklch(0.8_0.17_155)]" />
                {user?.username}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="rounded-full text-white/45 hover:bg-white/10 hover:text-white"
                onClick={logout}
                aria-label="退出登录"
              >
                <LogOut />
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              className="rounded-full text-white/65 hover:bg-white/10 hover:text-white"
              onClick={openLogin}
              disabled={status === "loading"}
            >
              <LogIn />
              <span className="hidden sm:inline">登录</span>
            </Button>
          )}
        </div>
      </header>

      <div className="relative z-10 mx-auto max-w-[1800px] px-3 pb-14 sm:px-8 sm:pb-20">
        <PlayerStage
          onOpenCollection={() => setCollectionOpen(true)}
          favorite={Boolean(player.current && favorites.has(player.current.id))}
          onToggleFavorite={toggleFavorite}
        />

        <section className="mx-auto mt-8 max-w-[1560px] sm:mt-12" aria-labelledby="featured-title">
          <div className="flex items-end justify-between gap-4 px-1">
            <div>
              <p className="text-[10px] font-semibold tracking-[0.28em] text-cyan-100/50 uppercase">
                From your library
              </p>
              <h2
                id="featured-title"
                className="mt-2 font-display text-2xl font-semibold tracking-tight sm:text-3xl"
              >
                曲库精选
              </h2>
            </div>
            <Button
              variant="ghost"
              className="rounded-full text-white/50 hover:bg-white/8 hover:text-white"
              onClick={() => setCollectionOpen(true)}
            >
              浏览全部
              <ArrowRight />
            </Button>
          </div>

          {loadError ? (
            <div className="mt-5 flex items-center justify-between gap-4 rounded-2xl border border-red-300/10 bg-red-300/5 px-5 py-4 text-sm text-red-100/70">
              <span>{loadError}</span>
              <Button variant="ghost" size="sm" onClick={() => void loadCatalog()}>
                <RefreshCw />
                重试
              </Button>
            </div>
          ) : null}

          {loading ? (
            <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
              {Array.from({ length: 6 }, (_, index) => (
                <div key={index}>
                  <Skeleton className="aspect-square rounded-2xl bg-white/7" />
                  <Skeleton className="mt-3 h-3 w-4/5 bg-white/7" />
                  <Skeleton className="mt-2 h-2.5 w-1/2 bg-white/5" />
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-5 grid grid-cols-2 gap-x-3 gap-y-6 sm:grid-cols-3 lg:grid-cols-6">
              {tracks.slice(0, 12).map((track) => (
                <FeaturedTrack
                  key={track.id}
                  track={track}
                  active={player.current?.id === track.id}
                  onPlay={() => player.playTrack(track, tracks)}
                />
              ))}
            </div>
          )}
          {!loading && !loadError && !tracks.length ? (
            <div className="mt-5 flex min-h-52 flex-col items-center justify-center rounded-3xl border border-dashed border-white/10 bg-white/[0.025] text-center">
              <Library className="size-7 text-white/25" />
              <p className="mt-4 text-sm text-white/45">曲库里还没有可播放的音乐</p>
              <Button variant="ghost" className="mt-2 text-cyan-100/65" onClick={openManagement}>
                前往管理曲库
              </Button>
            </div>
          ) : null}
        </section>
      </div>

      <CollectionDrawer
        open={collectionOpen}
        onOpenChange={setCollectionOpen}
        featured={tracks}
        albums={albums}
      />
    </main>
  );
}

function FeaturedTrack({
  track,
  active,
  onPlay,
}: {
  track: CatalogTrack;
  active: boolean;
  onPlay: () => void;
}) {
  return (
    <button className="featured-track group min-w-0 text-left" onClick={onPlay}>
      <span className="relative block overflow-hidden rounded-2xl">
        <CatalogArtwork
          mediaId={track.artworkMediaId ?? (track.hasArtwork ? track.mediaId : undefined)}
          alt={track.title}
          className="aspect-square w-full transition duration-500 group-hover:scale-[1.04]"
        />
        <span className="absolute inset-0 bg-gradient-to-t from-black/50 via-transparent to-transparent opacity-0 transition group-hover:opacity-100" />
        <span className="absolute right-3 bottom-3 grid size-10 translate-y-2 place-items-center rounded-full bg-white text-slate-950 opacity-0 shadow-xl transition group-hover:translate-y-0 group-hover:opacity-100 group-focus-visible:translate-y-0 group-focus-visible:opacity-100">
          <Play className="size-4 fill-current" />
        </span>
        {active ? (
          <span className="absolute top-3 left-3 rounded-full bg-black/55 px-2.5 py-1 text-[10px] font-medium text-cyan-100 backdrop-blur-md">
            正在播放
          </span>
        ) : null}
      </span>
      <span className="mt-3 block truncate text-sm font-medium text-white/88">{track.title}</span>
      <span className="mt-1 flex min-w-0 items-center justify-between gap-2 text-xs text-white/35">
        <span className="truncate">{track.artist}</span>
        <span className="shrink-0">{formatDuration(track.durationMs)}</span>
      </span>
    </button>
  );
}
