import { ArchiveRestore, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ApiError } from "@/lib/api";
import {
  applyTrashPurge,
  getTrashEntries,
  previewTrashPurge,
  restoreTrash,
} from "@/lib/api/methods/library";
import type { TrashEntry, TrashPurge } from "@/lib/api/types";
import { formatBytes, formatDate } from "@/lib/format";

export function TrashPanel({ onChanged }: { onChanged: () => Promise<void> }) {
  const [entries, setEntries] = useState<TrashEntry[]>([]);
  const [preview, setPreview] = useState<TrashPurge>();
  const [busyId, setBusyId] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      setEntries(await getTrashEntries().send());
    } catch (error) {
      showError(error);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function restore(operationId: string) {
    setBusyId(operationId);
    try {
      await restoreTrash(operationId).send();
      toast.success("文件已恢复到原路径");
      setPreview(undefined);
      await Promise.all([refresh(), onChanged()]);
    } catch (error) {
      showError(error);
    } finally {
      setBusyId(undefined);
    }
  }

  async function preparePurge(operationId: string) {
    setBusyId(operationId);
    try {
      const result = await previewTrashPurge(operationId).send();
      setPreview(result);
      if (result.items.some((item) => item.status === "failed")) {
        toast.warning("预检发现不安全或已变化的文件；这些文件不会被删除");
      }
      await refresh();
    } catch (error) {
      showError(error);
    } finally {
      setBusyId(undefined);
    }
  }

  async function confirmPurge() {
    if (!preview) return;
    setBusyId(preview.trashOperationId);
    try {
      const result = await applyTrashPurge(preview.id).send();
      setPreview(result);
      toast.success(
        result.status === "completed"
          ? "回收站文件已永久删除"
          : "永久清理完成，但有文件被安全检查阻止"
      );
      await Promise.all([refresh(), onChanged()]);
    } catch (error) {
      showError(error);
    } finally {
      setBusyId(undefined);
    }
  }

  return (
    <div className="space-y-5">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.2em] text-primary">Trash Journal</p>
          <h2 className="mt-2 font-display text-3xl font-semibold">回收站</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            移入回收站、恢复和永久清理都有持久化记录；永久清理会再次验证路径与 inode。
          </p>
        </div>
        <Button variant="outline" onClick={() => void refresh()}>
          <RefreshCw data-icon="inline-start" />
          刷新
        </Button>
      </div>

      <Alert variant="destructive">
        <ShieldAlert />
        <AlertTitle>永久删除不可撤销</AlertTitle>
        <AlertDescription>
          必须先生成永久清理
          Preview，再单独确认执行。系统只删除通过两次校验的普通文件，拒绝符号链接和路径逃逸。
        </AlertDescription>
      </Alert>

      {preview ? (
        <Card className="border-destructive/40 bg-destructive/5">
          <CardHeader>
            <CardTitle>永久清理 Preview</CardTitle>
            <CardDescription>
              {preview.totalItems} 个文件，约 {formatBytes(preview.totalBytes)}；
              {preview.items.filter((item) => item.status === "failed").length} 个未通过预检。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="max-h-52 space-y-2 overflow-auto rounded-xl border bg-background/70 p-3">
              {preview.items.map((item) => (
                <div key={item.id} className="flex items-start justify-between gap-3 text-xs">
                  <div className="min-w-0">
                    <p className="truncate font-mono">{item.path}</p>
                    {item.errorMessage ? (
                      <p className="mt-1 text-destructive">{item.errorMessage}</p>
                    ) : null}
                  </div>
                  <Badge variant={item.status === "failed" ? "destructive" : "secondary"}>
                    {item.status === "failed"
                      ? "已阻止"
                      : item.status === "success"
                        ? "已删除"
                        : "待确认"}
                  </Badge>
                </div>
              ))}
            </div>
            {preview.status === "previewed" ? (
              <div className="flex flex-wrap items-center justify-between gap-3">
                <p className="text-sm font-medium text-destructive">
                  请再次确认：永久删除不可撤销。
                </p>
                <Button
                  variant="destructive"
                  disabled={busyId === preview.trashOperationId}
                  onClick={() => void confirmPurge()}
                >
                  <Trash2 data-icon="inline-start" />
                  我已理解，永久删除
                </Button>
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">
                本次永久清理已结束，记录仍保留用于审计。
              </p>
            )}
          </CardContent>
        </Card>
      ) : null}

      {entries.length ? (
        <section className="grid gap-4 xl:grid-cols-2">
          {entries.map((entry) => {
            const purged = entry.purgeStatus && entry.purgeStatus !== "previewed";
            return (
              <Card key={entry.operationId} className="bg-card/70">
                <CardHeader>
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <CardTitle>{entry.itemCount} 个文件</CardTitle>
                      <CardDescription>
                        {formatDate(entry.finishedAt ?? entry.createdAt)}
                      </CardDescription>
                    </div>
                    <Badge variant={purged ? "outline" : "secondary"}>
                      {purged
                        ? "已永久清理"
                        : entry.purgeStatus === "previewed"
                          ? "待二次确认"
                          : "可恢复"}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent className="space-y-4">
                  <p className="font-mono text-sm">{formatBytes(entry.totalBytes)}</p>
                  <div className="flex flex-wrap gap-2">
                    {!purged ? (
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={busyId === entry.operationId}
                        onClick={() => void restore(entry.operationId)}
                      >
                        <ArchiveRestore data-icon="inline-start" />
                        恢复
                      </Button>
                    ) : null}
                    {!purged ? (
                      <Button
                        variant="destructive"
                        size="sm"
                        disabled={busyId === entry.operationId}
                        onClick={() => void preparePurge(entry.operationId)}
                      >
                        <Trash2 data-icon="inline-start" />
                        {entry.purgeStatus === "previewed"
                          ? "查看永久清理 Preview"
                          : "生成永久清理 Preview"}
                      </Button>
                    ) : null}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </section>
      ) : (
        <Alert>
          <Trash2 />
          <AlertTitle>回收站为空</AlertTitle>
          <AlertDescription>从重复分析或曲库工作台移入的文件会显示在这里。</AlertDescription>
        </Alert>
      )}
    </div>
  );
}

function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "回收站操作失败");
}
