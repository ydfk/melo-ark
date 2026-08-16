import { CheckCheck, CircleAlert } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { getReviewBatchPreviewItems } from "@/lib/api/methods/reviews";
import type { ReviewBatchItemPage, ReviewBatchPreview } from "@/lib/api/types";

import { ReviewPagination } from "./review-pagination";

export function ReviewPreviewDialog({
  preview,
  applying,
  onClose,
  onApply,
}: {
  preview?: ReviewBatchPreview;
  applying: boolean;
  onClose: () => void;
  onApply: () => void;
}) {
  const [page, setPage] = useState(1);
  const [items, setItems] = useState<ReviewBatchItemPage>();

  useEffect(() => {
    setPage(1);
  }, [preview?.id]);

  useEffect(() => {
    if (!preview) return;
    setItems(undefined);
    void getReviewBatchPreviewItems(preview.id, page).send().then(setItems);
  }, [page, preview]);

  return (
    <Dialog
      open={Boolean(preview)}
      onOpenChange={(open) => {
        if (!open) {
          setPage(1);
          onClose();
        }
      }}
    >
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>批量处理预览</DialogTitle>
          <DialogDescription>仅可执行项目会进入任务，受阻项目保持原状。</DialogDescription>
        </DialogHeader>
        {preview ? (
          <div className="space-y-4">
            <div className="grid grid-cols-3 gap-3 text-center">
              <PreviewCount label="选择" value={preview.totalItems} />
              <PreviewCount label="可执行" value={preview.eligibleItems} />
              <PreviewCount label="受阻" value={preview.blockedItems} />
            </div>
            {!items ? (
              <div className="flex justify-center py-12">
                <Spinner />
              </div>
            ) : (
              <>
                <div className="space-y-2">
                  {items.items.map((item) => (
                    <div key={item.reviewId} className="rounded-xl border p-3 text-sm">
                      <div className="flex items-center gap-2">
                        {item.eligible ? (
                          <CheckCheck className="size-4 text-emerald-500" />
                        ) : (
                          <CircleAlert className="size-4 text-amber-500" />
                        )}
                        <span className="font-medium">{item.title}</span>
                      </div>
                      {item.reason ? (
                        <p className="mt-1 whitespace-pre-line pl-6 text-xs text-muted-foreground">
                          {item.reason}
                        </p>
                      ) : null}
                    </div>
                  ))}
                </div>
                <ReviewPagination
                  page={items.page}
                  perPage={items.perPage}
                  total={items.total}
                  onPageChange={setPage}
                  onPerPageChange={() => undefined}
                  showPageSize={false}
                />
              </>
            )}
          </div>
        ) : null}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
          <Button disabled={!preview?.eligibleItems || applying} onClick={onApply}>
            {applying ? <Spinner data-icon="inline-start" /> : null}
            确认创建任务
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PreviewCount({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-xl border bg-muted/40 p-3">
      <p className="text-2xl font-semibold">{value}</p>
      <p className="text-xs text-muted-foreground">{label}</p>
    </div>
  );
}
