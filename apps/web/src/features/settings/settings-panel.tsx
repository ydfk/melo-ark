import {
  Bot,
  Captions,
  Database,
  FolderCog,
  Gauge,
  HardDrive,
  KeyRound,
  Link2,
  ListChecks,
  Music2,
  ServerCog,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getAiStatus, getProviders } from "@/lib/api/methods/library";
import type { AiStatus, DashboardStats, Job, LibraryRoot, ProviderSetting } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";

export function SettingsPanel({
  libraries,
  jobs,
  stats,
}: {
  libraries: LibraryRoot[];
  jobs: Job[];
  stats?: DashboardStats;
}) {
  const [providers, setProviders] = useState<ProviderSetting[]>([]);
  const [ai, setAi] = useState<AiStatus>();
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    void Promise.all([getProviders().send(), getAiStatus().send()])
      .then(([nextProviders, nextAi]) => {
        setProviders(nextProviders);
        setAi(nextAi);
      })
      .catch(() => setLoadFailed(true));
  }, []);

  return (
    <div className="space-y-5">
      <div>
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-primary">Control Surface</p>
        <h2 className="mt-2 font-display text-3xl font-semibold">设置</h2>
        <p className="mt-2 text-sm text-muted-foreground">
          把运行边界、Provider、存储和安全状态放在同一处；文件级危险操作仍只在对应工作台执行。
        </p>
      </div>
      {loadFailed ? (
        <Alert variant="destructive">
          <ServerCog />
          <AlertTitle>运行配置读取失败</AlertTitle>
          <AlertDescription>
            Library 与 Job 快照仍可查看；Provider 和 AI 状态可能不是最新值，请刷新重试。
          </AlertDescription>
        </Alert>
      ) : null}
      <Tabs defaultValue="general">
        <TabsList className="h-auto w-full justify-start overflow-x-auto rounded-xl border bg-card/70 p-1">
          <TabsTrigger value="general">
            <ServerCog />
            General
          </TabsTrigger>
          <TabsTrigger value="libraries">
            <FolderCog />
            Libraries
          </TabsTrigger>
          <TabsTrigger value="metadata">
            <Database />
            Metadata
          </TabsTrigger>
          <TabsTrigger value="lyrics">
            <Captions />
            Lyrics
          </TabsTrigger>
          <TabsTrigger value="ai">
            <Bot />
            AI
          </TabsTrigger>
          <TabsTrigger value="organizer">
            <Link2 />
            Organizer
          </TabsTrigger>
          <TabsTrigger value="playback">
            <Music2 />
            Playback
          </TabsTrigger>
          <TabsTrigger value="subsonic">
            <KeyRound />
            OpenSubsonic
          </TabsTrigger>
          <TabsTrigger value="jobs">
            <ListChecks />
            Jobs
          </TabsTrigger>
          <TabsTrigger value="storage">
            <HardDrive />
            Storage
          </TabsTrigger>
          <TabsTrigger value="security">
            <Gauge />
            Security
          </TabsTrigger>
        </TabsList>

        <SettingsTab value="general" title="General" description="单管理员、本地优先的运行身份。">
          <SettingRows
            rows={[
              ["服务", "MeloArk 0.1.0"],
              ["界面语言", "简体中文"],
              ["首版平台", "linux/amd64 · 单 Docker 镜像"],
              ["数据库", "SQLite WAL"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="libraries"
          title="Libraries"
          description="实际挂载与写入能力在曲库页预检后修改。"
        >
          <div className="grid gap-3 sm:grid-cols-2">
            {libraries.map((library) => (
              <div key={library.id} className="rounded-xl border bg-muted/20 p-4">
                <div className="flex items-center justify-between gap-3">
                  <strong>{library.name}</strong>
                  <Badge variant="outline">{library.role}</Badge>
                </div>
                <p className="mt-2 truncate font-mono text-xs text-muted-foreground">
                  {library.path}
                </p>
                <p className="mt-3 text-xs text-muted-foreground">
                  {library.scanEnabled ? "扫描开启" : "扫描关闭"} ·{" "}
                  {library.watchEnabled ? "Watch 开启" : "定期 Reconcile"} ·{" "}
                  {library.writable ? "允许写入" : "只读"}
                </p>
              </div>
            ))}
            {!libraries.length ? (
              <Empty text="还没有 Library Root。请先前往曲库页添加挂载路径。" />
            ) : null}
          </div>
        </SettingsTab>

        <ProviderSettings
          value="metadata"
          title="Metadata Providers"
          description="候选只进入 Diff，不直接写文件。"
          providers={providers.filter((item) => item.kind !== "lyrics")}
        />
        <ProviderSettings
          value="lyrics"
          title="Lyrics Providers"
          description="已有歌词默认冲突，不静默覆盖。"
          providers={providers.filter((item) => item.kind !== "metadata")}
        />

        <SettingsTab value="ai" title="AI" description="只处理结构化元数据，不上传音频。">
          <SettingRows
            rows={[
              ["状态", ai?.enabled ? "已启用" : "默认关闭"],
              ["模型", ai?.model ?? "未配置"],
              ["API Key", ai?.apiKeyConfigured ? "已配置（已脱敏）" : "未配置"],
              ["上传原始音频", "永不"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="organizer"
          title="Organizer"
          description="默认 Hardlink，跨文件系统不回退 Copy。"
        >
          <SettingRows
            rows={[
              ["默认模板", "{artist}/{album}/{track:02} - {title}.{ext}"],
              ["冲突策略", "不同 inode 拒绝覆盖"],
              ["命名策略", "跨平台安全字符 + 缺失值 fallback"],
              ["执行流程", "Preflight → Dry Run → Confirm → Apply"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="playback"
          title="Playback"
          description="原始格式优先；浏览器不支持时显式转码。"
        >
          <SettingRows
            rows={[
              ["原始播放", "HTTP Range + 10 分钟媒体令牌"],
              ["Opus", "192 kbps"],
              ["AAC", "256 kbps"],
              ["MP3", "320 kbps"],
              ["缓存", "按输入和 Profile 复用，LRU 清理"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="subsonic"
          title="OpenSubsonic"
          description="供 Symfonium 等客户端连接。"
        >
          <SettingRows
            rows={[
              ["API Version", "1.16.1"],
              ["认证", "salt + token；兼容 enc: password"],
              ["搜索", "中文、全拼、拼音首字母"],
              ["扩展", "songLyrics · formPost"],
              ["Server URL", "当前 MeloArk 根地址（不要附加 /api）"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="jobs"
          title="Jobs"
          description="扫描、Hash、指纹、刮削、歌词与文件操作均持久化。"
        >
          <SettingRows
            rows={[
              ["当前记录", `${jobs.length} 个`],
              [
                "运行中",
                `${jobs.filter((job) => ["queued", "running", "paused"].includes(job.status)).length} 个`,
              ],
              ["恢复策略", "容器重启后 running → interrupted"],
              ["失败策略", "逐项记录并支持 Retry Failed"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="storage"
          title="Storage"
          description="音乐留在挂载目录，应用状态单独备份。"
        >
          <SettingRows
            rows={[
              ["数据库", "/data/meloark.db"],
              ["转码缓存", "/data/cache"],
              ["索引媒体", formatBytes(stats?.totalBytes ?? 0)],
              ["物理文件", `${stats?.mediaFileCount ?? 0} 个`],
              ["回收站", "每个 Library Root/.meloark-trash"],
            ]}
          />
        </SettingsTab>

        <SettingsTab
          value="security"
          title="Security"
          description="公开部署前仍需 HTTPS 与反向代理边界。"
        >
          <div className="grid gap-3 sm:grid-cols-2">
            {[
              "管理员密码使用 Argon2",
              "JWT 与 Provider Secret 不写日志",
              "路径执行前 canonicalize 并校验 Root",
              "危险操作 Preview → Confirm → Apply",
              "容器 UID 10001 + 只读根文件系统",
              "AI 默认关闭且不上传音频",
            ].map((item) => (
              <div key={item} className="rounded-xl border bg-muted/20 p-4 text-sm">
                {item}
              </div>
            ))}
          </div>
        </SettingsTab>
      </Tabs>
    </div>
  );
}

function SettingsTab({
  value,
  title,
  description,
  children,
}: {
  value: string;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <TabsContent value={value} className="mt-5">
      <Card>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
          <CardDescription>{description}</CardDescription>
        </CardHeader>
        <CardContent>{children}</CardContent>
      </Card>
    </TabsContent>
  );
}

function ProviderSettings({
  value,
  title,
  description,
  providers,
}: {
  value: string;
  title: string;
  description: string;
  providers: ProviderSetting[];
}) {
  return (
    <SettingsTab value={value} title={title} description={description}>
      <div className="space-y-2">
        {providers.map((provider) => (
          <div
            key={provider.providerId}
            className="flex items-center justify-between gap-3 rounded-xl border bg-muted/20 p-3"
          >
            <div>
              <p className="text-sm font-medium">{provider.displayName}</p>
              <p className="mt-1 text-xs text-muted-foreground">
                优先级 {provider.priority} · Timeout {provider.timeoutMs} ms · 间隔{" "}
                {provider.rateLimitMs} ms
              </p>
            </div>
            <div className="flex gap-2">
              <Badge variant={provider.maturity === "beta" ? "secondary" : "outline"}>
                {provider.maturity}
              </Badge>
              <Badge variant={provider.enabled ? "default" : "outline"}>
                {provider.enabled ? "启用" : "关闭"}
              </Badge>
            </div>
          </div>
        ))}
      </div>
    </SettingsTab>
  );
}

function SettingRows({ rows }: { rows: Array<[string, string]> }) {
  return (
    <div className="divide-y rounded-xl border">
      {rows.map(([label, value]) => (
        <div
          key={label}
          className="flex flex-col justify-between gap-1 px-4 py-3 sm:flex-row sm:items-center"
        >
          <span className="text-sm text-muted-foreground">{label}</span>
          <strong className="font-mono text-sm font-medium">{value}</strong>
        </div>
      ))}
    </div>
  );
}

function Empty({ text }: { text: string }) {
  return (
    <Alert>
      <ServerCog />
      <AlertTitle>尚未配置</AlertTitle>
      <AlertDescription>{text}</AlertDescription>
    </Alert>
  );
}
