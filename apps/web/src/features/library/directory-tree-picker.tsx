import { Check, ChevronRight, Folder, FolderOpen, FolderPlus, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ApiError } from "@/lib/api";
import { createDirectory, getDirectories } from "@/lib/api/methods/library";
import type { DirectoryListing } from "@/lib/api/types";
import { cn } from "@/lib/utils";

export function DirectoryTreePicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (path: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState(value || "/");
  const [creating, setCreating] = useState(false);
  const [folderName, setFolderName] = useState("");

  useEffect(() => {
    if (open) {
      setSelected(value || "/");
      setFolderName("");
    }
  }, [open, value]);

  async function createFolder() {
    if (!folderName.trim()) return;
    setCreating(true);
    try {
      const directory = await createDirectory(selected, folderName.trim()).send();
      setSelected(directory.path);
      setFolderName("");
      toast.success("文件夹已创建并选中");
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建文件夹");
    } finally {
      setCreating(false);
    }
  }

  return (
    <>
      <Button
        type="button"
        variant="outline"
        className="h-auto min-h-10 w-full justify-start px-3 py-2 text-left font-mono font-normal"
        onClick={() => setOpen(true)}
      >
        <Folder />
        <span className="min-w-0 flex-1 truncate">{value || "选择目录"}</span>
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>选择目录</DialogTitle>
          </DialogHeader>
          <div className="rounded-xl border bg-muted/15">
            <div className="border-b px-4 py-3 font-mono text-xs text-muted-foreground">
              {selected}
            </div>
            <ScrollArea className="h-[min(55vh,32rem)] p-2">
              <DirectoryNode
                name="根目录"
                path="/"
                level={0}
                selected={selected}
                onSelect={setSelected}
                defaultOpen
              />
            </ScrollArea>
          </div>
          <div className="flex gap-2">
            <Input
              value={folderName}
              onChange={(event) => setFolderName(event.target.value)}
              placeholder="在当前目录中新建文件夹"
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void createFolder();
                }
              }}
            />
            <Button
              type="button"
              variant="secondary"
              disabled={creating || !folderName.trim()}
              onClick={() => void createFolder()}
            >
              {creating ? <LoaderCircle className="animate-spin" /> : <FolderPlus />}
              新建
            </Button>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              type="button"
              onClick={() => {
                onChange(selected);
                setOpen(false);
              }}
            >
              <Check />
              选择此目录
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

function DirectoryNode({
  name,
  path,
  level,
  readable = true,
  selected,
  onSelect,
  defaultOpen = false,
}: {
  name: string;
  path: string;
  level: number;
  readable?: boolean;
  selected: string;
  onSelect: (path: string) => void;
  defaultOpen?: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultOpen);
  const [listing, setListing] = useState<DirectoryListing>();
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!expanded || listing || loading || !readable) return;
    setLoading(true);
    getDirectories(path)
      .send()
      .then((next) => {
        setListing(next);
        setFailed(false);
      })
      .catch(() => setFailed(true))
      .finally(() => setLoading(false));
  }, [expanded, listing, loading, path, readable]);

  const Icon = expanded ? FolderOpen : Folder;
  return (
    <div>
      <div
        className={cn(
          "group flex items-center rounded-lg text-sm",
          selected === path ? "bg-primary/12 text-primary" : "hover:bg-muted/70",
          !readable && "opacity-45"
        )}
        style={{ paddingLeft: `${level * 18 + 4}px` }}
      >
        <button
          type="button"
          className="grid size-8 shrink-0 place-items-center"
          disabled={!readable}
          aria-label={expanded ? `收起 ${name}` : `展开 ${name}`}
          onClick={() => setExpanded((value) => !value)}
        >
          {loading ? (
            <LoaderCircle className="size-4 animate-spin" />
          ) : (
            <ChevronRight className={cn("size-4 transition-transform", expanded && "rotate-90")} />
          )}
        </button>
        <button
          type="button"
          className="flex min-w-0 flex-1 items-center gap-2.5 py-2 pr-3 text-left"
          disabled={!readable}
          onClick={() => onSelect(path)}
        >
          <Icon className="size-4 shrink-0" />
          <span className="truncate">{name}</span>
          {failed ? <span className="ml-auto text-xs text-destructive">无法读取</span> : null}
        </button>
      </div>
      {expanded && listing
        ? listing.directories.map((entry) => (
            <DirectoryNode
              key={entry.path}
              name={entry.name}
              path={entry.path}
              readable={entry.readable}
              level={level + 1}
              selected={selected}
              onSelect={onSelect}
            />
          ))
        : null}
    </div>
  );
}
