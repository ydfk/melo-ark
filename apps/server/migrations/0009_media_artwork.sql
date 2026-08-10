ALTER TABLE media_files
    ADD COLUMN has_artwork INTEGER NOT NULL DEFAULT 0 CHECK (has_artwork IN (0, 1));
