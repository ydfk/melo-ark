import { apiBaseURL, getAccessToken } from "@/lib/api";
import type { JobEvent } from "@/lib/api/types";

export function parseSseFrame(frame: string): JobEvent | undefined {
  const data = frame
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(5).trimStart())
    .join("\n");
  if (!data) {
    return undefined;
  }
  try {
    return JSON.parse(data) as JobEvent;
  } catch {
    return undefined;
  }
}

export async function subscribeToJobEvents(
  signal: AbortSignal,
  onEvent: (event: JobEvent) => void
) {
  const token = getAccessToken();
  const response = await fetch(`${apiBaseURL()}/events`, {
    headers: token ? { Authorization: `Bearer ${token}` } : undefined,
    signal,
  });
  if (!response.ok || !response.body) {
    throw new Error("无法连接任务事件流");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  while (!signal.aborted) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, "\n");
    let boundary = buffer.indexOf("\n\n");
    while (boundary >= 0) {
      const event = parseSseFrame(buffer.slice(0, boundary));
      if (event) {
        onEvent(event);
      }
      buffer = buffer.slice(boundary + 2);
      boundary = buffer.indexOf("\n\n");
    }
  }
}
