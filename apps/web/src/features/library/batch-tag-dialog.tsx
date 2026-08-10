import { Save, Tags, WandSparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { ApiError } from "@/lib/api";
import { applyTags, getTrackFiles, previewTags, undoTags } from "@/lib/api/methods/library";
import type { Operation, TagField, TagTransform } from "@/lib/api/types";
import { OperationPreview } from "@/features/library/operation-preview";

type BatchMode = "set" | "find" | "regex" | "simplify" | "punctuation" | "filename" | "trim";

export function BatchTagDialog({
  trackIds,
  open,
  onOpenChange,
  onChanged,
}: {
  trackIds: string[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChanged: () => Promise<void>;
}) {
  const [mediaIds, setMediaIds] = useState<string[]>([]);
  const [mode, setMode] = useState<BatchMode>("set");
  const [field, setField] = useState<TagField>("artists");
  const [value, setValue] = useState("");
  const [replacement, setReplacement] = useState("");
  const [operation, setOperation] = useState<Operation>();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setOperation(undefined);
    Promise.all(trackIds.map((id) => getTrackFiles(id).send()))
      .then((groups) =>
        setMediaIds(
          groups
            .flat()
            .filter((file) => file.libraryWritable)
            .map((file) => file.id)
        )
      )
      .catch(showError);
  }, [open, trackIds]);

  async function createPreview() {
    if (!mediaIds.length) return;
    setBusy(true);
    try {
      const fields: TagField[] = ["title", "artists", "album", "albumArtist", "genre"];
      let set:
        | Partial<{
            title: string;
            artists: string[];
            album: string;
            albumArtist: string;
            genre: string;
          }>
        | undefined;
      let transforms: TagTransform[] = [];
      if (mode === "set") {
        set = field === "artists" ? { artists: splitArtists(value) } : { [field]: value };
      } else if (mode === "find") {
        transforms = [{ kind: "findReplace", fields: [field], find: value, replacement }];
      } else if (mode === "regex") {
        transforms = [{ kind: "regexReplace", fields: [field], pattern: value, replacement }];
      } else if (mode === "simplify") {
        transforms = [{ kind: "traditionalToSimplified", fields }];
      } else if (mode === "punctuation") {
        transforms = [{ kind: "normalizePunctuation", fields }];
      } else if (mode === "filename") {
        transforms = [{ kind: "filenameToTags" }];
      } else {
        transforms = [{ kind: "trim", fields }];
      }
      setOperation(await previewTags({ mediaIds, set, transforms }).send());
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await applyTags(operation.id).send());
      toast.success("批量 Tag 操作已执行");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  async function undo() {
    if (!operation) return;
    setBusy(true);
    try {
      setOperation(await undoTags(operation.id).send());
      toast.success("批量 Tag 操作已撤销");
      await onChanged();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  }

  const needsField = ["set", "find", "regex"].includes(mode);
  const needsValue = ["set", "find", "regex"].includes(mode);
  const needsReplacement = ["find", "regex"].includes(mode);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[88vh] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>批量 Tag 编辑</DialogTitle>
          <DialogDescription>
            已选择 {trackIds.length} 首逻辑曲目、{mediaIds.length} 个可写物理文件。先生成
            Diff，再确认写入。
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 sm:grid-cols-2">
          <Field>
            <FieldLabel>处理方式</FieldLabel>
            <Select value={mode} onValueChange={(next) => setMode(next as BatchMode)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="set">仅设置填写字段</SelectItem>
                <SelectItem value="find">查找 / 替换</SelectItem>
                <SelectItem value="regex">正则替换</SelectItem>
                <SelectItem value="simplify">繁体转简体</SelectItem>
                <SelectItem value="punctuation">统一中西文标点</SelectItem>
                <SelectItem value="trim">清理首尾空格</SelectItem>
                <SelectItem value="filename">从文件名解析</SelectItem>
              </SelectContent>
            </Select>
          </Field>
          {needsField ? (
            <Field>
              <FieldLabel>字段</FieldLabel>
              <Select value={field} onValueChange={(next) => setField(next as TagField)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="title">标题</SelectItem>
                  <SelectItem value="artists">歌手</SelectItem>
                  <SelectItem value="album">专辑</SelectItem>
                  <SelectItem value="albumArtist">专辑歌手</SelectItem>
                  <SelectItem value="genre">流派</SelectItem>
                </SelectContent>
              </Select>
            </Field>
          ) : null}
          {needsValue ? (
            <Field>
              <FieldLabel htmlFor="batch-value">
                {mode === "regex" ? "正则表达式" : mode === "find" ? "查找内容" : "新值"}
              </FieldLabel>
              <Input
                id="batch-value"
                value={value}
                onChange={(event) => setValue(event.target.value)}
              />
              {mode === "regex" ? (
                <FieldDescription>正则在服务端校验；无效表达式不会生成操作。</FieldDescription>
              ) : null}
            </Field>
          ) : null}
          {needsReplacement ? (
            <Field>
              <FieldLabel htmlFor="batch-replacement">替换为</FieldLabel>
              <Input
                id="batch-replacement"
                value={replacement}
                onChange={(event) => setReplacement(event.target.value)}
              />
            </Field>
          ) : null}
        </div>
        <OperationPreview operation={operation} />
        <DialogFooter className="gap-2">
          {operation?.status === "completed" ? (
            <Button variant="outline" onClick={() => void undo()} disabled={busy}>
              撤销
            </Button>
          ) : null}
          <Button
            variant="outline"
            onClick={() => void createPreview()}
            disabled={busy || !mediaIds.length || (needsValue && !value)}
          >
            <WandSparkles data-icon="inline-start" />
            预览 Diff
          </Button>
          {operation?.status === "previewed" ? (
            <Button onClick={() => void apply()} disabled={busy}>
              {busy ? <Spinner data-icon="inline-start" /> : <Save data-icon="inline-start" />}
              确认写入
            </Button>
          ) : null}
          {!operation ? (
            <Button variant="ghost" disabled>
              <Tags data-icon="inline-start" />
              等待预览
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function splitArtists(value: string) {
  return value
    .split(/[;、&]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "批量操作失败");
}
