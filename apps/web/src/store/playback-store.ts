import { create } from "zustand";
import { persist } from "zustand/middleware";

export type QueueTrack = {
  id: string;
  mediaId: string;
  title: string;
  artist: string;
  album: string;
  durationMs?: number;
};

type RepeatMode = "off" | "all" | "one";

type PlaybackState = {
  queue: QueueTrack[];
  currentIndex: number;
  shuffle: boolean;
  repeat: RepeatMode;
  play: (track: QueueTrack, queue?: QueueTrack[]) => void;
  next: () => void;
  previous: () => void;
  remove: (index: number) => void;
  clear: () => void;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
};

export const usePlaybackStore = create<PlaybackState>()(
  persist(
    (set, get) => ({
      queue: [],
      currentIndex: -1,
      shuffle: false,
      repeat: "off",
      play: (track, queue) => {
        const nextQueue = queue?.length ? queue : get().queue;
        const existing = nextQueue.findIndex((item) => item.id === track.id);
        if (existing >= 0) set({ queue: nextQueue, currentIndex: existing });
        else set({ queue: [...nextQueue, track], currentIndex: nextQueue.length });
      },
      next: () => {
        const state = get();
        if (!state.queue.length) return;
        if (state.shuffle) set({ currentIndex: Math.floor(Math.random() * state.queue.length) });
        else if (state.currentIndex + 1 < state.queue.length)
          set({ currentIndex: state.currentIndex + 1 });
        else if (state.repeat === "all") set({ currentIndex: 0 });
      },
      previous: () => set((state) => ({ currentIndex: Math.max(0, state.currentIndex - 1) })),
      remove: (index) =>
        set((state) => ({
          queue: state.queue.filter((_, itemIndex) => itemIndex !== index),
          currentIndex:
            index < state.currentIndex
              ? state.currentIndex - 1
              : Math.min(state.currentIndex, state.queue.length - 2),
        })),
      clear: () => set({ queue: [], currentIndex: -1 }),
      toggleShuffle: () => set((state) => ({ shuffle: !state.shuffle })),
      cycleRepeat: () =>
        set((state) => ({
          repeat: state.repeat === "off" ? "all" : state.repeat === "all" ? "one" : "off",
        })),
    }),
    { name: "meloark-play-queue" }
  )
);
