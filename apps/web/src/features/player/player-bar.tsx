import {
  Heart,
  ListMusic,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Trash2,
  Volume2,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Slider } from "@/components/ui/slider";
import { apiBaseURL } from "@/lib/api";
import {
  getFavorites,
  getLyrics,
  getPlayToken,
  scrobble,
  starTrack,
  unstarTrack,
} from "@/lib/api/methods/library";
import { formatDuration } from "@/lib/format";
import { usePlaybackStore } from "@/store/playback-store";

export function PlayerBar() {
  const audio = useRef<HTMLAudioElement>(null);
  const {
    queue,
    currentIndex,
    next,
    previous,
    remove,
    clear,
    shuffle,
    repeat,
    toggleShuffle,
    cycleRepeat,
  } = usePlaybackStore();
  const current = queue[currentIndex];
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState(0);
  const [position, setPosition] = useState(0);
  const [volume, setVolume] = useState(0.8);
  const [fallback, setFallback] = useState(false);
  const [favorites, setFavorites] = useState<Set<string>>(new Set());
  const [lyrics, setLyrics] = useState<Array<{ at: number; text: string }>>([]);

  useEffect(() => {
    getFavorites()
      .send()
      .then((ids) => setFavorites(new Set(ids)))
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!current) return;
    let cancelled = false;
    setFallback(false);
    setLyrics([]);
    Promise.all([getPlayToken(current.mediaId).send(), getLyrics(current.id).send()])
      .then(([ticket, lyricItems]) => {
        if (cancelled || !audio.current) return;
        const token = encodeURIComponent(ticket.token);
        audio.current.src = `${apiBaseURL()}/media/${current.mediaId}/stream?token=${token}`;
        setLyrics(
          parseLrc(lyricItems.find((item) => item.active)?.content ?? lyricItems[0]?.content ?? "")
        );
        void audio.current.play().catch(() => toast.info("点击播放按钮开始试听"));
        void scrobble({
          trackId: current.id,
          mediaFileId: current.mediaId,
          completed: false,
        }).send();
      })
      .catch(() => toast.error("无法获取播放凭据"));
    return () => {
      cancelled = true;
    };
  }, [current?.id, current?.mediaId]);

  const currentLyric = useMemo(
    () => [...lyrics].reverse().find((line) => line.at <= position)?.text,
    [lyrics, position]
  );
  if (!current) return null;

  async function toggleFavorite() {
    const starred = favorites.has(current.id);
    try {
      if (starred) await unstarTrack(current.id).send();
      else await starTrack(current.id).send();
      setFavorites((value) => {
        const next = new Set(value);
        if (starred) next.delete(current.id);
        else next.add(current.id);
        return next;
      });
    } catch {
      toast.error("收藏状态更新失败");
    }
  }

  return (
    <div className="fixed inset-x-0 bottom-0 z-50 border-t bg-background/95 px-3 py-2 shadow-2xl backdrop-blur-xl">
      <audio
        ref={audio}
        preload="metadata"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onLoadedMetadata={(event) => setDuration(event.currentTarget.duration || 0)}
        onTimeUpdate={(event) => setPosition(event.currentTarget.currentTime)}
        onEnded={() => {
          void scrobble({
            trackId: current.id,
            mediaFileId: current.mediaId,
            completed: true,
            positionSec: Math.round(duration),
          }).send();
          if (repeat === "one" && audio.current) {
            audio.current.currentTime = 0;
            void audio.current.play();
          } else next();
        }}
        onError={() => {
          if (!fallback && audio.current) {
            setFallback(true);
            const source = audio.current.src.replace("/stream?", "/transcode?profile=opus-192&");
            audio.current.src = source;
            void audio.current
              .play()
              .catch(() => toast.error("浏览器无法播放原始格式，且自动转码失败"));
          }
        }}
      />
      <div className="mx-auto grid max-w-7xl items-center gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(18rem,2fr)_minmax(0,1fr)]">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="truncate font-medium">{current.title}</p>
            {fallback ? <Badge variant="secondary">Opus 转码</Badge> : null}
          </div>
          <p className="truncate text-xs text-muted-foreground">
            {current.artist} · {current.album}
          </p>
          <p className="truncate text-xs text-primary">{currentLyric ?? " "}</p>
        </div>
        <div>
          <div className="flex items-center justify-center gap-1">
            <Button
              variant={shuffle ? "secondary" : "ghost"}
              size="icon"
              className="size-8"
              onClick={toggleShuffle}
              aria-label="随机播放"
            >
              <Shuffle />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              onClick={previous}
              aria-label="上一首"
            >
              <SkipBack />
            </Button>
            <Button
              size="icon"
              className="rounded-full"
              onClick={() => {
                if (!audio.current) return;
                if (playing) audio.current.pause();
                else void audio.current.play();
              }}
              aria-label={playing ? "暂停" : "播放"}
            >
              {playing ? <Pause /> : <Play />}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              onClick={next}
              aria-label="下一首"
            >
              <SkipForward />
            </Button>
            <Button
              variant={repeat !== "off" ? "secondary" : "ghost"}
              size="icon"
              className="size-8"
              onClick={cycleRepeat}
              aria-label="循环模式"
            >
              {repeat === "one" ? <Repeat1 /> : <Repeat />}
            </Button>
          </div>
          <div className="mt-1 flex items-center gap-2 text-[10px] text-muted-foreground">
            <span className="w-9 text-right">{formatDuration(position * 1000)}</span>
            <Slider
              value={[position]}
              max={Math.max(duration, 1)}
              step={1}
              onValueChange={([value]) => {
                setPosition(value);
                if (audio.current) audio.current.currentTime = value;
              }}
            />
            <span className="w-9">{formatDuration(duration * 1000)}</span>
          </div>
        </div>
        <div className="flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            onClick={() => void toggleFavorite()}
            aria-label="收藏"
          >
            <Heart className={favorites.has(current.id) ? "fill-current text-primary" : ""} />
          </Button>
          <Volume2 className="size-4 text-muted-foreground" />
          <Slider
            className="hidden max-w-24 sm:flex"
            value={[volume]}
            max={1}
            step={0.05}
            onValueChange={([value]) => {
              setVolume(value);
              if (audio.current) audio.current.volume = value;
            }}
          />
          <Popover>
            <PopoverTrigger asChild>
              <Button variant="ghost" size="icon" className="size-8" aria-label="播放队列">
                <ListMusic />
              </Button>
            </PopoverTrigger>
            <PopoverContent align="end" className="w-80">
              <div className="flex items-center justify-between">
                <strong className="text-sm">播放队列 · {queue.length}</strong>
                <Button variant="ghost" size="sm" onClick={clear}>
                  清空
                </Button>
              </div>
              <div className="mt-3 max-h-64 space-y-1 overflow-y-auto">
                {queue.map((track, index) => (
                  <div
                    key={`${track.id}-${index}`}
                    className={`flex items-center gap-2 rounded-lg px-2 py-2 text-sm ${index === currentIndex ? "bg-primary/10" : ""}`}
                  >
                    <button
                      className="min-w-0 flex-1 truncate text-left"
                      onClick={() => usePlaybackStore.getState().play(track)}
                    >
                      {track.title} · {track.artist}
                    </button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-8"
                      onClick={() => remove(index)}
                      aria-label="移出队列"
                    >
                      <Trash2 />
                    </Button>
                  </div>
                ))}
              </div>
            </PopoverContent>
          </Popover>
        </div>
      </div>
    </div>
  );
}

function parseLrc(content: string) {
  return content
    .split(/\r?\n/)
    .flatMap((line) => {
      const matches = [...line.matchAll(/\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g)];
      const text = line.replace(/\[[^\]]+\]/g, "").trim();
      return matches.map((match) => ({
        at: Number(match[1]) * 60 + Number(match[2]) + Number(`0.${match[3] ?? 0}`),
        text,
      }));
    })
    .filter((line) => line.text)
    .sort((a, b) => a.at - b.at);
}
