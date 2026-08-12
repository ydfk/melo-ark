ALTER TABLE jobs ADD COLUMN source_type TEXT;
ALTER TABLE jobs ADD COLUMN source_id TEXT;

UPDATE libraries SET name = path;
UPDATE jobs
SET source_type = 'library', source_id = library_id
WHERE kind = 'scan' AND library_id IS NOT NULL;
UPDATE jobs
SET source_type = 'operation', source_id = id
WHERE kind IN ('tag_edit', 'organize', 'trash');
UPDATE jobs
SET source_type = 'workspace', source_id = 'library'
WHERE kind = 'scrape';
UPDATE jobs
SET source_type = 'workspace', source_id = 'duplicates'
WHERE kind = 'analyze';

DELETE FROM jobs WHERE kind = 'scan' AND library_id IS NULL;

CREATE TABLE job_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    level TEXT NOT NULL CHECK (level IN ('info', 'warn', 'error')),
    event_type TEXT NOT NULL,
    item_key TEXT,
    attempt INTEGER,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_job_logs_job_id_id ON job_logs(job_id, id DESC);
CREATE INDEX idx_job_logs_finished_cleanup ON job_logs(created_at, job_id);
