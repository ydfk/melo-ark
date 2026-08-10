import { ArrowRight } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type { Operation } from "@/lib/api/types";

export function OperationPreview({ operation }: { operation?: Operation }) {
  if (!operation) return null;
  return (
    <div className="space-y-3 rounded-xl border bg-muted/30 p-4">
      <div className="flex items-center justify-between">
        <p className="font-medium">操作预览</p>
        <Badge variant="secondary">{operation.status}</Badge>
      </div>
      {operation.items.map((item) => (
        <div key={item.id} className="rounded-lg border bg-background/60 p-3 text-sm">
          {item.sourcePath ? (
            <p className="break-all font-mono text-xs text-muted-foreground">{item.sourcePath}</p>
          ) : null}
          {item.targetPath ? (
            <p className="mt-2 break-all font-mono text-xs">
              <ArrowRight className="mr-1 inline size-3" />
              {item.targetPath}
            </p>
          ) : null}
          {item.diffs.map((diff) => (
            <div key={diff.field} className="mt-2 grid grid-cols-[7rem_1fr] gap-2">
              <span className="text-muted-foreground">{diff.field}</span>
              <span>
                <span className="line-through opacity-60">{diff.before || "∅"}</span>
                <ArrowRight className="mx-2 inline size-3" />
                {diff.after || "∅"}
              </span>
            </div>
          ))}
          {item.preflight ? (
            <div className="mt-3 flex flex-wrap gap-2">
              <Badge variant={item.preflight.sameFilesystem ? "secondary" : "destructive"}>
                {item.preflight.sameFilesystem ? "同一文件系统" : "跨文件系统"}
              </Badge>
              {item.preflight.pathConflict ? (
                <Badge variant="destructive">路径冲突</Badge>
              ) : (
                <Badge variant="secondary">无覆盖冲突</Badge>
              )}
            </div>
          ) : null}
          {item.errorMessage ? <p className="mt-2 text-destructive">{item.errorMessage}</p> : null}
        </div>
      ))}
    </div>
  );
}
