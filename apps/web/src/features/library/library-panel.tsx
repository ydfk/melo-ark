import { zodResolver } from "@hookform/resolvers/zod";
import { FolderSearch, Plus } from "lucide-react";
import { useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { toast } from "sonner";
import { z } from "zod";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { LibraryGroupCard } from "@/features/library/library-root-card";
import { DirectoryTreePicker } from "@/features/library/directory-tree-picker";
import { useJobActivity } from "@/features/tasks/job-activity-context";
import { ApiError } from "@/lib/api";
import { createLibrary, preflightLibraryPath, scanLibrary } from "@/lib/api/methods/library";
import type { LibraryGroup, LibrarySource } from "@/lib/api/types";

const librarySchema = z.object({
  sourcePath: z.string().trim().min(1, "请选择来源目录"),
  organizedPath: z.string().trim().min(1, "请选择整理后目录"),
  watchEnabled: z.boolean(),
  autoIngestEnabled: z.boolean(),
});

type LibraryForm = z.infer<typeof librarySchema>;

type LibraryPanelProps = {
  libraries: LibraryGroup[];
  onChanged: () => Promise<void>;
};

export function LibraryPanel({ libraries, onChanged }: LibraryPanelProps) {
  const { registerJob } = useJobActivity();
  const [dialogOpen, setDialogOpen] = useState(false);
  const form = useForm<LibraryForm>({
    resolver: zodResolver(librarySchema),
    defaultValues: {
      sourcePath: "",
      organizedPath: "",
      watchEnabled: false,
      autoIngestEnabled: true,
    },
  });

  const submit = form.handleSubmit(async (values) => {
    try {
      const [source, organized] = await Promise.all([
        preflightLibraryPath(values.sourcePath).send(),
        preflightLibraryPath(values.organizedPath).send(),
      ]);
      await createLibrary({
        sourcePath: source.canonicalPath,
        organizedPath: organized.canonicalPath,
        watchEnabled: values.watchEnabled,
        autoIngestEnabled: values.autoIngestEnabled,
      }).send();
      toast.success("曲库已添加");
      form.reset({
        sourcePath: "",
        organizedPath: "",
        watchEnabled: false,
        autoIngestEnabled: true,
      });
      setDialogOpen(false);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法添加曲库");
    }
  });

  async function startScan(library: LibrarySource) {
    try {
      registerJob(await scanLibrary(library.id).send());
      toast.success(`已开始扫描「${library.sourcePath}」`);
      await onChanged();
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "无法创建扫描任务");
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.24em] text-primary">Ingest Flow</p>
          <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">曲库接入</h1>
        </div>
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          {libraries.length ? (
            <DialogTrigger asChild>
              <Button>
                <Plus data-icon="inline-start" />
                添加曲库
              </Button>
            </DialogTrigger>
          ) : null}
          <DialogContent>
            <DialogHeader>
              <DialogTitle>添加音乐目录</DialogTitle>
              <DialogDescription>选择音乐来源和整理后的播放目录。</DialogDescription>
            </DialogHeader>
            <form id="library-form" onSubmit={submit}>
              <FieldGroup>
                <Controller
                  control={form.control}
                  name="sourcePath"
                  render={({ field }) => (
                    <Field data-invalid={Boolean(form.formState.errors.sourcePath)}>
                      <FieldLabel>来源目录</FieldLabel>
                      <DirectoryTreePicker value={field.value} onChange={field.onChange} />
                      <FieldError errors={[form.formState.errors.sourcePath]} />
                    </Field>
                  )}
                />
                <Controller
                  control={form.control}
                  name="organizedPath"
                  render={({ field }) => (
                    <Field data-invalid={Boolean(form.formState.errors.organizedPath)}>
                      <FieldLabel>整理后目录</FieldLabel>
                      <DirectoryTreePicker value={field.value} onChange={field.onChange} />
                      <FieldDescription>播放、搜索和专辑只使用此目录中的文件。</FieldDescription>
                      <FieldError errors={[form.formState.errors.organizedPath]} />
                    </Field>
                  )}
                />
                <Controller
                  control={form.control}
                  name="autoIngestEnabled"
                  render={({ field }) => (
                    <Field orientation="horizontal">
                      <div className="flex-1">
                        <FieldLabel htmlFor="auto-ingest-enabled">自动处理新增音乐</FieldLabel>
                        <FieldDescription>
                          扫描到新增文件后创建硬链接并加入待处理。
                        </FieldDescription>
                      </div>
                      <Switch
                        id="auto-ingest-enabled"
                        checked={field.value}
                        onCheckedChange={field.onChange}
                      />
                    </Field>
                  )}
                />
                <Controller
                  control={form.control}
                  name="watchEnabled"
                  render={({ field }) => (
                    <Field orientation="horizontal">
                      <div className="flex-1">
                        <FieldLabel htmlFor="watch-enabled">文件监听</FieldLabel>
                      </div>
                      <Switch
                        id="watch-enabled"
                        checked={field.value}
                        onCheckedChange={field.onChange}
                      />
                    </Field>
                  )}
                />
              </FieldGroup>
            </form>
            <DialogFooter>
              <Button variant="outline" onClick={() => setDialogOpen(false)}>
                取消
              </Button>
              <Button form="library-form" type="submit" disabled={form.formState.isSubmitting}>
                {form.formState.isSubmitting ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <FolderSearch data-icon="inline-start" />
                )}
                预检并添加
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>

      {libraries.length ? (
        <section className="grid gap-4">
          {libraries.map((library) => (
            <LibraryGroupCard
              key={library.organizedLibraryId ?? library.sources[0]?.id}
              library={library}
              onScan={startScan}
              onChanged={onChanged}
            />
          ))}
        </section>
      ) : (
        <Alert className="flex min-h-52 flex-col items-center justify-center gap-4 text-center">
          <FolderSearch />
          <AlertTitle>尚未添加曲库</AlertTitle>
          <AlertDescription>选择来源目录和整理后目录即可开始。</AlertDescription>
          <Button onClick={() => setDialogOpen(true)}>
            <Plus data-icon="inline-start" />
            添加第一个曲库
          </Button>
        </Alert>
      )}
    </div>
  );
}
