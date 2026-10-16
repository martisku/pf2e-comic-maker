CREATE TABLE workspace (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision BIGINT NOT NULL CHECK (revision >= 0),
    data JSONB NOT NULL
);

-- An intent is committed before PUT. Never discard abandoned keys: a late PUT
-- after cleanup must still be discoverable by the next reconciliation run.
CREATE TABLE image_uploads (
    id UUID PRIMARY KEY,
    panel_id INTEGER NOT NULL CHECK (panel_id BETWEEN 1 AND 4),
    expected_revision BIGINT NOT NULL CHECK (expected_revision >= 0),
    digest TEXT NOT NULL,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL CHECK (byte_size BETWEEN 1 AND 20000000),
    object_key TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'abandoned')),
    touched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX image_uploads_cleanup ON image_uploads (state, touched_at);
