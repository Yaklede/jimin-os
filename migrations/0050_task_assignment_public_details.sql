-- Outbound task notifications must never reconstruct public content by
-- parsing editable notes that may also contain imported source evidence.
-- Persist the reviewed public assignment projection separately so every
-- provider delivery uses the same bounded snapshot.
CREATE TABLE task_assignment_public_details (
    task_id UUID PRIMARY KEY REFERENCES tasks (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    summary TEXT NULL CHECK (
        summary IS NULL OR char_length(btrim(summary)) BETWEEN 1 AND 2000
    ),
    action_items TEXT[] NOT NULL DEFAULT '{}',
    completion_criteria TEXT NULL CHECK (
        completion_criteria IS NULL
        OR char_length(btrim(completion_criteria)) BETWEEN 1 AND 2000
    ),
    reference_links TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (cardinality(action_items) <= 20),
    CHECK (cardinality(reference_links) <= 20)
);

CREATE INDEX task_assignment_public_details_user_idx
    ON task_assignment_public_details (user_id, task_id);

CREATE TRIGGER task_assignment_public_details_set_updated_at
BEFORE UPDATE ON task_assignment_public_details
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 50
WHERE singleton = TRUE;
