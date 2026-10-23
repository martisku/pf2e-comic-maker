-- Expand in place so an old server can continue writing row 1 during rollout.
ALTER TABLE workspace DROP CONSTRAINT workspace_id_check;
ALTER TABLE workspace ADD COLUMN project_id UUID NOT NULL
    DEFAULT '00000000-0000-4000-8000-000000000001' UNIQUE;
ALTER TABLE workspace ADD COLUMN description TEXT NOT NULL DEFAULT '';
-- Keep original creation content separate from mutable metadata for UUID replay.
ALTER TABLE workspace ADD COLUMN creation JSONB;
CREATE SEQUENCE workspace_ids START WITH 2 OWNED BY workspace.id;
SELECT setval('workspace_ids', GREATEST(2, COALESCE((SELECT max(id) + 1 FROM workspace), 2)), false);
ALTER TABLE workspace ALTER COLUMN id SET DEFAULT nextval('workspace_ids');

ALTER TABLE image_uploads ADD COLUMN project_id UUID NOT NULL
    DEFAULT '00000000-0000-4000-8000-000000000001'
    REFERENCES workspace(project_id);
CREATE INDEX image_uploads_project ON image_uploads(project_id);
