import { History } from "lucide-react";
import { useEffect, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { getPlaybackHistory, getTrackOperations } from "@/lib/api/methods/library";
import type { PlaybackHistory, TrackOperationHistory } from "@/lib/api/types";
import { formatDate } from "@/lib/format";

export function TrackHistoryPanel({ trackId }: { trackId: string }) {
  const [items, setItems] = useState<PlaybackHistory[]>([]);
  const [operations, setOperations] = useState<TrackOperationHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setFailed(false);
    void Promise.all([getPlaybackHistory().send(), getTrackOperations(trackId).send()])
      .then(([history, operationHistory]) => {
        if (!active) return;
        setItems(history.filter((item) => item.trackId === trackId));
        setOperations(operationHistory);
      })
      .catch(() => active && setFailed(true))
      .finally(() => active && setLoading(false));
    return () => {
      active = false;
    };
  }, [trackId]);

  if (loading)
    return <p className="py-10 text-center text-sm text-muted-foreground">正在读取历史…</p>;
  if (failed) {
    return (
      <Alert variant="destructive">
        <History />
        <AlertTitle>历史加载失败</AlertTitle>
        <AlertDescription>请稍后重试，当前不会隐藏或改写已有记录。</AlertDescription>
      </Alert>
    );
  }
  if (!items.length && !operations.length) {
    return (
      <Alert>
        <History />
        <AlertTitle>还没有历史</AlertTitle>
        <AlertDescription>
          Tag、整理、回收站操作，以及 Web/OpenSubsonic 播放记录会显示在这里。
        </AlertDescription>
      </Alert>
    );
  }
  return (
    <div className="space-y-5">
      {operations.length ? (
        <section>
          <h3 className="mb-2 text-sm font-semibold">文件与元数据操作</h3>
          <div className="space-y-2">
            {operations.map((item) => (
              <div key={item.id} className="rounded-xl border bg-card/60 p-3">
                <div className="flex items-center justify-between gap-3">
                  <p className="text-sm font-medium">{operationLabel(item.kind, item.action)}</p>
                  <Badge variant={item.status === "failed" ? "destructive" : "secondary"}>
                    {item.status}
                  </Badge>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">{formatDate(item.updatedAt)}</p>
                {item.targetPath ? (
                  <p className="mt-2 truncate font-mono text-xs text-muted-foreground">
                    → {item.targetPath}
                  </p>
                ) : null}
                {item.errorMessage ? (
                  <p className="mt-2 text-xs text-destructive">{item.errorMessage}</p>
                ) : null}
              </div>
            ))}
          </div>
        </section>
      ) : null}
      {items.length ? (
        <section>
          <h3 className="mb-2 text-sm font-semibold">播放记录</h3>
          <div className="space-y-2">
            {items.map((item) => (
              <div
                key={item.id}
                className="flex items-center justify-between gap-3 rounded-xl border bg-card/60 p-3"
              >
                <div>
                  <p className="text-sm font-medium">{formatDate(item.playedAt)}</p>
                  <p className="mt-1 text-xs text-muted-foreground">{item.client}</p>
                </div>
                <Badge variant={item.completed ? "outline" : "secondary"}>
                  {item.completed ? "已完成" : "未播完"}
                </Badge>
              </div>
            ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}

function operationLabel(kind: string, action: string) {
  const kindLabels: Record<string, string> = {
    tag_edit: "Tag 写入",
    organize: "Hardlink 整理",
    trash: "回收站",
  };
  return `${kindLabels[kind] ?? kind} · ${action}`;
}
