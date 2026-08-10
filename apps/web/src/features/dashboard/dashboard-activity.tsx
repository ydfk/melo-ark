import { Clock3, History, Play } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { CoverArtwork } from "@/features/library/cover-artwork";
import type { DashboardStats } from "@/lib/api/types";
import { formatDate } from "@/lib/format";
import { usePlaybackStore } from "@/store/playback-store";

export function DashboardActivity({ stats }: { stats?: DashboardStats }) {
  return (
    <section className="grid gap-4 lg:grid-cols-2">
      <Card>
        <CardHeader>
          <CardTitle>最近加入</CardTitle>
          <CardDescription>按首次进入 MeloArk 索引的时间排列。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          {stats?.recentAdded.length ? (
            stats.recentAdded.map((track) => (
              <div
                key={track.id}
                className="flex items-center gap-3 rounded-xl border bg-muted/20 p-2"
              >
                <CoverArtwork
                  mediaId={track.mediaId}
                  hasArtwork={track.hasArtwork}
                  alt={`${track.album}封面`}
                  className="size-11 shrink-0 rounded-lg"
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{track.title}</p>
                  <p className="truncate text-xs text-muted-foreground">
                    {track.artist} · {track.album}
                  </p>
                </div>
                <span className="hidden text-xs text-muted-foreground sm:block">
                  {formatDate(track.createdAt)}
                </span>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`播放 ${track.title}`}
                  onClick={() =>
                    usePlaybackStore.getState().play({
                      id: track.id,
                      mediaId: track.mediaId,
                      title: track.title,
                      artist: track.artist,
                      album: track.album,
                    })
                  }
                >
                  <Play />
                </Button>
              </div>
            ))
          ) : (
            <EmptyActivity icon={Clock3} text="首次扫描后，这里会显示最近加入的曲目。" />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>最近播放</CardTitle>
          <CardDescription>合并 Web 与 OpenSubsonic 客户端的 Scrobble。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          {stats?.recentPlayed.length ? (
            stats.recentPlayed.map((item, index) => (
              <div
                key={`${item.trackId}-${item.playedAt}-${index}`}
                className="flex items-center gap-3 rounded-xl border bg-muted/20 px-3 py-2.5"
              >
                <History className="size-4 shrink-0 text-primary" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{item.title}</p>
                  <p className="truncate text-xs text-muted-foreground">{item.artist}</p>
                </div>
                <div className="text-right">
                  <Badge variant="outline">{item.client}</Badge>
                  <p className="mt-1 text-xs text-muted-foreground">{formatDate(item.playedAt)}</p>
                </div>
              </div>
            ))
          ) : (
            <EmptyActivity icon={History} text="播放完成或客户端 Scrobble 后会留下记录。" />
          )}
        </CardContent>
      </Card>
    </section>
  );
}

function EmptyActivity({ icon: Icon, text }: { icon: typeof History; text: string }) {
  return (
    <div className="grid min-h-32 place-items-center rounded-xl border border-dashed text-center text-sm text-muted-foreground">
      <div>
        <Icon className="mx-auto mb-2 size-5" />
        <p>{text}</p>
      </div>
    </div>
  );
}
