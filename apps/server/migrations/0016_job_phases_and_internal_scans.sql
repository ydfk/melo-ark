ALTER TABLE jobs ADD COLUMN phase TEXT;
ALTER TABLE jobs ADD COLUMN phase_processed_items INTEGER NOT NULL DEFAULT 0;
ALTER TABLE jobs ADD COLUMN phase_total_items INTEGER;
ALTER TABLE jobs ADD COLUMN internal INTEGER NOT NULL DEFAULT 0 CHECK (internal IN (0, 1));

UPDATE jobs
SET phase = CASE
        WHEN kind = 'scan' THEN 'scanning'
        WHEN kind = 'ingest' THEN 'processing'
        ELSE NULL
    END,
    phase_processed_items = processed_items,
    phase_total_items = total_items
WHERE status IN ('queued', 'running', 'paused', 'cancel_requested', 'interrupted');

UPDATE jobs AS target_scan
SET parent_job_id = (
        SELECT ingest_job.id
        FROM jobs AS ingest_job
        JOIN ingest_records AS ingest_record ON ingest_record.job_id = ingest_job.id
        WHERE ingest_job.kind = 'ingest'
          AND ingest_record.target_library_id = target_scan.library_id
          AND ingest_job.created_at <= target_scan.created_at
          AND COALESCE(ingest_job.finished_at, ingest_job.updated_at) >= target_scan.created_at
        GROUP BY ingest_job.id
        ORDER BY ingest_job.created_at DESC
        LIMIT 1
    ),
    internal = 1
WHERE target_scan.kind = 'scan'
  AND target_scan.parent_job_id IS NULL
  AND (
      SELECT COUNT(DISTINCT ingest_job.id)
      FROM jobs AS ingest_job
      JOIN ingest_records AS ingest_record ON ingest_record.job_id = ingest_job.id
      WHERE ingest_job.kind = 'ingest'
        AND ingest_record.target_library_id = target_scan.library_id
        AND ingest_job.created_at <= target_scan.created_at
        AND COALESCE(ingest_job.finished_at, ingest_job.updated_at) >= target_scan.created_at
  ) = 1;

UPDATE jobs
SET phase_processed_items = (
        SELECT COUNT(*)
        FROM ingest_records
        WHERE ingest_records.job_id = jobs.id
          AND stage IN ('completed', 'failed')
    ),
    phase_total_items = total_items
WHERE kind = 'ingest'
  AND status IN ('queued', 'running', 'paused', 'cancel_requested', 'interrupted');

