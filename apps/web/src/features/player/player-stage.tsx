import {
  Heart,
  ListMusic,
  Music2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
} from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

import { usePlayer } from "./player-context";

type PlayerStageProps = {
  onOpenCollection: () => void;
  favorite: boolean;
  onToggleFavorite: () => void;
};

export function PlayerStage({ onOpenCollection, favorite, onToggleFavorite }: PlayerStageProps) {
  const player = usePlayer();
  const [artworkFailed, setArtworkFailed] = useState(false);

  useEffect(() => setArtworkFailed(false), [player.artworkUrl]);

  if (!player.current) {
    return (
      <section className="player-stage player-stage-empty" aria-label="播放器">
        <div className="player-ambient player-ambient-empty" aria-hidden="true" />
        <div className="relative z-10 mx-auto flex max-w-2xl flex-col items-center text-center">
          <div className="empty-record" aria-hidden="true">
            <span className="empty-record-label">
              <Music2 className="size-9" />
            </span>
          </div>
          <p className="mt-8 text-xs font-semibold tracking-[0.3em] text-cyan-200/65 uppercase">
            Your music, in orbit
          </p>
          <h1 className="mt-3 font-display text-4xl font-semibold tracking-[-0.045em] text-white sm:text-6xl">
            让下一首歌转起来
          </h1>
          <p className="mt-4 max-w-lg text-sm leading-7 text-white/55 sm:text-base">
            从自己的曲库里挑一张唱片。MeloArk 不会自动播放，舞台等你落针。
          </p>
          <Button
            size="lg"
            className="mt-8 h-12 rounded-full bg-white px-7 text-slate-950 shadow-xl shadow-cyan-400/10 hover:bg-cyan-50"
            onClick={onOpenCollection}
          >
            <ListMusic />
            选择音乐
          </Button>
        </div>
      </section>
    );
  }

  const currentLine = player.currentLyricIndex;
  const visibleLyrics = player.lyrics.slice(Math.max(0, currentLine - 1), currentLine + 3);
  const lyricOffset = Math.max(0, currentLine - 1);

  return (
    <section className="player-stage" aria-label="正在播放">
      {player.artworkUrl && !artworkFailed ? (
        <div
          className="player-ambient"
          style={{ backgroundImage: `url(${JSON.stringify(player.artworkUrl)})` }}
          aria-hidden="true"
        />
      ) : (
        <div className="player-ambient player-ambient-empty" aria-hidden="true" />
      )}
      <div className="player-stage-grid">
        <div className="record-deck" aria-label={`${player.current.title} 唱片封面`}>
          <div className={cn("vinyl-record", player.playing && "is-playing")}>
            <div className="vinyl-grooves" aria-hidden="true" />
            <div className="vinyl-artwork">
              {player.artworkUrl && !artworkFailed ? (
                <img
                  src={player.artworkUrl}
                  alt={`${player.current.title} 封面`}
                  onError={() => setArtworkFailed(true)}
                />
              ) : (
                <Music2 className="size-12 text-white/40" aria-hidden="true" />
              )}
            </div>
            <span className="vinyl-spindle" aria-hidden="true" />
          </div>
          <div className={cn("tonearm", player.playing && "is-playing")} aria-hidden="true">
            <span className="tonearm-pivot" />
            <span className="tonearm-bar" />
            <span className="tonearm-head" />
          </div>
        </div>

        <div className="min-w-0 self-center">
          <div className="flex items-center gap-2 text-xs font-semibold tracking-[0.26em] text-cyan-200/65 uppercase">
            <span className="size-1.5 rounded-full bg-cyan-300 shadow-[0_0_16px_4px_oklch(0.8_0.14_205/0.5)]" />
            {player.loading ? "正在读取唱片" : player.playing ? "Now playing" : "Ready"}
          </div>
          <h1 className="mt-5 truncate font-display text-4xl font-semibold tracking-[-0.045em] text-white sm:text-5xl xl:text-6xl">
            {player.current.title}
          </h1>
          <p className="mt-3 truncate text-lg text-white/60">
            {player.current.artist}
            <span className="mx-2 text-white/25">—</span>
            {player.current.album}
          </p>

          <div className="mt-8 min-h-36 border-l border-white/10 pl-5" aria-live="polite">
            {visibleLyrics.length ? (
              visibleLyrics.map((line, index) => {
                const absoluteIndex = lyricOffset + index;
                return (
                  <p
                    key={`${line.at}-${line.text}`}
                    className={cn(
                      "py-1.5 text-base leading-7 transition-all duration-500 sm:text-lg",
                      absoluteIndex === currentLine
                        ? "translate-x-1 font-medium text-white"
                        : "text-white/28"
                    )}
                  >
                    {line.text}
                  </p>
                );
              })
            ) : (
              <p className="pt-2 text-base text-white/35">这首歌暂时没有可显示的歌词</p>
            )}
          </div>

          <div className="mt-6">
            <Slider
              value={[player.position]}
              max={Math.max(player.duration, 1)}
              step={1}
              onValueChange={([value]) => player.seek(value)}
              aria-label="播放进度"
              className="[&_[data-slot=slider-range]]:bg-cyan-200 [&_[data-slot=slider-thumb]]:border-cyan-100 [&_[data-slot=slider-thumb]]:bg-white"
            />
            <div className="mt-2 flex justify-between font-mono text-[11px] text-white/40">
              <span>{formatDuration(player.position * 1000)}</span>
              <span>{formatDuration(player.duration * 1000)}</span>
            </div>
          </div>

          <div className="mt-5 flex flex-wrap items-center justify-between gap-4">
            <div className="flex items-center gap-1 sm:gap-2">
              <StageButton label="随机播放" active={player.shuffle} onClick={player.toggleShuffle}>
                <Shuffle />
              </StageButton>
              <StageButton label="上一首" onClick={player.playPrevious}>
                <SkipBack />
              </StageButton>
              <Button
                size="icon"
                className="mx-1 size-14 rounded-full bg-white text-slate-950 shadow-xl shadow-black/30 hover:scale-105 hover:bg-cyan-50"
                onClick={player.togglePlaying}
                disabled={player.loading}
                aria-label={player.playing ? "暂停" : "播放"}
              >
                {player.playing ? (
                  <Pause className="size-6 fill-current" />
                ) : (
                  <Play className="size-6 fill-current" />
                )}
              </Button>
              <StageButton label="下一首" onClick={player.playNext}>
                <SkipForward />
              </StageButton>
              <StageButton
                label="循环模式"
                active={player.repeat !== "off"}
                onClick={player.cycleRepeat}
              >
                {player.repeat === "one" ? <Repeat1 /> : <Repeat />}
              </StageButton>
            </div>
            <div className="flex items-center gap-1">
              <StageButton label={favorite ? "取消收藏" : "收藏"} onClick={onToggleFavorite}>
                <Heart className={favorite ? "fill-cyan-200 text-cyan-200" : ""} />
              </StageButton>
              <Volume2 className="ml-2 size-4 text-white/45" />
              <Slider
                value={[player.volume]}
                max={1}
                step={0.05}
                onValueChange={([value]) => player.setVolume(value)}
                aria-label="音量"
                className="w-20 sm:w-24"
              />
              <StageButton label="播放队列" onClick={onOpenCollection}>
                <ListMusic />
              </StageButton>
            </div>
          </div>
          {player.fallback ? (
            <p className="mt-3 text-xs text-amber-200/70">正在使用 Opus 兼容转码</p>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function StageButton({
  label,
  active = false,
  onClick,
  children,
}: {
  label: string;
  active?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn(
        "size-10 rounded-full text-white/55 hover:bg-white/10 hover:text-white",
        active && "bg-cyan-200/12 text-cyan-100"
      )}
      onClick={onClick}
      aria-label={label}
    >
      {children}
    </Button>
  );
}
