import { describe, expect, test } from "vitest";

import { parseSseFrame } from "@/lib/api/events";

describe("SSE parser", () => {
  test("parses a job update without exposing auth in the URL", () => {
    const event = parseSseFrame(
      'event: job.updated\ndata: {"event":"job.updated","job":{"id":"job-1","status":"running"}}'
    );
    expect(event?.event).toBe("job.updated");
    expect(event?.event === "job.updated" ? event.job.status : undefined).toBe("running");
  });

  test("ignores keep-alive frames", () => {
    expect(parseSseFrame(": keep-alive")).toBeUndefined();
  });

  test("parses a structured job log event", () => {
    const event = parseSseFrame(
      'event: job.log\ndata: {"event":"job.log","log":{"id":4,"jobId":"job-1","level":"error","eventType":"failed","message":"读取失败","createdAt":"2026-08-12T10:00:00Z"}}'
    );
    expect(event?.event).toBe("job.log");
    expect(event?.event === "job.log" ? event.log.message : undefined).toBe("读取失败");
  });
});
