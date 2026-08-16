import { alovaInstance } from "@/lib/api";
import type {
  Job,
  ReviewBatchItemPage,
  ReviewBatchPreview,
  ReviewBatchRule,
  ReviewItem,
  ReviewKind,
  ReviewPage,
  ReviewStatus,
} from "@/lib/api/types";

export const getReviews = (query: {
  status?: ReviewStatus;
  kind?: ReviewKind;
  marked?: boolean;
  page?: number;
  perPage?: number;
}) => alovaInstance.Get<ReviewPage>("/reviews", { params: query });

export const updateReview = (id: string, request: { marked?: boolean; status?: ReviewStatus }) =>
  alovaInstance.Patch<ReviewItem>(`/reviews/${id}`, request);

export const clearReviewMarks = (status: ReviewStatus, kind?: ReviewKind) =>
  alovaInstance.Post<{ count: number }>("/reviews/marks/clear", { status, kind });

export const previewReviewBatch = (
  selection: { status: ReviewStatus; kind?: ReviewKind },
  rule: ReviewBatchRule
) => alovaInstance.Post<ReviewBatchPreview>("/reviews/batch/preview", { selection, rule });

export const getReviewBatchPreviewItems = (id: string, page: number, perPage = 25) =>
  alovaInstance.Get<ReviewBatchItemPage>(`/reviews/batch/previews/${id}/items`, {
    params: { page, perPage },
  });

export const applyReviewBatch = (previewId: string) =>
  alovaInstance.Post<Job>("/reviews/batch/apply", {
    previewId,
    confirmation: "APPLY",
  });
