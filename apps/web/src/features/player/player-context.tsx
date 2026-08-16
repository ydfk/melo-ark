import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";

import { useAuth } from "@/features/auth/auth-context";
import { apiBaseURL } from "@/lib/api";
import { getCatalogLyrics, getPublicPlayToken, type CatalogTrack } from "@/lib/api/methods/catalog";
import { scrobble } from "@/lib/api/methods/library";
import { usePlaybackStore, type QueueTrack } from "@/store/playback-store";

export type LyricLine = { at: number; text: string };

type PlayerContextValue = {
  current?: QueueTrack;
  queue: QueueTrack[];
  currentIndex: number;
  playing: boolean;
  loading: boolean;
  fallback: boolean;
  duration: number;
  position: number;
  volume: number;
  artworkUrl?: string;
  lyrics: LyricLine[];
  currentLyricIndex: number;
  shuffle: boolean;
  repeat: "off" | "all" | "one";
  playTrack: (track: CatalogTrack, tracks?: CatalogTrack[]) => void;
  playQueueItem: (index: number) => void;
  togglePlaying: () => void;
  playNext: () => void;
  playPrevious: () => void;
  seek: (position: number) => void;
  setVolume: (volume: number) => void;
  removeFromQueue: (index: number) => void;
  clearQueue: () => void;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
};

const PlayerContext = createContext<PlayerContextValue | null>(null);

