import {
  CloudCog,
  CopyCheck,
  LayoutDashboard,
  Library,
  ListMusic,
  LogOut,
  RadioTower,
  Settings,
  Trash2,
} from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";

import { BrandMark } from "@/components/brand-mark";
import { DashboardCommandPalette, type DashboardTab } from "@/components/dashboard-command-palette";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { OverviewPanel } from "@/features/dashboard/overview-panel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { subscribeToJobEvents } from "@/lib/api/events";
import { getJobs } from "@/lib/api/methods/jobs";
import { getDashboardStats, getLibraries } from "@/lib/api/methods/library";
import type { DashboardStats, Job, LibraryRoot, UserResponse } from "@/lib/api/types";

type DashboardPageProps = {
  user: UserResponse;
  onLogout: () => void;
};

const LibraryPanel = lazy(() =>
  import("@/features/library/library-panel").then((module) => ({
    default: module.LibraryPanel,
  }))
);
const TasksPanel = lazy(() =>
  import("@/features/tasks/tasks-panel").then((module) => ({
    default: module.TasksPanel,
  }))
);
const ProviderPanel = lazy(() =>
  import("@/features/scraper/provider-panel").then((module) => ({ default: module.ProviderPanel }))
);
const DuplicatePanel = lazy(() =>
  import("@/features/duplicates/duplicate-panel").then((module) => ({
    default: module.DuplicatePanel,
  }))
);
const PlaybackPanel = lazy(() =>
  import("@/features/player/playback-panel").then((module) => ({
    default: module.PlaybackPanel,
  }))
);
const TrashPanel = lazy(() =>
  import("@/features/trash/trash-panel").then((module) => ({ default: module.TrashPanel }))
);
const SettingsPanel = lazy(() =>
  import("@/features/settings/settings-panel").then((module) => ({
    default: module.SettingsPanel,
  }))
);
const PlayerBar = lazy(() =>
  import("@/features/player/player-bar").then((module) => ({ default: module.PlayerBar }))
);

export function DashboardPage({ user, onLogout }: DashboardPageProps) {
  const [stats, setStats] = useState<DashboardStats>();
  const [libraries, setLibraries] = useState<LibraryRoot[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [refreshKey, setRefreshKey] = useState(0);
  const [activeTab, setActiveTab] = useState<DashboardTab>("dashboard");
  const [loadError, setLoadError] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      const [nextStats, nextLibraries, nextJobs] = await Promise.all([
        getDashboardStats().send(),
        getLibraries().send(),
        getJobs().send(),
      ]);
      setStats(nextStats);
      setLibraries(nextLibraries);
      setJobs(nextJobs);
      setLoadError(undefined);
    } catch {
      setLoadError("无法读取服务状态。请确认 MeloArk Server 正在运行，然后重试。");
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const controller = new AbortController();
    async function connect() {
      while (!controller.signal.aborted) {
        try {
          await subscribeToJobEvents(controller.signal, ({ job }) => {
            setJobs((current) => [job, ...current.filter((item) => item.id !== job.id)]);
            if (
              ["completed", "completed_with_errors", "failed", "cancelled"].includes(job.status)
            ) {
              setRefreshKey((value) => value + 1);
              void refresh();
            }
          });
        } catch {
          if (!controller.signal.aborted) {
            await new Promise((resolve) => window.setTimeout(resolve, 2_000));
          }
        }
      }
    }
    void connect();
    return () => controller.abort();
  }, [refresh]);

  return (
    <main className="min-h-screen px-4 py-4 pb-52 sm:px-7 sm:pb-32">
      <header className="mx-auto flex max-w-7xl items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <BrandMark className="size-9" />
          <div>
            <p className="font-display font-semibold tracking-tight">MeloArk</p>
            <p className="text-xs text-muted-foreground">音乐管理与整理中心</p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <DashboardCommandPalette
            onNavigate={setActiveTab}
            onRefresh={() => void refresh()}
            onLogout={onLogout}
          />
          <Badge variant="secondary">{user.username}</Badge>
          <Button variant="ghost" size="icon" onClick={onLogout} aria-label="退出登录">
            <LogOut />
          </Button>
        </div>
      </header>

      {loadError ? (
        <Alert variant="destructive" className="mx-auto mt-6 max-w-7xl">
          <CloudCog />
          <AlertTitle>服务状态不可用</AlertTitle>
          <AlertDescription className="flex flex-wrap items-center justify-between gap-3">
            <span>{loadError}</span>
            <Button variant="outline" size="sm" onClick={() => void refresh()}>
              重试
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}

      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as DashboardTab)}
        className="mx-auto mt-7 max-w-7xl"
      >
        <TabsList className="h-auto w-full justify-start overflow-x-auto rounded-xl border bg-card/70 p-1 backdrop-blur-md sm:w-fit">
          <TabsTrigger value="dashboard">
            <LayoutDashboard />
            总览
          </TabsTrigger>
          <TabsTrigger value="library">
            <Library />
            曲库
          </TabsTrigger>
          <TabsTrigger value="tasks">
            <ListMusic />
            任务
            {stats?.runningJobCount ? (
              <Badge variant="secondary">{stats.runningJobCount}</Badge>
            ) : null}
          </TabsTrigger>
          <TabsTrigger value="providers">
            <CloudCog />
            Provider
          </TabsTrigger>
          <TabsTrigger value="duplicates">
            <CopyCheck />
            重复
          </TabsTrigger>
          <TabsTrigger value="playback">
            <RadioTower />
            播放
          </TabsTrigger>
          <TabsTrigger value="trash">
            <Trash2 />
            回收站
          </TabsTrigger>
          <TabsTrigger value="settings">
            <Settings />
            设置
          </TabsTrigger>
        </TabsList>
        <TabsContent value="dashboard" className="mt-6">
          <OverviewPanel stats={stats} libraries={libraries} jobs={jobs} />
        </TabsContent>
        <TabsContent value="library" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <LibraryPanel
              libraries={libraries}
              refreshKey={refreshKey}
              onChanged={async () => {
                await refresh();
                setRefreshKey((value) => value + 1);
              }}
            />
          </Suspense>
        </TabsContent>
        <TabsContent value="tasks" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <TasksPanel jobs={jobs} onChanged={refresh} />
          </Suspense>
        </TabsContent>
        <TabsContent value="providers" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <ProviderPanel />
          </Suspense>
        </TabsContent>
        <TabsContent value="duplicates" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <DuplicatePanel onChanged={refresh} />
          </Suspense>
        </TabsContent>
        <TabsContent value="playback" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <PlaybackPanel />
          </Suspense>
        </TabsContent>
        <TabsContent value="trash" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <TrashPanel onChanged={refresh} />
          </Suspense>
        </TabsContent>
        <TabsContent value="settings" className="mt-6">
          <Suspense fallback={<PanelLoading />}>
            <SettingsPanel libraries={libraries} jobs={jobs} stats={stats} />
          </Suspense>
        </TabsContent>
      </Tabs>
      <Suspense fallback={null}>
        <PlayerBar />
      </Suspense>
    </main>
  );
}

function PanelLoading() {
  return (
    <div className="grid gap-4 py-2 sm:grid-cols-2" aria-label="正在装载管理舱">
      <Skeleton className="h-44 rounded-2xl" />
      <Skeleton className="h-44 rounded-2xl" />
      <Skeleton className="h-64 rounded-2xl sm:col-span-2" />
    </div>
  );
}
