import {
  Captions,
  Columns3,
  Grid2X2,
  ImageOff,
  List,
  Play,
  Search,
  Tags,
  WandSparkles,
} from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { CoverArtwork } from "@/features/library/cover-artwork";
import type { ManagedMediaFile, ManagedMediaFilePage, TrackFilter } from "@/lib/api/types";
import { formatBytes, formatDuration } from "@/lib/format";

type ColumnKey =
  | "cover"
  | "title"
  | "artist"
  | "album"
  | "year"
  | "format"
  | "quality"
  | "duration"
  | "size"
  | "lyrics"
  | "tags"
  | "path";

const columns: Array<{ key: ColumnKey; label: string }> = [
  { key: "cover", label: "封面" },
  { key: "title", label: "标题" },
  { key: "artist", label: "歌手" },
  { key: "album", label: "专辑" },
  { key: "year", label: "年份" },
  { key: "format", label: "格式" },
  { key: "quality", label: "质量" },
  { key: "duration", label: "时长" },
  { key: "size", label: "大小" },
  { key: "lyrics", label: "歌词" },
  { key: "tags", label: "Tag 健康" },
  { key: "path", label: "路径" },
];

const defaultColumns = new Set<ColumnKey>([
  "cover",
  "title",
  "artist",
  "album",
  "year",
  "format",
  "quality",
  "duration",
  "size",
  "lyrics",
  "tags",
]);

const filters: Array<{ value: TrackFilter; label: string }> = [
  { value: "missing_tags", label: "缺失 Tag" },
  { value: "missing_lyrics", label: "缺失歌词" },
  { value: "missing_cover", label: "缺失封面" },
  { value: "duplicates", label: "重复组成员" },
];

type TrackCatalogProps = {
  tracks?: ManagedMediaFilePage;
  loading: boolean;
  search: string;
  filter?: TrackFilter;
  page: number;
  perPage: number;
  selected: Set<string>;
  onSearchChange: (value: string) => void;
  onSearch: () => void;
  onFilterChange: (value?: TrackFilter) => void;
  onPageChange: (page: number) => void;
  onPerPageChange: (perPage: number) => void;
  onSelectionChange: (value: Set<string>) => void;
  onOpenTrack: (id: string) => void;
  onPlayTrack: (id: string) => void;
  onBatchTag: () => void;
  onBatchScrape: () => void;
};

