import { Clock3, ListMusic, Plus, RadioTower, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  createPlaylist,
  deletePlaylist,
  getPlaybackHistory,
  getPlaylists,
} from "@/lib/api/methods/library";
import type { PlaybackHistory, Playlist } from "@/lib/api/types";
import { formatDate, formatDuration } from "@/lib/format";
import { usePlaybackStore } from "@/store/playback-store";

export function PlaybackPanel() {
  const queue = usePlaybackStore((state) => state.queue);
  const [history, setHistory] = useState<PlaybackHistory[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  async function refresh() {
    setLoading(true);
    try {
      const [nextHistory, nextPlaylists] = await Promise.all([
        getPlaybackHistory().send(),
        getPlaylists().send(),
      ]);
      setHistory(nextHistory);
      setPlaylists(nextPlaylists);
    } catch {
      toast.error("无法加载播放中心");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function saveQueue() {
    if (!name.trim() || !queue.length) return;
    setSaving(true);
    try {
      await createPlaylist({ name: name.trim(), trackIds: queue.map((track) => track.id) }).send();
      setName("");
      toast.success("当前队列已保存为歌单");
      await refresh();
    } catch {
      toast.error("保存歌单失败");
    } finally {
      setSaving(false);
    }
  }

  async function removePlaylist(id: string) {
    try {
      await deletePlaylist(id).send();
      setPlaylists((current) => current.filter((playlist) => playlist.id !== id));
      toast.success("歌单已删除，音乐文件未受影响");
    } catch {
      toast.error("删除歌单失败");
    }
  }

  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ListMusic />
            歌单
          </CardTitle>
          <CardDescription>将底部播放器中的当前队列保存为独立歌单。</CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="flex gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              void saveQueue();
            }}
          >
            <Input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder={
                queue.length ? `为当前 ${queue.length} 首曲目命名` : "先从曲库加入播放队列"
              }
              disabled={!queue.length}
              aria-label="歌单名称"
            />
            <Button disabled={!queue.length || !name.trim() || saving}>
              {saving ? <Spinner data-icon="inline-start" /> : <Plus data-icon="inline-start" />}
              保存
            </Button>
          </form>
          <div className="mt-4 space-y-2">
            {playlists.map((playlist) => (
              <div
                key={playlist.id}
                className="flex items-center gap-3 rounded-xl border bg-card/50 p-3"
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium">{playlist.name}</p>
                  <p className="text-xs text-muted-foreground">
                    {playlist.songCount} 首 · {formatDuration(playlist.durationSec * 1000)}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  onClick={() => void removePlaylist(playlist.id)}
                  aria-label={`删除歌单 ${playlist.name}`}
                >
                  <Trash2 />
                </Button>
              </div>
            ))}
            {!loading && !playlists.length ? (
              <Alert>
                <RadioTower />
                <AlertTitle>还没有歌单</AlertTitle>
                <AlertDescription>播放几首歌曲后，可以把队列保存到这里。</AlertDescription>
              </Alert>
            ) : null}
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Clock3 />
            最近播放
          </CardTitle>
          <CardDescription>记录 Web 与 OpenSubsonic 客户端的播放活动。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          {loading ? (
            <div className="flex justify-center py-10">
              <Spinner />
            </div>
          ) : null}
          {history.map((item) => (
            <div
              key={item.id}
              className="flex items-center gap-3 rounded-xl border-b px-2 py-3 last:border-b-0"
            >
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium">{item.title}</p>
                <p className="truncate text-xs text-muted-foreground">
                  {item.artist} · {formatDate(item.playedAt)}
                </p>
              </div>
              <Badge variant="secondary">{item.client}</Badge>
            </div>
          ))}
          {!loading && !history.length ? (
            <p className="py-10 text-center text-sm text-muted-foreground">
              播放记录会显示在这里。
            </p>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}
