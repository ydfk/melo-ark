import { alovaInstance } from "@/lib/api";
import type { Job, JobLogPage } from "@/lib/api/types";

export const getJobs = () => alovaInstance.Get<Job[]>("/jobs", { params: { limit: 100 } });

export const pauseJob = (id: string) => alovaInstance.Post<Job>(`/jobs/${id}/pause`);

export const resumeJob = (id: string) => alovaInstance.Post<Job>(`/jobs/${id}/resume`);

export const cancelJob = (id: string) => alovaInstance.Post<Job>(`/jobs/${id}/cancel`);

export const retryFailedJob = (id: string) => alovaInstance.Post<Job>(`/jobs/${id}/retry-failed`);

export const getJobLogs = (id: string, before?: number, level?: string) =>
  alovaInstance.Get<JobLogPage>(`/jobs/${id}/logs`, {
    params: { before, level, limit: 200 },
  });
