CREATE TABLE trash_purges (
    id TEXT PRIMARY KEY NOT NULL,
    trash_operation_id TEXT NOT NULL UNIQUE REFERENCES operations(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('previewed', 'running', 'completed', 'completed_with_errors')),
    created_by TEXT NOT NULL REFERENCES users(id),
    total_items INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    confirmed_at TEXT,
    finished_at TEXT
);

CREATE TABLE trash_purge_items (
    id TEXT PRIMARY KEY NOT NULL,
    purge_id TEXT NOT NULL REFERENCES trash_purges(id) ON DELETE CASCADE,
    source_operation_item_id TEXT NOT NULL REFERENCES operation_items(id) ON DELETE CASCADE,
    library_id TEXT REFERENCES libraries(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    expected_size INTEGER NOT NULL,
    expected_device_id TEXT NOT NULL,
    expected_inode TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('previewed', 'success', 'failed')),
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (purge_id, source_operation_item_id)
);

CREATE INDEX idx_trash_purge_items_status ON trash_purge_items(purge_id, status);
