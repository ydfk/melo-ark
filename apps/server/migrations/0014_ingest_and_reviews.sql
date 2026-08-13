ALTER TABLE libraries
    ADD COLUMN target_library_id TEXT REFERENCES libraries(id) ON DELETE SET NULL;
ALTER TABLE libraries
    ADD COLUMN auto_ingest_enabled INTEGER NOT NULL DEFAULT 0 CHECK (auto_ingest_enabled IN (0, 1));

ALTER TABLE media_files
    ADD COLUMN available INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1));
ALTER TABLE media_files
    ADD COLUMN missing_since TEXT;

CREATE INDEX idx_libraries_target ON libraries(target_library_id);
CREATE INDEX idx_media_files_available ON media_files(available, library_id);

CREATE TABLE ingest_records (
    id TEXT PRIMARY KEY NOT NULL,
    source_media_file_id TEXT NOT NULL UNIQUE REFERENCES media_files(id) ON DELETE CASCADE,
    target_library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    target_media_file_id TEXT REFERENCES media_files(id) ON DELETE SET NULL,
    job_id TEXT UNIQUE REFERENCES jobs(id) ON DELETE SET NULL,
    stage TEXT NOT NULL CHECK (stage IN (
        'pending', 'linking', 'indexed', 'matching', 'analyzing',
        'reviewing', 'completed', 'failed'
    )),
    target_relative_path TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX idx_ingest_records_stage ON ingest_records(stage, updated_at);
CREATE INDEX idx_ingest_records_target ON ingest_records(target_library_id);

CREATE TABLE review_items (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'resolved', 'ignored')),
    marked INTEGER NOT NULL DEFAULT 0 CHECK (marked IN (0, 1)),
    title TEXT NOT NULL,
    detail TEXT NOT NULL,
    subject_key TEXT NOT NULL,
    track_id TEXT REFERENCES tracks(id) ON DELETE CASCADE,
    media_file_id TEXT REFERENCES media_files(id) ON DELETE CASCADE,
    library_id TEXT REFERENCES libraries(id) ON DELETE CASCADE,
    confidence REAL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(kind, subject_key)
);

CREATE INDEX idx_review_items_status_marked ON review_items(status, marked, updated_at DESC);
CREATE INDEX idx_review_items_kind ON review_items(kind, updated_at DESC);

CREATE TABLE review_batch_previews (
    id TEXT PRIMARY KEY NOT NULL,
    rule TEXT NOT NULL,
    review_ids_json TEXT NOT NULL,
    items_json TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    applied_at TEXT
);

CREATE INDEX idx_review_batch_previews_expiry ON review_batch_previews(expires_at);

INSERT INTO review_items (
    id, kind, title, detail, subject_key, track_id, media_file_id, library_id,
    payload_json, created_at, updated_at
)
SELECT
    randomblob(16), 'source_missing', '来源文件不可用',
    mf.relative_path, mf.id, mf.track_id, mf.id, mf.library_id, '{}',
    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
FROM media_files mf
WHERE mf.available = 0
ON CONFLICT(kind, subject_key) DO NOTHING;
