import { describe, expect, test } from "vitest";

import { parseLyrics } from "./player-context";

describe("parseLyrics", () => {
  test("解析多时间戳 LRC 并按时间排序", () => {
    expect(parseLyrics("[00:10.50][00:20.00]同一句\n[00:05.00]开场")).toEqual([
      { at: 5, text: "开场" },
      { at: 10.5, text: "同一句" },
      { at: 20, text: "同一句" },
    ]);
  });
});
