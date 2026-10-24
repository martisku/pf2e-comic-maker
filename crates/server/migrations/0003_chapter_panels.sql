-- Stable project-wide panel identities may grow beyond the prototype's four.
-- JSON workspace validation and the monotonic counter enforce upper bounds and
-- non-reuse; PostgreSQL INTEGER already enforces the signed upper bound.
ALTER TABLE image_uploads DROP CONSTRAINT image_uploads_panel_id_check;
ALTER TABLE image_uploads ADD CONSTRAINT image_uploads_panel_id_check CHECK (panel_id > 0);
