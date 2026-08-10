import { describe, expect, test } from "vitest";

import { parseSseFrame } from "@/lib/api/events";

describe("SSE parser", () => {
  test("parses a job update without exposing auth in the URL", () => {
    const event = parseSseFrame(
      'event: job.updated\ndata: {"event":"job.updated","job":{"id":"job-1","status":"running"}}'
    );
    expect(event?.event).toBe("job.updated");
    expect(event?.job.status).toBe("running");
  });

  test("ignores keep-alive frames", () => {
    expect(parseSseFrame(": keep-alive")).toBeUndefined();
  });
});
