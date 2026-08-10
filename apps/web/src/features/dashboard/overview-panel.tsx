import {
  Activity,
  Captions,
  CircleHelp,
  CopyCheck,
  Disc3,
  FileAudio,
  HardDrive,
  ImageOff,
  Music2,
  ShieldCheck,
  Tags,
  Users,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DashboardActivity } from "@/features/dashboard/dashboard-activity";
import { formatBytes, formatDate } from "@/lib/format";
import type { DashboardStats, Job, LibraryRoot } from "@/lib/api/types";

type OverviewPanelProps = {
  stats?: DashboardStats;
  libraries: LibraryRoot[];
  jobs: Job[];
};

export function OverviewPanel({ stats, libraries, jobs }: OverviewPanelProps) {
  const activeJob = jobs.find((job) => ["queued", "running", "paused"].includes(job.status));
  const values = [
    { label: "逻辑曲目", value: stats?.trackCount.toLocaleString("zh-CN") ?? "—", icon: Music2 },
    { label: "艺术家", value: stats?.artistCount.toLocaleString("zh-CN") ?? "—", icon: Users },
    { label: "专辑", value: stats?.albumCount.toLocaleString("zh-CN") ?? "—", icon: Disc3 },
    {
      label: "物理文件",
      value: stats?.mediaFileCount.toLocaleString("zh-CN") ?? "—",
      icon: FileAudio,
    },
    { label: "媒体容量", value: formatBytes(stats?.totalBytes ?? 0), icon: HardDrive },
    { label: "缺失标题", value: stats?.missingTagCount.toLocaleString("zh-CN") ?? "—", icon: Tags },
    {
      label: "缺失歌词",
      value: stats?.missingLyricsCount.toLocaleString("zh-CN") ?? "—",
      icon: Captions,
    },
    {
      label: "缺失封面",
      value: stats?.missingCoverCount.toLocaleString("zh-CN") ?? "—",
      icon: ImageOff,
    },
    {
      label: "Exact Duplicate",
      value: stats?.exactDuplicateCount.toLocaleString("zh-CN") ?? "—",
      icon: CopyCheck,
    },
    {
      label: "疑似重复",
      value: stats?.possibleDuplicateCount.toLocaleString("zh-CN") ?? "—",
      icon: CircleHelp,
    },
  ];

  return (
    <div className="flex flex-col gap-6">
      <section className="hero-deck relative overflow-hidden rounded-3xl border p-7 sm:p-10">
        <div className="relative z-10 max-w-2xl">
          <Badge variant="secondary">
            <Activity />
            {activeJob ? `正在处理：${activeJob.currentItem ?? activeJob.kind}` : "本地服务在线"}
          </Badge>
          <p className="mt-7 font-mono text-xs uppercase tracking-[0.24em] text-primary">
            Music + Management Dashboard
          </p>
          <h1 className="mt-3 font-display text-4xl font-semibold tracking-[-0.04em] sm:text-5xl">
            {libraries.length ? "曲库状态一目了然。" : "曲库舱已就绪。"}
          </h1>
          <p className="mt-4 max-w-xl leading-7 text-muted-foreground">
            {libraries.length
              ? `已连接 ${libraries.length} 个 Library Root。基础扫描只读取发生变化的文件，不会在每次启动时 Hash 或 Fingerprint 整个曲库。`
              : "添加 NAS 中已经挂载到容器的音乐目录。MeloArk 不会下载音乐，也不会未经确认删除源文件。"}
          </p>
        </div>
        <div className="record-groove" aria-hidden="true" />
      </section>

      <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
        {values.map(({ label, value, icon: Icon }) => (
          <Card key={label}>
            <CardHeader>
              <CardDescription className="flex items-center gap-2">
                <Icon aria-hidden="true" />
                {label}
              </CardDescription>
              <CardTitle className="font-mono text-3xl">{value}</CardTitle>
            </CardHeader>
          </Card>
        ))}
      </section>

      <section className="grid gap-4 lg:grid-cols-[2fr_1fr]">
        <Card>
          <CardHeader>
            <CardTitle>格式分布</CardTitle>
            <CardDescription>
              按物理文件容量排序，不把 Hardlink 路径误算为重复建议。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {stats?.formatDistribution.length ? (
              stats.formatDistribution.map((format) => {
                const percent = stats.totalBytes
                  ? Math.max(2, (format.totalBytes / stats.totalBytes) * 100)
                  : 0;
                return (
                  <div key={format.extension}>
                    <div className="flex items-center justify-between gap-4 text-sm">
                      <strong>{format.extension}</strong>
                      <span className="text-muted-foreground">
                        {format.count.toLocaleString("zh-CN")} 个 · {formatBytes(format.totalBytes)}
                      </span>
                    </div>
                    <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
                      <div
                        className="h-full rounded-full bg-primary"
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                  </div>
                );
              })
            ) : (
              <p className="py-8 text-center text-sm text-muted-foreground">扫描后显示格式分布。</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>最近扫描</CardTitle>
            <CardDescription>所有 Library Root 中最近一次完成的扫描。</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="font-mono text-lg">{formatDate(stats?.recentScanAt)}</p>
            <p className="mt-3 text-sm text-muted-foreground">
              当前有 {stats?.runningJobCount ?? 0} 个运行中或排队任务。
            </p>
          </CardContent>
        </Card>
      </section>

      <DashboardActivity stats={stats} />

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck aria-hidden="true" />
            安全基线
          </CardTitle>
          <CardDescription>这些规则不会因扫描规模或 Provider 状态而改变。</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm text-muted-foreground sm:grid-cols-3">
          <p>批量修改必须 Preview → Confirm → Apply</p>
          <p>Hardlink 跨文件系统直接报错</p>
          <p>重复结果只推荐，不自动删除</p>
        </CardContent>
      </Card>
    </div>
  );
}
