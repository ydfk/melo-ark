CREATE TABLE embedded_metadata_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    media_file_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mtime_ms INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    inode TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_metadata_snapshots_media_created
    ON embedded_metadata_snapshots(media_file_id, created_at DESC);

CREATE TABLE operations (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('tag_edit', 'organize', 'trash')),
    status TEXT NOT NULL CHECK (status IN (
        'previewed', 'running', 'completed', 'completed_with_errors', 'failed', 'rolled_back'
    )),
    request_json TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    confirmed_at TEXT,
    finished_at TEXT,
    rolled_back_at TEXT
);

CREATE TABLE operation_items (
    id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL REFERENCES operations(id) ON DELETE CASCADE,
    media_file_id TEXT REFERENCES media_files(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('previewed', 'running', 'success', 'failed', 'rolled_back')),
    before_json TEXT,
    after_json TEXT,
    source_path TEXT,
    target_path TEXT,
    preflight_json TEXT,
    error_message TEXT,
    retryable INTEGER NOT NULL DEFAULT 0 CHECK (retryable IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_operation_items_operation_status
    ON operation_items(operation_id, status);