export function TrackCatalog(props: TrackCatalogProps) {
  const [view, setView] = useState<"table" | "albums">("table");
  const [visibleColumns, setVisibleColumns] = useState(() => new Set(defaultColumns));
  const albumGroups = useMemo(() => groupAlbums(props.tracks?.items ?? []), [props.tracks?.items]);
  const totalPages = Math.max(1, Math.ceil((props.tracks?.total ?? 0) / props.perPage));

  function toggleColumn(key: ColumnKey, checked: boolean) {
    setVisibleColumns((current) => {
      const next = new Set(current);
      if (checked) next.add(key);
      else next.delete(key);
      return next;
    });
  }

  return (
    <Card>
      <CardHeader className="gap-4">
        <div className="flex flex-col justify-between gap-4 lg:flex-row lg:items-start">
          <div>
            <CardTitle>歌曲列表</CardTitle>
          </div>
          <div className="flex flex-wrap gap-2">
            <ToggleGroup
              type="single"
              value={view}
              onValueChange={(value) => value && setView(value as typeof view)}
              variant="outline"
              aria-label="曲库视图"
            >
              <ToggleGroupItem value="table" aria-label="表格视图">
                <List />
                表格
              </ToggleGroupItem>
              <ToggleGroupItem value="albums" aria-label="专辑网格视图">
                <Grid2X2 />
                专辑
              </ToggleGroupItem>
            </ToggleGroup>
            {view === "table" ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline">
                    <Columns3 data-icon="inline-start" />列
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-44">
                  <DropdownMenuLabel>显示列</DropdownMenuLabel>
                  {columns.map((column) => (
                    <DropdownMenuCheckboxItem
                      key={column.key}
                      checked={visibleColumns.has(column.key)}
                      onCheckedChange={(checked) => toggleColumn(column.key, checked)}
                    >
                      {column.label}
                    </DropdownMenuCheckboxItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            ) : null}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {props.selected.size ? (
          <div className="mb-4 flex flex-col justify-between gap-3 rounded-xl border bg-primary/5 px-4 py-3 sm:flex-row sm:items-center">
            <span className="text-sm">已选择 {props.selected.size} 个整理文件</span>
            <div className="flex flex-wrap gap-2">
              <Button variant="ghost" size="sm" onClick={() => props.onSelectionChange(new Set())}>
                清除
              </Button>
              <Button size="sm" onClick={props.onBatchTag}>
                <Tags data-icon="inline-start" />
                批量 Tag
              </Button>
              <Button variant="secondary" size="sm" onClick={props.onBatchScrape}>
                <WandSparkles data-icon="inline-start" />
                批量刮削
              </Button>
            </div>
          </div>
        ) : null}

        <form
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            props.onSearch();
          }}
        >
          <div className="flex gap-2">
            <Input
              value={props.search}
              onChange={(event) => props.onSearchChange(event.target.value)}
              placeholder="搜索歌曲、歌手、专辑或路径"
              aria-label="搜索曲库"
            />
            <Button type="submit" variant="secondary">
              <Search data-icon="inline-start" />
              搜索
            </Button>
          </div>
          <ToggleGroup
            type="single"
            value={props.filter ?? "all"}
            onValueChange={(value) =>
              props.onFilterChange(value && value !== "all" ? (value as TrackFilter) : undefined)
            }
            className="justify-start overflow-x-auto pb-1"
            aria-label="快捷筛选"
          >
            <ToggleGroupItem value="all">全部</ToggleGroupItem>
            {filters.map((item) => (
              <ToggleGroupItem key={item.value} value={item.value}>
                {item.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </form>

        <div className="mt-5">
          {props.loading ? (
            <CatalogSkeleton view={view} />
          ) : view === "table" ? (
            <TrackTable {...props} visibleColumns={visibleColumns} />
          ) : (
            <AlbumGrid
              groups={albumGroups}
              selected={props.selected}
              onSelectionChange={props.onSelectionChange}
              onOpenTrack={props.onOpenTrack}
              onPlayTrack={props.onPlayTrack}
            />
          )}
        </div>

        <div className="mt-4 flex flex-col gap-3 text-sm text-muted-foreground lg:flex-row lg:items-center lg:justify-between">
          <div className="flex flex-wrap items-center gap-3">
            <span>共 {props.tracks?.total ?? 0} 个整理文件</span>
            <span>
              第 {props.page} / {totalPages} 页
            </span>
            <Select
              value={String(props.perPage)}
              onValueChange={(value) => props.onPerPageChange(Number(value))}
            >
              <SelectTrigger className="h-8 w-[128px]" aria-label="每页整理文件数量">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[25, 50, 100].map((value) => (
                  <SelectItem key={value} value={String(value)}>
                    每页 {value} 个文件
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={props.page <= 1}
              onClick={() => props.onPageChange(1)}
            >
              首页
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={props.page <= 1}
              onClick={() => props.onPageChange(props.page - 1)}
            >
              上一页
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={props.page >= totalPages}
              onClick={() => props.onPageChange(props.page + 1)}
            >
              下一页
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={props.page >= totalPages}
              onClick={() => props.onPageChange(totalPages)}
            >
              末页
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function TrackTable(
  props: TrackCatalogProps & {
    visibleColumns: Set<ColumnKey>;
  }
) {
  const items = props.tracks?.items ?? [];
  const visible = props.visibleColumns;
  return (
    <div className="overflow-x-auto rounded-xl border">
      <Table className="min-w-max">
        <TableHeader>
          <TableRow>
            <TableHead className="w-10">
              <Checkbox
                aria-label="选择当前页全部整理文件"
                checked={
                  Boolean(items.length) && items.every((track) => props.selected.has(track.mediaId))
                }
                onCheckedChange={(checked) =>
                  props.onSelectionChange(
                    checked ? new Set(items.map((track) => track.mediaId)) : new Set()
                  )
                }
              />
            </TableHead>
            <TableHead className="w-10">
              <span className="sr-only">播放</span>
            </TableHead>
            {columns.map((column) =>
              visible.has(column.key) ? (
                <TableHead key={column.key}>{column.label}</TableHead>
              ) : null
            )}
          </TableRow>
        </TableHeader>
        <TableBody>
          {items.map((track) => (
            <TableRow key={track.mediaId} className="content-auto">
              <TableCell>
                <Checkbox
                  aria-label={`选择 ${track.title}`}
                  checked={props.selected.has(track.mediaId)}
                  onCheckedChange={(checked) => {
                    const next = new Set(props.selected);
                    if (checked) next.add(track.mediaId);
                    else next.delete(track.mediaId);
                    props.onSelectionChange(next);
                  }}
                />
              </TableCell>
              <TableCell>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  onClick={() => props.onPlayTrack(track.mediaId)}
                  aria-label={`播放 ${track.title}`}
                >
                  <Play />
                </Button>
              </TableCell>
              {visible.has("cover") ? (
                <TableCell>
                  <CoverArtwork
                    mediaId={track.mediaId}
                    hasArtwork={track.hasArtwork}
                    alt={`${track.album}封面`}
                    className="size-10 rounded-md"
                  />
                </TableCell>
              ) : null}
              {visible.has("title") ? (
                <TableCell className="max-w-64">
                  <Button
                    variant="link"
                    className="h-auto max-w-full justify-start truncate p-0 text-foreground"
                    onClick={() => props.onOpenTrack(track.trackId)}
                  >
                    {track.title}
                  </Button>
                </TableCell>
              ) : null}
              {visible.has("artist") ? <TableCell>{track.artist}</TableCell> : null}
              {visible.has("album") ? <TableCell>{track.album}</TableCell> : null}
              {visible.has("year") ? <TableCell>{track.year ?? "—"}</TableCell> : null}
              {visible.has("format") ? (
                <TableCell>
                  <Badge variant="secondary">{track.extension.toUpperCase()}</Badge>
                </TableCell>
              ) : null}
              {visible.has("quality") ? (
                <TableCell className="font-mono">{track.qualityScore ?? "—"}</TableCell>
              ) : null}
              {visible.has("duration") ? (
                <TableCell className="font-mono">{formatDuration(track.durationMs)}</TableCell>
              ) : null}
              {visible.has("size") ? (
                <TableCell className="font-mono">{formatBytes(track.fileSize)}</TableCell>
              ) : null}
              {visible.has("lyrics") ? (
                <TableCell>
                  {track.hasLyrics ? (
                    <Badge variant="outline">
                      <Captions />
                      有歌词
                    </Badge>
                  ) : (
                    <Badge variant="secondary">缺失</Badge>
                  )}
                </TableCell>
              ) : null}
              {visible.has("tags") ? (
                <TableCell>
                  <Badge variant={track.tagHealth === "complete" ? "outline" : "destructive"}>
                    {track.tagHealth === "complete" ? "完整" : "待补全"}
                  </Badge>
                </TableCell>
              ) : null}
              {visible.has("path") ? (
                <TableCell
                  className="max-w-80 truncate font-mono text-xs text-muted-foreground"
                  title={track.path}
                >
                  {track.path}
                </TableCell>
              ) : null}
            </TableRow>
          ))}
          {!items.length ? (
            <TableRow>
              <TableCell
                colSpan={columns.length + 2}
                className="h-28 text-center text-muted-foreground"
              >
                当前搜索或筛选没有整理文件。
              </TableCell>
            </TableRow>
          ) : null}
        </TableBody>
      </Table>
    </div>
  );
}

type AlbumGroup = {
  key: string;
  title: string;
  artist: string;
  year?: number;
  tracks: ManagedMediaFile[];
  formats: string[];
};

function AlbumGrid({
  groups,
  selected,
  onSelectionChange,
  onOpenTrack,
  onPlayTrack,
}: {
  groups: AlbumGroup[];
  selected: Set<string>;
  onSelectionChange: (value: Set<string>) => void;
  onOpenTrack: (id: string) => void;
  onPlayTrack: (id: string) => void;
}) {
  if (!groups.length)
    return (
      <p className="py-16 text-center text-sm text-muted-foreground">当前搜索或筛选没有专辑。</p>
    );
  return (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {groups.map((group) => {
        const lead = group.tracks[0];
        const selectedAll = group.tracks.every((track) => selected.has(track.mediaId));
        return (
          <article
            key={group.key}
            className="group overflow-hidden rounded-2xl border bg-card/70 transition-transform hover:-translate-y-0.5"
          >
            <div className="relative aspect-square">
              <CoverArtwork
                mediaId={lead.mediaId}
                hasArtwork={lead.hasArtwork}
                alt={`${group.title}封面`}
                className="size-full rounded-none"
              />
              <div className="absolute inset-x-3 top-3 flex justify-between">
                <Checkbox
                  aria-label={`选择专辑 ${group.title}`}
                  checked={selectedAll}
                  onCheckedChange={(checked) => {
                    const next = new Set(selected);
                    for (const track of group.tracks) {
                      if (checked) next.add(track.mediaId);
                      else next.delete(track.mediaId);
                    }
                    onSelectionChange(next);
                  }}
                  className="border-white/70 bg-black/40"
                />
                {!lead.hasArtwork ? (
                  <Badge variant="secondary">
                    <ImageOff />
                    无封面
                  </Badge>
                ) : null}
              </div>
              <Button
                size="icon"
                className="absolute bottom-3 right-3 rounded-full opacity-0 shadow-lg transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                onClick={() => onPlayTrack(lead.mediaId)}
                aria-label={`播放专辑 ${group.title}`}
              >
                <Play />
              </Button>
            </div>
            <div className="p-4">
              <Button
                variant="link"
                className="h-auto max-w-full justify-start truncate p-0 text-base font-semibold text-foreground"
                onClick={() => onOpenTrack(lead.trackId)}
              >
                {group.title}
              </Button>
              <p className="mt-1 truncate text-sm text-muted-foreground">{group.artist}</p>
              <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span>{group.year ?? "年份未知"}</span>
                <span>·</span>
                <span>{group.tracks.length} 个文件</span>
                {group.formats.map((format) => (
                  <Badge key={format} variant="outline">
                    {format}
                  </Badge>
                ))}
              </div>
            </div>
          </article>
        );
      })}
    </div>
  );
}

function CatalogSkeleton({ view }: { view: "table" | "albums" }) {
  return view === "albums" ? (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
      {Array.from({ length: 4 }, (_, index) => (
        <Skeleton key={index} className="aspect-[4/5] rounded-2xl" />
      ))}
    </div>
  ) : (
    <div className="space-y-2 rounded-xl border p-3">
      {Array.from({ length: 6 }, (_, index) => (
        <Skeleton key={index} className="h-12 w-full" />
      ))}
    </div>
  );
}

function groupAlbums(tracks: ManagedMediaFile[]): AlbumGroup[] {
  const groups = new Map<string, AlbumGroup>();
  for (const track of tracks) {
    const key = `${track.artist}\u0000${track.album}`;
    const current = groups.get(key);
    if (current) {
      current.tracks.push(track);
      if (!current.formats.includes(track.extension.toUpperCase()))
        current.formats.push(track.extension.toUpperCase());
    } else {
      groups.set(key, {
        key,
        title: track.album,
        artist: track.artist,
        year: track.year,
        tracks: [track],
        formats: [track.extension.toUpperCase()],
      });
    }
  }
  return [...groups.values()];
}
