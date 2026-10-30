-- A restore identity is reserved before any destination workspace is visible.
CREATE TABLE project_restores (
    id UUID PRIMARY KEY,
    digest TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready')),
    cleaned_at TIMESTAMPTZ
);
-- Immutable candidate key plans commit before the publishing lock is acquired.
CREATE TABLE project_restore_attempts (
    id UUID PRIMARY KEY,
    restore_id UUID NOT NULL,
    keys JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('cleanup', 'ready'))
);
CREATE INDEX project_restore_attempts_parent ON project_restore_attempts(restore_id);