export function PlayerProvider({ children }: { children: React.ReactNode }) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const autoplayNext = useRef(false);
  const startedTrack = useRef<string | undefined>(undefined);
  const { isAuthenticated } = useAuth();
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
  const [loading, setLoading] = useState(false);
  const [fallback, setFallback] = useState(false);
  const [duration, setDuration] = useState(0);
  const [position, setPosition] = useState(0);
  const [volume, setVolumeState] = useState(0.82);
  const [artworkUrl, setArtworkUrl] = useState<string>();
  const [lyrics, setLyrics] = useState<LyricLine[]>([]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.volume = volume;
  }, [volume]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!current || !audio) {
      setArtworkUrl(undefined);
      setLyrics([]);
      return;
    }

    let active = true;
    const shouldPlay = autoplayNext.current;
    autoplayNext.current = false;
    setLoading(true);
    setFallback(false);
    setPosition(0);
    setDuration(current.durationMs ? current.durationMs / 1000 : 0);
    setLyrics([]);
    startedTrack.current = undefined;

    void Promise.allSettled([
      getPublicPlayToken(current.mediaId).send(),
      getCatalogLyrics(current.id).send(),
    ]).then(async ([ticketResult, lyricsResult]) => {
      if (!active) return;
      if (lyricsResult.status === "fulfilled" && lyricsResult.value) {
        setLyrics(parseLyrics(lyricsResult.value.content));
      }
      if (ticketResult.status === "rejected") {
        setLoading(false);
        toast.error("无法获取播放凭据");
        return;
      }

      const token = encodeURIComponent(ticketResult.value.token);
      const base = apiBaseURL();
      audio.src = `${base}/media/${current.mediaId}/stream?token=${token}`;
      setArtworkUrl(`${base}/catalog/artwork/${current.mediaId}`);
      audio.load();
      setLoading(false);
      if (shouldPlay) {
        await audio.play().catch(() => toast.info("点击播放按钮开始试听"));
      }
    });

    return () => {
      active = false;
    };
  }, [current?.durationMs, current?.id, current?.mediaId]);

  const reportPlayback = useCallback(
    (completed: boolean) => {
      if (!isAuthenticated || !current) return;
      void scrobble({
        trackId: current.id,
        mediaFileId: current.mediaId,
        completed,
        positionSec: completed ? Math.round(duration) : undefined,
      })
        .send()
        .catch(() => undefined);
    },
    [current, duration, isAuthenticated]
  );

  const playNext = useCallback(() => {
    autoplayNext.current = true;
    next();
  }, [next]);

  const playPrevious = useCallback(() => {
    autoplayNext.current = true;
    previous();
  }, [previous]);

  const playTrack = useCallback(
    (track: CatalogTrack, tracks?: CatalogTrack[]) => {
      const target = toQueueTrack(track);
      const nextQueue = tracks?.map(toQueueTrack);
      if (current?.id === track.id && current.mediaId === track.mediaId && audioRef.current?.src) {
        void audioRef.current.play();
        return;
      }
      autoplayNext.current = true;
      usePlaybackStore.getState().play(target, nextQueue);
    },
    [current?.id, current?.mediaId]
  );

  const togglePlaying = useCallback(() => {
    const audio = audioRef.current;
    if (!audio || !current) return;
    if (audio.paused) void audio.play().catch(() => toast.error("暂时无法播放这首歌曲"));
    else audio.pause();
  }, [current]);

  const playQueueItem = useCallback(
    (index: number) => {
      const track = queue[index];
      if (!track) return;
      autoplayNext.current = true;
      usePlaybackStore.getState().play(track);
    },
    [queue]
  );

  const seek = useCallback((nextPosition: number) => {
    if (!audioRef.current) return;
    audioRef.current.currentTime = nextPosition;
    setPosition(nextPosition);
  }, []);

  const updateVolume = useCallback((nextVolume: number) => {
    setVolumeState(nextVolume);
    if (audioRef.current) audioRef.current.volume = nextVolume;
  }, []);

  const currentLyricIndex = useMemo(() => {
    for (let index = lyrics.length - 1; index >= 0; index -= 1) {
      if (lyrics[index].at <= position) return index;
    }
    return -1;
  }, [lyrics, position]);

  return (
    <PlayerContext.Provider
      value={{
        current,
        queue,
        currentIndex,
        playing,
        loading,
        fallback,
        duration,
        position,
        volume,
        artworkUrl,
        lyrics,
        currentLyricIndex,
        shuffle,
        repeat,
        playTrack,
        playQueueItem,
        togglePlaying,
        playNext,
        playPrevious,
        seek,
        setVolume: updateVolume,
        removeFromQueue: remove,
        clearQueue: clear,
        toggleShuffle,
        cycleRepeat,
      }}
    >
      {children}
      <audio
        ref={audioRef}
        preload="metadata"
        onPlay={() => {
          setPlaying(true);
          if (current && startedTrack.current !== current.id) {
            startedTrack.current = current.id;
            reportPlayback(false);
          }
        }}
        onPause={() => setPlaying(false)}
        onLoadedMetadata={(event) => setDuration(event.currentTarget.duration || 0)}
        onTimeUpdate={(event) => setPosition(event.currentTarget.currentTime)}
        onEnded={() => {
          setPlaying(false);
          reportPlayback(true);
          if (repeat === "one" && audioRef.current) {
            audioRef.current.currentTime = 0;
            void audioRef.current.play();
          } else {
            playNext();
          }
        }}
        onError={() => {
          const audio = audioRef.current;
          if (!audio || fallback || !audio.src.includes("/stream?")) return;
          setFallback(true);
          audio.src = audio.src.replace("/stream?", "/transcode?profile=opus-192&");
          void audio.play().catch(() => toast.error("原始格式与自动转码均无法播放"));
        }}
      />
    </PlayerContext.Provider>
  );
}

export function usePlayer() {
  const value = useContext(PlayerContext);
  if (!value) throw new Error("usePlayer 必须在 PlayerProvider 内使用");
  return value;
}

export function parseLyrics(content: string): LyricLine[] {
  const lines = content.split(/\r?\n/).flatMap((line, index) => {
    const matches = [...line.matchAll(/\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g)];
    const text = line.replace(/\[[^\]]+\]/g, "").trim();
    if (!matches.length) return text ? [{ at: index * 5, text }] : [];
    return matches.map((match) => ({
      at: Number(match[1]) * 60 + Number(match[2]) + Number(`0.${match[3] ?? 0}`),
      text,
    }));
  });
  return lines.filter((line) => line.text).sort((left, right) => left.at - right.at);
}

function toQueueTrack(track: CatalogTrack): QueueTrack {
  return {
    id: track.id,
    mediaId: track.mediaId,
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
  };
}
