ALTER TABLE jobs ADD COLUMN parent_job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL;

CREATE INDEX idx_jobs_parent_kind ON jobs(parent_job_id, kind, created_at DESC);

UPDATE jobs AS ingest_job
SET parent_job_id = (
    SELECT scan_job.id
    FROM jobs AS scan_job
    WHERE scan_job.kind = 'scan'
      AND scan_job.library_id = ingest_job.library_id
      AND scan_job.created_at <= ingest_job.created_at
      AND COALESCE(scan_job.finished_at, scan_job.updated_at) >= ingest_job.created_at
    ORDER BY scan_job.created_at DESC
    LIMIT 1
)
WHERE ingest_job.kind = 'ingest'
  AND ingest_job.parent_job_id IS NULL;

ALTER TABLE ingest_records RENAME TO ingest_records_legacy;

CREATE TABLE ingest_records (
    id TEXT PRIMARY KEY NOT NULL,
    source_media_file_id TEXT NOT NULL UNIQUE REFERENCES media_files(id) ON DELETE CASCADE,
    target_library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    target_media_file_id TEXT REFERENCES media_files(id) ON DELETE SET NULL,
    job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL,
    stage TEXT NOT NULL CHECK (stage IN (
        'pending', 'linking', 'indexed', 'matching', 'analyzing',
        'reviewing', 'completed', 'failed'
    )),
    target_relative_path TEXT,
    last_error TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

INSERT INTO ingest_records (
    id, source_media_file_id, target_library_id, target_media_file_id, job_id,
    stage, target_relative_path, last_error, attempt_count, created_at, updated_at, completed_at
)
SELECT
    id, source_media_file_id, target_library_id, target_media_file_id, job_id,
    stage, target_relative_path, last_error, attempt_count, created_at, updated_at, completed_at
FROM ingest_records_legacy;

DROP TABLE ingest_records_legacy;

CREATE INDEX idx_ingest_records_stage ON ingest_records(stage, updated_at);
CREATE INDEX idx_ingest_records_target ON ingest_records(target_library_id);
CREATE INDEX idx_ingest_records_job ON ingest_records(job_id, stage);
