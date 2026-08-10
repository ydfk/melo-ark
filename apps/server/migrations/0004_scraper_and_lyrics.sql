CREATE TABLE provider_settings (
    provider_id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('metadata', 'lyrics', 'both')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL,
    maturity TEXT NOT NULL CHECK (maturity IN ('stable', 'beta')),
    base_url TEXT,
    timeout_ms INTEGER NOT NULL DEFAULT 8000,
    rate_limit_ms INTEGER NOT NULL DEFAULT 1000,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    circuit_open_until TEXT,
    last_success_at TEXT,
    last_error TEXT,
    updated_at TEXT NOT NULL
);

INSERT INTO provider_settings
    (provider_id, display_name, kind, enabled, priority, maturity, base_url, rate_limit_ms, updated_at)
VALUES
    ('qq', 'QQ Music', 'both', 1, 10, 'stable', 'https://u.y.qq.com', 500, CURRENT_TIMESTAMP),
    ('netease', 'NetEase Cloud Music', 'both', 1, 20, 'stable', 'https://music.163.com', 500, CURRENT_TIMESTAMP),
    ('kugou', 'Kugou', 'both', 1, 30, 'stable', 'https://songsearch.kugou.com', 500, CURRENT_TIMESTAMP),
    ('kuwo', 'Kuwo', 'both', 0, 40, 'beta', NULL, 800, CURRENT_TIMESTAMP),
    ('migu', 'Migu', 'both', 0, 50, 'beta', NULL, 800, CURRENT_TIMESTAMP),
    ('musicbrainz', 'MusicBrainz', 'metadata', 1, 60, 'stable', 'https://musicbrainz.org', 1000, CURRENT_TIMESTAMP),
    ('external_lrc', 'LrcApi Compatible', 'lyrics', 0, 70, 'beta', NULL, 1000, CURRENT_TIMESTAMP)
ON CONFLICT(provider_id) DO NOTHING;

CREATE TABLE provider_cache (
    cache_key TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL REFERENCES provider_settings(provider_id) ON DELETE CASCADE,
    response_json TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_provider_cache_expiry ON provider_cache(provider_id, expires_at);

CREATE TABLE scrape_candidates (
    id TEXT PRIMARY KEY NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL REFERENCES provider_settings(provider_id),
    provider_item_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artists_json TEXT NOT NULL,
    album TEXT,
    duration_ms INTEGER,
    year INTEGER,
    track_no INTEGER,
    version_label TEXT,
    artwork_url TEXT,
    score INTEGER NOT NULL,
    confidence TEXT NOT NULL CHECK (confidence IN ('high', 'review', 'low')),
    differences_json TEXT NOT NULL,
    raw_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(track_id, provider_id, provider_item_id)
);
CREATE INDEX idx_scrape_candidates_track_score ON scrape_candidates(track_id, score DESC);

CREATE TABLE scrape_jobs (
    job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    provider_ids_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);

CREATE TABLE lyrics (
    id TEXT PRIMARY KEY NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    media_file_id TEXT REFERENCES media_files(id) ON DELETE SET NULL,
    provider_id TEXT,
    provider_item_id TEXT,
    format TEXT NOT NULL CHECK (format IN ('plain', 'lrc')),
    language TEXT,
    content TEXT NOT NULL,
    translated_content TEXT,
    synced INTEGER NOT NULL CHECK (synced IN (0, 1)),
    coverage_percent INTEGER NOT NULL DEFAULT 0,
    quality_score INTEGER NOT NULL DEFAULT 0,
    storage TEXT NOT NULL CHECK (storage IN ('candidate', 'external', 'embedded', 'both')),
    external_path TEXT,
    active INTEGER NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_lyrics_track_active ON lyrics(track_id, active, quality_score DESC);
