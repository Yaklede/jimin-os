-- The Codex runtime publishes its currently available model picker entries.
-- Users may either follow the runtime default or pin one available model.
CREATE TABLE agent_models (
    id TEXT PRIMARY KEY CHECK (char_length(btrim(id)) BETWEEN 1 AND 200),
    display_name TEXT NOT NULL CHECK (char_length(btrim(display_name)) BETWEEN 1 AND 200),
    description TEXT NOT NULL CHECK (char_length(description) <= 2000),
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    available BOOLEAN NOT NULL DEFAULT TRUE,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX agent_models_single_default_idx
    ON agent_models (is_default)
    WHERE is_default = TRUE AND available = TRUE;

CREATE TABLE agent_preferences (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    model_id TEXT NOT NULL REFERENCES agent_models (id) ON DELETE RESTRICT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE TRIGGER agent_preferences_set_updated_at
BEFORE UPDATE ON agent_preferences
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 12
WHERE singleton = TRUE;
