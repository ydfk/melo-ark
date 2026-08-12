import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

import { subscribeToJobEvents } from "@/lib/api/events";
import { getJobs } from "@/lib/api/methods/jobs";
import type { Job, JobLog } from "@/lib/api/types";

import { JobLogSheet } from "./job-log-sheet";

type JobActivityContextValue = {
  jobs: Job[];
  registerJob: (job: Job) => void;
  latestJob: (sourceType: string, sourceId: string, kind?: string) => Job | undefined;
  openLogs: (jobId: string) => void;
  refreshJobs: () => Promise<void>;
};

const JobActivityContext = createContext<JobActivityContextValue>({
  jobs: [],
  registerJob: () => undefined,
  latestJob: () => undefined,
  openLogs: () => undefined,
  refreshJobs: async () => undefined,
});

export function JobActivityProvider({
  children,
  onTerminal,
}: {
  children: React.ReactNode;
  onTerminal: () => void;
}) {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [selectedJobId, setSelectedJobId] = useState<string>();
  const [liveLogs, setLiveLogs] = useState<Record<string, JobLog[]>>({});

  const registerJob = useCallback((job: Job) => {
    setJobs((current) => [job, ...current.filter((item) => item.id !== job.id)].slice(0, 200));
  }, []);

  const refreshJobs = useCallback(async () => {
    setJobs(await getJobs().send());
  }, []);

  useEffect(() => {
    void refreshJobs();
  }, [refreshJobs]);

  useEffect(() => {
    const controller = new AbortController();
    async function connect() {
      while (!controller.signal.aborted) {
        try {
          await subscribeToJobEvents(controller.signal, (event) => {
            if (event.event === "job.updated") {
              registerJob(event.job);
              if (
                ["completed", "completed_with_errors", "failed", "cancelled"].includes(
                  event.job.status
                )
              ) {
                onTerminal();
              }
              return;
            }
            setLiveLogs((current) => ({
              ...current,
              [event.log.jobId]: [...(current[event.log.jobId] ?? []), event.log].slice(-300),
            }));
          });
        } catch {
          if (!controller.signal.aborted) {
            await new Promise((resolve) => window.setTimeout(resolve, 2_000));
          }
        }
      }
    }
    void connect();
    return () => controller.abort();
  }, [onTerminal, registerJob]);

  const latestJob = useCallback(
    (sourceType: string, sourceId: string, kind?: string) =>
      jobs.find(
        (job) =>
          job.sourceType === sourceType && job.sourceId === sourceId && (!kind || job.kind === kind)
      ),
    [jobs]
  );
  const selectedJob = jobs.find((job) => job.id === selectedJobId);
  const value = useMemo(
    () => ({ jobs, registerJob, latestJob, openLogs: setSelectedJobId, refreshJobs }),
    [jobs, latestJob, refreshJobs, registerJob]
  );

  return (
    <JobActivityContext.Provider value={value}>
      {children}
      <JobLogSheet
        job={selectedJob}
        liveLogs={selectedJobId ? (liveLogs[selectedJobId] ?? []) : []}
        open={Boolean(selectedJobId)}
        onOpenChange={(open) => !open && setSelectedJobId(undefined)}
      />
    </JobActivityContext.Provider>
  );
}

export function useJobActivity() {
  return useContext(JobActivityContext);
}
