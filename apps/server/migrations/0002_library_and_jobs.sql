CREATE TABLE libraries (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    scan_enabled INTEGER NOT NULL DEFAULT 1 CHECK (scan_enabled IN (0, 1)),
    watch_enabled INTEGER NOT NULL DEFAULT 0 CHECK (watch_enabled IN (0, 1)),
    writable INTEGER NOT NULL DEFAULT 0 CHECK (writable IN (0, 1)),
    role TEXT NOT NULL CHECK (role IN ('source', 'managed', 'both')),
    exclude_patterns TEXT NOT NULL DEFAULT '[]',
    last_scan_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE artists (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    sort_name TEXT,
    normalized_name TEXT NOT NULL UNIQUE
);

CREATE TABLE albums (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    album_artist TEXT,
    normalized_title TEXT NOT NULL,
    year INTEGER,
    cover_art_id TEXT,
    UNIQUE (normalized_title, album_artist, year)
);

CREATE TABLE tracks (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    normalized_title TEXT NOT NULL,
    album_id TEXT REFERENCES albums(id) ON DELETE SET NULL,
    track_no INTEGER,
    disc_no INTEGER,
    year INTEGER,
    genre TEXT,
    duration_ms INTEGER,
    version_label TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE track_artists (
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id TEXT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, artist_id)
);

CREATE TABLE media_files (
    id TEXT PRIMARY KEY NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    extension TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mtime_ms INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    inode TEXT NOT NULL,
    hardlink_count INTEGER NOT NULL,
    codec TEXT,
    container TEXT,
    duration_ms INTEGER,
    bitrate INTEGER,
    sample_rate INTEGER,
    bit_depth INTEGER,
    channels INTEGER,
    metadata_readable INTEGER NOT NULL DEFAULT 0,
    metadata_writable INTEGER NOT NULL DEFAULT 0,
    fingerprint_status TEXT NOT NULL DEFAULT 'pending',
    hash_status TEXT NOT NULL DEFAULT 'pending',
    scan_error TEXT,
    last_seen_scan_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (library_id, relative_path)
);

CREATE INDEX idx_media_files_track_id ON media_files(track_id);
CREATE INDEX idx_media_files_physical ON media_files(device_id, inode);
CREATE INDEX idx_media_files_incremental ON media_files(library_id, relative_path, file_size, mtime_ms, device_id, inode);
CREATE INDEX idx_media_files_last_seen ON media_files(library_id, last_seen_scan_id);
CREATE INDEX idx_tracks_album_id ON tracks(album_id);
CREATE INDEX idx_tracks_normalized_title ON tracks(normalized_title);

CREATE VIRTUAL TABLE track_search USING fts5(
    track_id UNINDEXED,
    media_id UNINDEXED,
    title,
    artist,
    album,
    path,
    normalized_text,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE jobs (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'queued', 'running', 'paused', 'cancel_requested', 'cancelled',
        'completed', 'completed_with_errors', 'failed', 'interrupted'
    )),
    library_id TEXT REFERENCES libraries(id) ON DELETE SET NULL,
    total_items INTEGER NOT NULL DEFAULT 0,
    processed_items INTEGER NOT NULL DEFAULT 0,
    success_items INTEGER NOT NULL DEFAULT 0,
    skipped_items INTEGER NOT NULL DEFAULT 0,
    failed_items INTEGER NOT NULL DEFAULT 0,
    current_item TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE job_items (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    item_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'skipped', 'failed')),
    error_code TEXT,
    message TEXT,
    retryable INTEGER NOT NULL DEFAULT 0,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    UNIQUE (job_id, item_key)
);

CREATE INDEX idx_jobs_status_created ON jobs(status, created_at DESC);
CREATE INDEX idx_job_items_job_status ON job_items(job_id, status);
