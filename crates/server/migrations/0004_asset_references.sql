CREATE TABLE reference_uploads (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES workspace(project_id),
    asset_id UUID NOT NULL,
    expected_revision BIGINT NOT NULL CHECK (expected_revision >= 0),
    digest TEXT NOT NULL,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL CHECK (byte_size BETWEEN 1 AND 20000000),
    object_key TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'abandoned')),
    touched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX reference_uploads_cleanup ON reference_uploads (state, touched_at);
CREATE INDEX reference_uploads_project ON reference_uploads (project_id);
