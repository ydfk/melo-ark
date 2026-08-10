ALTER TABLE media_files ADD COLUMN full_hash TEXT;
ALTER TABLE media_files ADD COLUMN fingerprint_json TEXT;
ALTER TABLE media_files ADD COLUMN fingerprint_duration_ms INTEGER;
ALTER TABLE media_files ADD COLUMN quality_score INTEGER;
ALTER TABLE media_files ADD COLUMN analysis_error TEXT;

CREATE INDEX idx_media_files_full_hash ON media_files(full_hash) WHERE full_hash IS NOT NULL;

CREATE TABLE analysis_jobs (
    job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    calculate_hash INTEGER NOT NULL CHECK (calculate_hash IN (0, 1)),
    calculate_fingerprint INTEGER NOT NULL CHECK (calculate_fingerprint IN (0, 1)),
    created_at TEXT NOT NULL
);

CREATE TABLE duplicate_groups (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('hardlink_alias', 'binary_exact', 'audio_duplicate', 'quality_variant', 'possible_duplicate')),
    confidence INTEGER NOT NULL,
    reclaimable_bytes INTEGER NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    analysis_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE duplicate_group_members (
    group_id TEXT NOT NULL REFERENCES duplicate_groups(id) ON DELETE CASCADE,
    media_file_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    similarity REAL,
    quality_score INTEGER NOT NULL,
    recommended_keep INTEGER NOT NULL DEFAULT 0 CHECK (recommended_keep IN (0, 1)),
    PRIMARY KEY (group_id, media_file_id)
);
CREATE INDEX idx_duplicate_members_media ON duplicate_group_members(media_file_id);

CREATE TABLE ai_recommendations (
    id TEXT PRIMARY KEY NOT NULL,
    subject_kind TEXT NOT NULL CHECK (subject_kind IN ('duplicate_group', 'scrape_candidates')),
    subject_id TEXT NOT NULL,
    relation TEXT NOT NULL,
    confidence REAL NOT NULL,
    reason TEXT NOT NULL,
    request_json TEXT NOT NULL,
    response_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
