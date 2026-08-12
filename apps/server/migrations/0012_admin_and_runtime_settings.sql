ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0
    CHECK (must_change_password IN (0, 1));

CREATE TABLE runtime_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    settings_json TEXT NOT NULL,
    ai_api_key_ciphertext TEXT,
    updated_at TEXT NOT NULL
);
