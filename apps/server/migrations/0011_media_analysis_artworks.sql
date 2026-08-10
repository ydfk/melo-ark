CREATE TABLE audio_hashes (
    media_file_id TEXT PRIMARY KEY NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    blake3 TEXT NOT NULL,
    calculated_at TEXT NOT NULL,
    source_size INTEGER NOT NULL,
    source_mtime INTEGER NOT NULL
);

CREATE TABLE audio_fingerprints (
    media_file_id TEXT PRIMARY KEY NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    algorithm TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    duration_ms INTEGER,
    calculated_at TEXT NOT NULL
);

CREATE TABLE artworks (
    id TEXT PRIMARY KEY NOT NULL,
    media_file_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    mime_type TEXT,
    width INTEGER,
    height INTEGER,
    source TEXT NOT NULL,
    cache_path TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(media_file_id, kind)
);

INSERT INTO audio_hashes (media_file_id, blake3, calculated_at, source_size, source_mtime)
SELECT id, full_hash, updated_at, file_size, mtime_ms
FROM media_files WHERE full_hash IS NOT NULL;

INSERT INTO audio_fingerprints
    (media_file_id, algorithm, fingerprint, duration_ms, calculated_at)
SELECT id, 'chromaprint', fingerprint_json, fingerprint_duration_ms, updated_at
FROM media_files WHERE fingerprint_json IS NOT NULL;

INSERT INTO artworks (id, media_file_id, kind, source, created_at)
SELECT lower(hex(randomblob(16))), id, 'front', 'embedded', updated_at
FROM media_files WHERE has_artwork = 1;
