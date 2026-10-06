-- First-class project reports keep generated writing separate from chat
-- messages. The first supported document is a project weekly report.
CREATE TABLE reports (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    report_type TEXT NOT NULL CHECK (report_type = 'project_weekly'),
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (
        status IN ('draft', 'finalized', 'archived', 'failed')
    ),
    current_version BIGINT NOT NULL DEFAULT 1 CHECK (current_version > 0),
    generated_at TIMESTAMPTZ NOT NULL,
    finalized_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (period_start < period_end),
    CHECK (finalized_at IS NULL OR finalized_at >= generated_at),
    UNIQUE (user_id, workspace_id, project_id, report_type, period_start, period_end)
);

CREATE INDEX reports_project_history_idx
    ON reports (user_id, workspace_id, project_id, period_start DESC, updated_at DESC);

CREATE TABLE report_versions (
    id UUID PRIMARY KEY,
    report_id UUID NOT NULL REFERENCES reports (id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    content JSONB NOT NULL CHECK (jsonb_typeof(content) = 'object'),
    generated_by TEXT NOT NULL CHECK (generated_by IN ('system', 'assistant', 'user')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (report_id, version)
);

CREATE INDEX report_versions_history_idx
    ON report_versions (report_id, version DESC);

CREATE TRIGGER reports_set_updated_at
BEFORE UPDATE ON reports
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 55
WHERE singleton = TRUE;
