-- ITSM credentials are deployment-owned secrets. Projects record only whether
-- their Google Chat inflow may use the globally configured read-only client
-- and the non-secret parent project identifier used for boundary checks;
-- no origin, token, or authorization header is persisted in application data.
CREATE TABLE project_itsm_connections (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    itsm_project_id TEXT NOT NULL CHECK (
        char_length(itsm_project_id) BETWEEN 1 AND 20
        AND itsm_project_id ~ '^[1-9][0-9]{0,19}$'
    ),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    FOREIGN KEY (project_id, user_id)
        REFERENCES projects (id, user_id) ON DELETE CASCADE,
    UNIQUE (project_id)
);

CREATE INDEX project_itsm_connections_enabled_idx
    ON project_itsm_connections (project_id)
    WHERE enabled = TRUE;

CREATE TRIGGER project_itsm_connections_set_updated_at
BEFORE UPDATE ON project_itsm_connections
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 51
WHERE singleton = TRUE;
