CREATE INDEX idx_review_items_paged
    ON review_items(status, kind, marked DESC, updated_at DESC, id DESC);

CREATE TABLE review_batch_preview_items (
    id TEXT PRIMARY KEY NOT NULL,
    preview_id TEXT NOT NULL REFERENCES review_batch_previews(id) ON DELETE CASCADE,
    review_id TEXT NOT NULL REFERENCES review_items(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    title TEXT NOT NULL,
    eligible INTEGER NOT NULL CHECK (eligible IN (0, 1)),
    reason TEXT,
    UNIQUE(preview_id, review_id)
);

CREATE INDEX idx_review_batch_preview_items_page
    ON review_batch_preview_items(preview_id, position);
