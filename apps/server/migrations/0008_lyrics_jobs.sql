CREATE TABLE lyrics_jobs (
    job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    request_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
