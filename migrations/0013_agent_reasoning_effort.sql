-- Reasoning depth belongs to the selected model catalog. Keep the option
-- identifiers dynamic because Codex may add or remove supported efforts.
ALTER TABLE agent_models
    ADD COLUMN default_reasoning_effort TEXT NULL
        CHECK (
            default_reasoning_effort IS NULL
            OR char_length(btrim(default_reasoning_effort)) BETWEEN 1 AND 80
        );

CREATE TABLE agent_model_reasoning_efforts (
    model_id TEXT NOT NULL REFERENCES agent_models (id) ON DELETE CASCADE,
    effort TEXT NOT NULL CHECK (char_length(btrim(effort)) BETWEEN 1 AND 80),
    description TEXT NOT NULL CHECK (char_length(description) <= 1000),
    position SMALLINT NOT NULL CHECK (position >= 0),
    PRIMARY KEY (model_id, effort),
    UNIQUE (model_id, position)
);

ALTER TABLE agent_preferences
    DROP CONSTRAINT agent_preferences_model_id_fkey,
    ALTER COLUMN model_id DROP NOT NULL,
    ADD CONSTRAINT agent_preferences_model_id_fkey
        FOREIGN KEY (model_id) REFERENCES agent_models (id) ON DELETE SET NULL,
    ADD COLUMN reasoning_effort TEXT NULL
        CHECK (
            reasoning_effort IS NULL
            OR char_length(btrim(reasoning_effort)) BETWEEN 1 AND 80
        );

UPDATE jimin_schema_metadata
SET schema_version = 13
WHERE singleton = TRUE;
