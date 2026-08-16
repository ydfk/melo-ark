import { beforeEach, describe, expect, test } from "vitest";

import { usePlaybackStore, type QueueTrack } from "./playback-store";

describe("playback store", () => {
  beforeEach(() => {
    usePlaybackStore.setState({
      queue: [],
      currentIndex: -1,
      shuffle: false,
      repeat: "off",
    });
  });

  test("selects an exact organized file when one track has multiple media files", () => {
    const flac: QueueTrack = {
      id: "track-1",
      mediaId: "media-flac",
      title: "同一首歌",
      artist: "歌手",
      album: "专辑",
    };
    const wav: QueueTrack = { ...flac, mediaId: "media-wav" };

    usePlaybackStore.getState().play(wav, [flac, wav]);

    expect(usePlaybackStore.getState().queue).toEqual([flac, wav]);
    expect(usePlaybackStore.getState().currentIndex).toBe(1);
  });
});
