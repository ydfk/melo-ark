import { Disc3, Expand, Pause, Play, SkipBack, SkipForward, Volume2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { usePlayer } from "@/features/player/player-context";
import { formatDuration } from "@/lib/format";

export function ManagementMiniPlayer({ onOpenPlayer }: { onOpenPlayer: () => void }) {
  const player = usePlayer();

  return (
    <>
      <aside className="sticky top-6 hidden h-fit overflow-hidden rounded-3xl border bg-card/90 shadow-xl shadow-black/5 backdrop-blur-xl xl:block">
        <div className="relative aspect-square overflow-hidden bg-muted">
          {player.artworkUrl ? (
            <img
              src={player.artworkUrl}
              alt={player.current?.title ?? "当前播放封面"}
              className="size-full object-cover"
            />
          ) : (
            <div className="grid size-full place-items-center bg-[radial-gradient(circle_at_40%_35%,oklch(0.55_0.13_230/0.35),transparent_30%),linear-gradient(145deg,oklch(0.2_0.025_250),oklch(0.1_0.015_260))] text-white/50">
              <Disc3 className="size-24" />
            </div>
          )}
          <Button
            variant="secondary"
            size="icon"
            className="absolute right-3 top-3 bg-background/80 backdrop-blur"
            onClick={onOpenPlayer}
            aria-label="打开完整播放器"
          >
            <Expand />
          </Button>
        </div>
        <div className="space-y-5 p-5">
          {player.current ? (
            <>
              <div className="min-w-0">
                <p className="truncate font-display text-lg font-semibold">
                  {player.current.title}
                </p>
                <p className="mt-1 truncate text-sm text-muted-foreground">
                  {player.current.artist} · {player.current.album}
                </p>
              </div>
              <div>
                <Slider
                  value={[Math.min(player.position, player.duration || 0)]}
                  max={Math.max(player.duration, 1)}
                  step={1}
                  onValueChange={([value]) => player.seek(value)}
                  aria-label="播放进度"
                />
                <div className="mt-2 flex justify-between font-mono text-[11px] text-muted-foreground">
                  <span>{formatDuration(player.position * 1000)}</span>
                  <span>{formatDuration(player.duration * 1000)}</span>
                </div>
              </div>
              <div className="flex items-center justify-center gap-3">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={player.playPrevious}
                  aria-label="上一首"
                >
                  <SkipBack />
                </Button>
                <Button
                  size="icon"
                  className="size-12 rounded-full"
                  onClick={player.togglePlaying}
                  aria-label={player.playing ? "暂停" : "播放"}
                >
                  {player.playing ? <Pause /> : <Play className="translate-x-px" />}
                </Button>
                <Button variant="ghost" size="icon" onClick={player.playNext} aria-label="下一首">
                  <SkipForward />
                </Button>
              </div>
              <div className="flex items-center gap-3">
                <Volume2 className="size-4 text-muted-foreground" />
                <Slider
                  value={[player.volume]}
                  max={1}
                  step={0.01}
                  onValueChange={([value]) => player.setVolume(value)}
                  aria-label="音量"
                />
              </div>
            </>
          ) : (
            <div className="py-2 text-center">
              <p className="font-display font-semibold">还没有播放音乐</p>
              <p className="mt-2 text-sm text-muted-foreground">从播放中心选择一首歌曲。</p>
              <Button className="mt-5" onClick={onOpenPlayer}>
                <Disc3 />
                选择音乐
              </Button>
            </div>
          )}
        </div>
      </aside>

      <div className="fixed inset-x-3 bottom-3 z-40 flex items-center gap-3 rounded-2xl border bg-card/95 p-2.5 shadow-2xl backdrop-blur-xl xl:hidden">
        <button className="flex min-w-0 flex-1 items-center gap-3 text-left" onClick={onOpenPlayer}>
          {player.artworkUrl ? (
            <img src={player.artworkUrl} alt="" className="size-12 rounded-xl object-cover" />
          ) : (
            <span className="grid size-12 shrink-0 place-items-center rounded-xl bg-muted">
              <Disc3 className="size-6 text-muted-foreground" />
            </span>
          )}
          <span className="min-w-0">
            <span className="block truncate text-sm font-semibold">
              {player.current?.title ?? "选择音乐"}
            </span>
            <span className="block truncate text-xs text-muted-foreground">
              {player.current?.artist ?? "打开完整播放器"}
            </span>
          </span>
        </button>
        <Button
          variant="ghost"
          size="icon"
          disabled={!player.current}
          onClick={player.togglePlaying}
          aria-label={player.playing ? "暂停" : "播放"}
        >
          {player.playing ? <Pause /> : <Play />}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          disabled={!player.current}
          onClick={player.playNext}
          aria-label="下一首"
        >
          <SkipForward />
        </Button>
      </div>
    </>
  );
}
