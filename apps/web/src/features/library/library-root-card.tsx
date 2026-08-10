import { Save, ScanLine, Settings2, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ApiError } from "@/lib/api";
import { deleteLibrary, preflightLibraryPath, updateLibrary } from "@/lib/api/methods/library";
import type { LibraryRoot } from "@/lib/api/types";
import { formatDate } from "@/lib/format";

type LibraryDraft = {
  name: string;
  path: string;
  role: LibraryRoot["role"];
  scanEnabled: boolean;
  watchEnabled: boolean;
  writable: boolean;
  excludePatterns: string;
};

export function LibraryRootCard({
  library,
  onScan,
  onChanged,
}: {
  library: LibraryRoot;
  onScan: (library: LibraryRoot) => Promise<void>;
  onChanged: () => Promise<void>;
}) {
  const [draft, setDraft] = useState<LibraryDraft>();
  const [saving, setSaving] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  function openSettings() {
    setDraft({
      name: library.name,
      path: library.path,
      role: library.role,
      scanEnabled: library.scanEnabled,
      watchEnabled: library.watchEnabled,
      writable: library.writable,
      excludePatterns: library.excludePatterns.join(", "),
    });
    setConfirmDelete(false);
  }

  async function save() {
    if (!draft) return;
    setSaving(true);
    try {
      const preflight = await preflightLibraryPath(draft.path.trim()).send();
      await updateLibrary(library.id, {
        name: draft.name.trim(),
        path: preflight.canonicalPath,
        role: draft.role,
        scanEnabled: draft.scanEnabled,
        watchEnabled: draft.watchEnabled,
        writable: draft.writable,
        excludePatterns: draft.excludePatterns
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
      }).send();
      setDraft(undefined);
      toast.success("Library Root 已更新");
      await onChanged();
    } catch (error) {
      showError(error, "Library Root 更新失败");
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    setSaving(true);
    try {
      await deleteLibrary(library.id).send();
      setDraft(undefined);
      toast.success("已删除曲库配置和索引，音乐文件未受影响");
      await onChanged();
    } catch (error) {
      showError(error, "Library Root 删除失败");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <Card>
        <CardHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <CardTitle>{library.name}</CardTitle>
              <CardDescription className="mt-1 break-all font-mono">{library.path}</CardDescription>
            </div>
            <Badge variant="secondary">{library.role}</Badge>
          </div>
        </CardHeader>
        <CardContent className="flex items-end justify-between gap-4">
          <div className="flex flex-col gap-1 text-xs text-muted-foreground">
            <span>{library.watchEnabled ? "Watch 已启用" : "仅定时/手动扫描"}</span>
            <span>上次扫描：{formatDate(library.lastScanAt)}</span>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={openSettings}>
              <Settings2 data-icon="inline-start" />
              配置
            </Button>
            <Button size="sm" onClick={() => void onScan(library)}>
              <ScanLine data-icon="inline-start" />
              扫描
            </Button>
          </div>
        </CardContent>
      </Card>

      <Dialog open={Boolean(draft)} onOpenChange={(open) => !open && setDraft(undefined)}>
        <DialogContent className="max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>配置 Library Root</DialogTitle>
            <DialogDescription>
              路径保存前会再次预检。删除只移除 MeloArk 配置和索引，不会删除音乐文件。
            </DialogDescription>
          </DialogHeader>
          {draft ? (
            <form
              className="grid gap-4"
              onSubmit={(event) => {
                event.preventDefault();
                void save();
              }}
            >
              <LibraryTextField
                id="edit-library-name"
                label="名称"
                value={draft.name}
                onChange={(name) =>
                  setDraft((current) => (current ? { ...current, name } : current))
                }
              />
              <LibraryTextField
                id="edit-library-path"
                label="容器内路径"
                value={draft.path}
                monospace
                onChange={(path) =>
                  setDraft((current) => (current ? { ...current, path } : current))
                }
              />
              <div className="grid gap-2">
                <Label>用途</Label>
                <ToggleGroup
                  type="single"
                  value={draft.role}
                  onValueChange={(role: LibraryRoot["role"]) =>
                    role && setDraft((current) => (current ? { ...current, role } : current))
                  }
                >
                  <ToggleGroupItem value="source">来源</ToggleGroupItem>
                  <ToggleGroupItem value="managed">已整理</ToggleGroupItem>
                  <ToggleGroupItem value="both">两者</ToggleGroupItem>
                </ToggleGroup>
              </div>
              <LibraryTextField
                id="edit-library-excludes"
                label="排除目录（逗号分隔）"
                value={draft.excludePatterns}
                onChange={(excludePatterns) =>
                  setDraft((current) => (current ? { ...current, excludePatterns } : current))
                }
              />
              <LibrarySwitch
                id="edit-library-scan"
                label="允许扫描"
                checked={draft.scanEnabled}
                onChange={(scanEnabled) =>
                  setDraft((current) => (current ? { ...current, scanEnabled } : current))
                }
              />
              <LibrarySwitch
                id="edit-library-watch"
                label="启用 Watch"
                checked={draft.watchEnabled}
                onChange={(watchEnabled) =>
                  setDraft((current) => (current ? { ...current, watchEnabled } : current))
                }
              />
              <LibrarySwitch
                id="edit-library-writable"
                label="允许 Tag、歌词和文件操作写入"
                checked={draft.writable}
                onChange={(writable) =>
                  setDraft((current) => (current ? { ...current, writable } : current))
                }
              />
              {confirmDelete ? (
                <Alert variant="destructive">
                  <Trash2 />
                  <AlertTitle>确认删除「{library.name}」的配置与索引？</AlertTitle>
                  <AlertDescription>
                    音乐文件不会被删除；正在运行的任务会阻止此操作。
                  </AlertDescription>
                </Alert>
              ) : null}
              <DialogFooter className="gap-2 sm:justify-between">
                {confirmDelete ? (
                  <Button
                    type="button"
                    variant="destructive"
                    disabled={saving}
                    onClick={() => void remove()}
                  >
                    确认删除配置与索引
                  </Button>
                ) : (
                  <Button type="button" variant="ghost" onClick={() => setConfirmDelete(true)}>
                    <Trash2 data-icon="inline-start" />
                    删除曲库
                  </Button>
                )}
                <Button disabled={saving || !draft.name.trim() || !draft.path.trim()}>
                  <Save data-icon="inline-start" />
                  保存
                </Button>
              </DialogFooter>
            </form>
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}

function LibraryTextField({
  id,
  label,
  value,
  monospace = false,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  monospace?: boolean;
  onChange: (value: string) => void;
}) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        required
        className={monospace ? "font-mono" : undefined}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function LibrarySwitch({
  id,
  label,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border p-3">
      <Label htmlFor={id}>{label}</Label>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

function showError(error: unknown, fallback: string) {
  toast.error(error instanceof ApiError ? error.problem.detail : fallback);
}
