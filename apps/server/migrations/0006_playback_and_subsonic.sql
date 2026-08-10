ALTER TABLE users ADD COLUMN subsonic_secret TEXT;

CREATE TABLE favorites (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, track_id)
);

CREATE TABLE play_history (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    media_file_id TEXT REFERENCES media_files(id) ON DELETE SET NULL,
    client TEXT NOT NULL,
    played_at TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1))
);
CREATE INDEX idx_play_history_user_time ON play_history(user_id, played_at DESC);

CREATE TABLE now_playing (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    media_file_id TEXT REFERENCES media_files(id) ON DELETE SET NULL,
    client TEXT NOT NULL,
    position_sec INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE TABLE playlists (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    comment TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE playlist_tracks (
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX idx_playlist_tracks_track ON playlist_tracks(track_id);

CREATE TABLE transcode_cache (
    cache_key TEXT PRIMARY KEY NOT NULL,
    media_file_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    profile TEXT NOT NULL,
    path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    last_accessed_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
