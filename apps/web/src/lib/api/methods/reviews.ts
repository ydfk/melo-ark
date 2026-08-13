import { alovaInstance } from "@/lib/api";
import type {
  Job,
  ReviewBatchPreview,
  ReviewBatchRule,
  ReviewItem,
  ReviewKind,
  ReviewPage,
  ReviewStatus,
} from "@/lib/api/types";

export const getReviews = (query: { status?: ReviewStatus; kind?: ReviewKind; marked?: boolean }) =>
  alovaInstance.Get<ReviewPage>("/reviews", { params: query });

export const updateReview = (id: string, request: { marked?: boolean; status?: ReviewStatus }) =>
  alovaInstance.Patch<ReviewItem>(`/reviews/${id}`, request);

export const previewReviewBatch = (reviewIds: string[], rule: ReviewBatchRule) =>
  alovaInstance.Post<ReviewBatchPreview>("/reviews/batch/preview", { reviewIds, rule });

export const applyReviewBatch = (previewId: string) =>
  alovaInstance.Post<Job>("/reviews/batch/apply", {
    previewId,
    confirmation: "APPLY",
  });
