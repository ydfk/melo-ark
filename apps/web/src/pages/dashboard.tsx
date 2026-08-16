import {
  ClipboardList,
  CloudCog,
  Disc3,
  ListTodo,
  Music2,
  Settings,
  Trash2,
  Workflow,
} from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";

import { BrandMark } from "@/components/brand-mark";
import { DashboardCommandPalette, type DashboardTab } from "@/components/dashboard-command-palette";
import { ProfileMenu } from "@/components/profile-menu";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { ManagementMiniPlayer } from "@/features/player/management-mini-player";
import { ManagementPipeline } from "@/features/dashboard/management-pipeline";
import { JobActivityProvider } from "@/features/tasks/job-activity-context";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getDashboardStats, getLibraries } from "@/lib/api/methods/library";
import type { DashboardStats, LibraryGroup, UserResponse } from "@/lib/api/types";

type DashboardPageProps = {
  user: UserResponse;
  onUserChanged: (user: UserResponse) => void;
  onLogout: () => void;
  onOpenPlayer: () => void;
};

const LibraryPanel = lazy(() =>
  import("@/features/library/library-panel").then((module) => ({
    default: module.LibraryPanel,
  }))
);
const SongsPanel = lazy(() =>
  import("@/features/library/songs-panel").then((module) => ({
    default: module.SongsPanel,
  }))
);
const TasksPanel = lazy(() =>
  import("@/features/tasks/tasks-panel").then((module) => ({
    default: module.TasksPanel,
  }))
);
const ReviewPanel = lazy(() =>
  import("@/features/reviews/review-panel").then((module) => ({
    default: module.ReviewPanel,
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
export function DashboardPage({ user, onUserChanged, onLogout, onOpenPlayer }: DashboardPageProps) {
  const [stats, setStats] = useState<DashboardStats>();
  const [libraries, setLibraries] = useState<LibraryGroup[]>([]);
  const [refreshKey, setRefreshKey] = useState(0);
  const [activeTab, setActiveTab] = useState<DashboardTab>("library");
  const [loadError, setLoadError] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      const [nextStats, nextLibraries] = await Promise.all([
        getDashboardStats().send(),
        getLibraries().send(),
      ]);
      setStats(nextStats);
      setLibraries(nextLibraries);
      setLoadError(undefined);
    } catch {
      setLoadError("无法读取服务状态。请确认 MeloArk Server 正在运行，然后重试。");
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleJobTerminal = useCallback(() => {
    setRefreshKey((value) => value + 1);
    void refresh();
  }, [refresh]);

  return (
    <JobActivityProvider onTerminal={handleJobTerminal}>
      <main className="min-h-screen px-4 pb-24 pt-4 sm:px-6 sm:pb-28 sm:pt-6 xl:px-8 xl:pb-8">
        <header className="flex w-full items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <BrandMark className="size-9" />
            <div>
              <p className="font-display font-semibold tracking-tight">MeloArk</p>
              <p className="text-xs text-muted-foreground">曲库管理工作区</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <div className="hidden rounded-xl border bg-muted/50 p-1 sm:flex" aria-label="工作模式">
              <Button variant="ghost" size="sm" onClick={onOpenPlayer}>
                <Disc3 data-icon="inline-start" />
                播放
              </Button>
              <Button size="sm">
                <Workflow data-icon="inline-start" />
                管理
              </Button>
            </div>
            <Button
              variant="outline"
              size="icon"
              className="sm:hidden"
              onClick={onOpenPlayer}
              aria-label="返回播放"
            >
              <Disc3 />
            </Button>
            <DashboardCommandPalette
              onNavigate={setActiveTab}
              onRefresh={() => void refresh()}
              onLogout={onLogout}
            />
            <ProfileMenu user={user} onChanged={onUserChanged} onLogout={onLogout} />
          </div>
        </header>

        {loadError ? (
          <Alert variant="destructive" className="mt-6">
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

        <div className="mt-7 grid w-full gap-6 xl:grid-cols-[minmax(0,1fr)_300px] xl:items-start">
          <Tabs
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as DashboardTab)}
            className="min-w-0"
          >
            <TabsList className="h-auto w-full justify-start overflow-x-auto rounded-xl border bg-card/80 p-1 shadow-sm [scrollbar-width:none] backdrop-blur-md [&::-webkit-scrollbar]:hidden sm:w-fit">
              <TabsTrigger value="library">
                <Workflow />
                曲库接入
              </TabsTrigger>
              <TabsTrigger value="songs">
                <Music2 />
                歌曲列表
              </TabsTrigger>
              <TabsTrigger value="reviews">
                <ClipboardList />
                待处理
              </TabsTrigger>
              <TabsTrigger value="tasks">
                <ListTodo />
                任务
                {stats?.runningJobCount ? (
                  <Badge variant="secondary">{stats.runningJobCount}</Badge>
                ) : null}
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
            <ManagementPipeline
              stats={stats}
              libraries={libraries}
              activeTab={activeTab}
              onNavigate={setActiveTab}
            />
            <TabsContent value="library" className="mt-6">
              <Suspense fallback={<PanelLoading />}>
                <LibraryPanel
                  libraries={libraries}
                  onChanged={async () => {
                    await refresh();
                    setRefreshKey((value) => value + 1);
                  }}
                />
              </Suspense>
            </TabsContent>
            <TabsContent value="songs" className="mt-6">
              <Suspense fallback={<PanelLoading />}>
                <SongsPanel
                  libraries={libraries}
                  refreshKey={refreshKey}
                  onChanged={async () => {
                    await refresh();
                    setRefreshKey((value) => value + 1);
                  }}
                />
              </Suspense>
            </TabsContent>
            <TabsContent value="reviews" className="mt-6">
              <Suspense fallback={<PanelLoading />}>
                <ReviewPanel onChanged={refresh} />
              </Suspense>
            </TabsContent>
            <TabsContent value="tasks" className="mt-6">
              <Suspense fallback={<PanelLoading />}>
                <TasksPanel onChanged={refresh} />
              </Suspense>
            </TabsContent>
            <TabsContent value="trash" className="mt-6">
              <Suspense fallback={<PanelLoading />}>
                <TrashPanel onChanged={refresh} />
              </Suspense>
            </TabsContent>
            <TabsContent value="settings" className="mt-6">
              <Suspense fallback={<PanelLoading />}>
                <SettingsPanel />
              </Suspense>
            </TabsContent>
          </Tabs>
          <ManagementMiniPlayer onOpenPlayer={onOpenPlayer} />
        </div>
      </main>
    </JobActivityProvider>
  );
}

function PanelLoading() {
  return (
    <div className="grid gap-4 py-2 sm:grid-cols-2" aria-label="正在加载页面">
      <Skeleton className="h-44 rounded-2xl" />
      <Skeleton className="h-44 rounded-2xl" />
      <Skeleton className="h-64 rounded-2xl sm:col-span-2" />
    </div>
  );
}
