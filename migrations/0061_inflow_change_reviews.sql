CREATE TABLE inflow_change_reviews (
 id UUID PRIMARY KEY,
 user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 analysis_id UUID NOT NULL REFERENCES project_inflow_analyses(id) ON DELETE CASCADE,
 source_revision INTEGER NOT NULL CHECK (source_revision > 0),
 task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
 task_version BIGINT NOT NULL CHECK (task_version > 0),
 previous_notes TEXT,
 approved_notes TEXT,
 previous_due_at TIMESTAMPTZ,
 approved_due_at TIMESTAMPTZ,
 approved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (analysis_id, source_revision)
);
UPDATE jimin_schema_metadata SET schema_version = 61 WHERE singleton = TRUE;
