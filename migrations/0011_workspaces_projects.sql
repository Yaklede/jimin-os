-- Work is owned by a personal or company workspace. The two scopes share a
-- calendar only at the availability level; project records never cross scope.
CREATE TABLE workspaces (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('personal', 'company')),
    name TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 80),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, scope)
);

CREATE INDEX workspaces_user_scope_idx ON workspaces (user_id, scope);

CREATE TABLE projects (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE RESTRICT,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    objective TEXT NULL CHECK (objective IS NULL OR char_length(objective) <= 10000),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed')),
    risk_level SMALLINT NOT NULL DEFAULT 0 CHECK (risk_level BETWEEN 0 AND 3),
    next_action TEXT NULL CHECK (next_action IS NULL OR char_length(next_action) <= 500),
    due_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (id, user_id)
);

CREATE INDEX projects_user_workspace_status_idx
    ON projects (user_id, workspace_id, status, due_at NULLS LAST, updated_at DESC);

ALTER TABLE tasks
    ADD COLUMN project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL;

CREATE INDEX tasks_project_open_idx
    ON tasks (project_id, priority DESC, due_at NULLS LAST, created_at)
    WHERE status = 'open';

CREATE TRIGGER workspaces_set_updated_at
BEFORE UPDATE ON workspaces
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER projects_set_updated_at
BEFORE UPDATE ON projects
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 11
WHERE singleton = TRUE;
