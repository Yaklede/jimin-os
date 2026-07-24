-- Projects with a defined finish line and continuously operated projects need
-- different health signals. Existing projects retain completion-mode behavior
-- until the owner explicitly changes their management mode.
ALTER TABLE projects
    ADD COLUMN management_mode TEXT NOT NULL DEFAULT 'completion'
        CHECK (management_mode IN ('completion', 'operation')),
    ADD COLUMN reporting_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN stale_threshold_days SMALLINT NOT NULL DEFAULT 7
        CHECK (stale_threshold_days BETWEEN 1 AND 90);

CREATE INDEX projects_user_management_mode_idx
    ON projects (user_id, management_mode, status, updated_at DESC);

UPDATE jimin_schema_metadata
SET schema_version = 35
WHERE singleton = TRUE;
