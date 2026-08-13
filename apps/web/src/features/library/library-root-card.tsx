import {
  CircleAlert,
  FolderInput,
  FolderTree,
  Plus,
  ScanLine,
  Settings2,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { DirectoryTreePicker } from "@/features/library/directory-tree-picker";
import { InlineJobStatus } from "@/features/tasks/inline-job-status";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { ApiError } from "@/lib/api";
import {
  createLibrary,
  deleteLibrary,
  deleteLibraryGroup,
  preflightLibraryPath,
  updateLibrary,
} from "@/lib/api/methods/library";
import type { LibraryGroup, LibrarySource } from "@/lib/api/types";
import { formatDate } from "@/lib/format";

type SourceDraft = {
  source?: LibrarySource;
  sourcePath: string;
  organizedPath: string;
  watchEnabled: boolean;
  autoIngestEnabled: boolean;
};

export function LibraryGroupCard({
  library,
  onScan,
  onChanged,
}: {
  library: LibraryGroup;
  onScan: (library: LibrarySource) => Promise<void>;
  onChanged: () => Promise<void>;
}) {
  const { latestJob } = useJobActivity();
  const [draft, setDraft] = useState<SourceDraft>();
  const [removeSource, setRemoveSource] = useState<LibrarySource>();
  const [removeGroup, setRemoveGroup] = useState(false);
  const [saving, setSaving] = useState(false);
  const ready = library.status === "ready" && Boolean(library.organizedPath);

  function openSource(source?: LibrarySource) {
    setDraft({
      source,
      sourcePath: source?.sourcePath ?? "",
      organizedPath: library.organizedPath ?? "",
      watchEnabled: source?.watchEnabled ?? false,
      autoIngestEnabled: source?.autoIngestEnabled ?? true,
    });
  }

  async function saveSource() {
    if (!draft?.sourcePath || !draft.organizedPath) return;
    setSaving(true);
    try {
      const [source, organized] = await Promise.all([
        preflightLibraryPath(draft.sourcePath).send(),
        preflightLibraryPath(draft.organizedPath).send(),
      ]);
      const request = {
        sourcePath: source.canonicalPath,
        organizedPath: organized.canonicalPath,
        watchEnabled: draft.watchEnabled,
        autoIngestEnabled: draft.autoIngestEnabled,
      };
      if (draft.source) await updateLibrary(draft.source.id, request).send();
      else await createLibrary(request).send();
      toast.success(draft.source ? "来源配置已更新" : "来源目录已添加");
      setDraft(undefined);
      await onChanged();
    } catch (error) {
      showError(error, "无法保存来源目录");
    } finally {
      setSaving(false);
    }
  }

  async function confirmRemoveSource() {
    if (!removeSource) return;
    setSaving(true);
    try {
      await deleteLibrary(removeSource.id).send();
      toast.success("来源配置和索引已移除，音乐文件未受影响");
      setRemoveSource(undefined);
      await onChanged();
    } catch (error) {
      showError(error, "无法移除来源目录");
    } finally {
      setSaving(false);
    }
  }

  async function confirmRemoveGroup() {
    if (!library.organizedLibraryId) return;
    setSaving(true);
    try {
      await deleteLibraryGroup(library.organizedLibraryId).send();
      toast.success("曲库索引已删除，磁盘文件未受影响");
      setRemoveGroup(false);
      await onChanged();
    } catch (error) {
      showError(error, "无法删除曲库");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <Card className="overflow-hidden">
        <CardHeader className="border-b bg-muted/20">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
                <FolderTree className="size-4" />
                整理后目录
              </div>
              <CardTitle className="break-all font-mono text-base">
                {library.organizedPath ?? "尚未设置整理目录"}
              </CardTitle>
            </div>
            <Badge variant={ready ? "secondary" : "destructive"}>
              {ready ? `${library.sources.length} 个来源` : "待配置"}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-3 pt-5">
          {!ready ? (
            <Alert variant="destructive">
              <CircleAlert />
              <AlertTitle>暂不进入播放目录</AlertTitle>
              <AlertDescription>设置整理后目录即可恢复自动处理。</AlertDescription>
            </Alert>
          ) : null}
          {library.sources.map((source) => (
            <div key={source.id} className="rounded-xl border bg-background/70 p-3">
              <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-start">
                <div className="min-w-0">
                  <p className="mb-1 text-xs text-muted-foreground">来源目录</p>
                  <p className="break-all font-mono text-sm">{source.sourcePath}</p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {source.watchEnabled ? "监听变化" : "定时或手动扫描"} · 上次扫描：
                    {formatDate(source.lastScanAt)}
                  </p>
                </div>
                <div className="flex shrink-0 gap-2">
                  <Button
                    variant="outline"
                    size="icon"
                    className="size-8"
                    onClick={() => openSource(source)}
                  >
                    <Settings2 />
                    <span className="sr-only">配置来源</span>
                  </Button>
                  <Button
                    variant="outline"
                    size="icon"
                    className="size-8"
                    onClick={() => setRemoveSource(source)}
                  >
                    <Trash2 />
                    <span className="sr-only">移除来源</span>
                  </Button>
                  <Button size="sm" onClick={() => void onScan(source)}>
                    <ScanLine />
                    扫描
                  </Button>
                </div>
              </div>
              <div className="mt-3 space-y-2">
                <InlineJobStatus job={latestJob("library", source.id, "scan")} />
                <InlineJobStatus job={latestJob("library", source.id, "ingest")} />
              </div>
            </div>
          ))}
          <div className="flex flex-wrap justify-between gap-2 pt-1">
            <Button variant="outline" size="sm" onClick={() => openSource()}>
              <Plus />
              添加来源
            </Button>
            {library.organizedLibraryId ? (
              <Button variant="ghost" size="sm" onClick={() => setRemoveGroup(true)}>
                <Trash2 />
                删除曲库
              </Button>
            ) : null}
          </div>
        </CardContent>
      </Card>

      <Dialog open={Boolean(draft)} onOpenChange={(open) => !open && setDraft(undefined)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{draft?.source ? "配置来源目录" : "添加来源目录"}</DialogTitle>
            <DialogDescription>来源中的新增音乐会硬链接到整理后目录。</DialogDescription>
          </DialogHeader>
          {draft ? (
            <div className="space-y-5">
              <div className="space-y-2">
                <Label>来源目录</Label>
                <DirectoryTreePicker
                  value={draft.sourcePath}
                  onChange={(sourcePath) => setDraft((value) => value && { ...value, sourcePath })}
                />
              </div>
              <div className="space-y-2">
                <Label>整理后目录</Label>
                <DirectoryTreePicker
                  value={draft.organizedPath}
                  onChange={(organizedPath) =>
                    setDraft((value) => value && { ...value, organizedPath })
                  }
                />
              </div>
              <SourceSwitch
                label="自动处理新增音乐"
                checked={draft.autoIngestEnabled}
                onChange={(autoIngestEnabled) =>
                  setDraft((value) => value && { ...value, autoIngestEnabled })
                }
              />
              <SourceSwitch
                label="监听目录变化"
                checked={draft.watchEnabled}
                onChange={(watchEnabled) =>
                  setDraft((value) => value && { ...value, watchEnabled })
                }
              />
            </div>
          ) : null}
          <DialogFooter>
            <Button variant="outline" onClick={() => setDraft(undefined)}>
              取消
            </Button>
            <Button
              disabled={saving || !draft?.sourcePath || !draft.organizedPath}
              onClick={() => void saveSource()}
            >
              <FolderInput />
              保存
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDeleteDialog
        open={Boolean(removeSource)}
        title="移除这个来源目录？"
        detail="只删除来源配置和索引，不会删除来源或整理后的音乐文件。"
        saving={saving}
        onClose={() => setRemoveSource(undefined)}
        onConfirm={() => void confirmRemoveSource()}
      />
      <ConfirmDeleteDialog
        open={removeGroup}
        title="删除整个曲库？"
        detail="所有来源和整理目录索引都会移除，但磁盘上的音乐文件不会被删除。"
        saving={saving}
        onClose={() => setRemoveGroup(false)}
        onConfirm={() => void confirmRemoveGroup()}
      />
    </>
  );
}

function SourceSwitch({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border p-3">
      <Label>{label}</Label>
      <Switch checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

function ConfirmDeleteDialog({
  open,
  title,
  detail,
  saving,
  onClose,
  onConfirm,
}: {
  open: boolean;
  title: string;
  detail: string;
  saving: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{detail}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
          <Button variant="destructive" disabled={saving} onClick={onConfirm}>
            确认删除
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function showError(error: unknown, fallback: string) {
  toast.error(error instanceof ApiError ? error.problem.detail : fallback);
}
