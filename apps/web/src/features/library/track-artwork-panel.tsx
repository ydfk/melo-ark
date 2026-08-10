import { ImagePlus } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { CoverArtwork } from "@/features/library/cover-artwork";
import type { MediaFile } from "@/lib/api/types";

export function TrackArtworkPanel({
  files,
  pending,
  onChange,
}: {
  files: MediaFile[];
  pending: boolean;
  onChange: (value?: string) => void;
}) {
  return (
    <div className="space-y-5">
      <div className="grid gap-4 sm:grid-cols-2">
        {files.map((file) => (
          <div key={file.id} className="overflow-hidden rounded-2xl border bg-card/60">
            <CoverArtwork
              mediaId={file.id}
              hasArtwork={file.hasArtwork}
              alt={`${file.path}封面`}
              className="aspect-square w-full rounded-none"
            />
            <div className="flex items-center justify-between gap-3 p-3">
              <p className="min-w-0 truncate font-mono text-xs text-muted-foreground">
                {file.path}
              </p>
              <Badge variant={file.hasArtwork ? "outline" : "secondary"}>
                {file.hasArtwork ? "已嵌入" : "无封面"}
              </Badge>
            </div>
          </div>
        ))}
      </div>

      <Field className="rounded-xl border p-4">
        <FieldLabel htmlFor="artwork-upload">替换所有可写变体的封面</FieldLabel>
        <Input
          id="artwork-upload"
          type="file"
          accept="image/jpeg,image/png,image/webp"
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (!file) {
              onChange(undefined);
              return;
            }
            if (file.size > 10 * 1024 * 1024) {
              toast.error("封面不能超过 10 MiB");
              event.target.value = "";
              return;
            }
            const reader = new FileReader();
            reader.onload = () => onChange(String(reader.result).split(",", 2)[1]);
            reader.onerror = () => toast.error("无法读取封面文件");
            reader.readAsDataURL(file);
          }}
        />
        <FieldDescription>
          支持 JPEG、PNG、WebP；选择后仍需到 Tag 页生成 Diff 并确认。
        </FieldDescription>
      </Field>

      {pending ? (
        <Alert>
          <ImagePlus />
          <AlertTitle>新封面已进入草稿</AlertTitle>
          <AlertDescription>文件尚未被修改。前往 Tag 页检查 Diff 后再确认写入。</AlertDescription>
        </Alert>
      ) : null}
    </div>
  );
}
