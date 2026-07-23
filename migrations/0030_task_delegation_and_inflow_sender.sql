-- A Chat conversation can become an assigned task in one decision. Keep the
-- human-readable assignee on the task while Google user resource names remain
-- provider metadata used only to recover sender names and expand mentions.
ALTER TABLE tasks
    ADD COLUMN assignee_name TEXT NULL,
    ADD CONSTRAINT tasks_assignee_name_length CHECK (
        assignee_name IS NULL
        OR char_length(btrim(assignee_name)) BETWEEN 1 AND 80
    );

ALTER TABLE project_inflow_items
    ADD COLUMN sender_provider_name TEXT NULL,
    ADD CONSTRAINT project_inflow_items_sender_provider_name_shape CHECK (
        sender_provider_name IS NULL
        OR sender_provider_name ~ '^users/([0-9]{1,40}|app)$'
    );

-- Sender metadata can be repaired by a later provider reconciliation. Treat
-- that repair as a visible sync change even when the work decision is intact.
CREATE OR REPLACE FUNCTION jimin_project_inflow_item_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    IF ROW(
        NEW.status, NEW.promoted_task_id, NEW.sender_name,
        NEW.sender_provider_name
    ) IS DISTINCT FROM ROW(
        OLD.status, OLD.promoted_task_id, OLD.sender_name,
        OLD.sender_provider_name
    ) THEN
        NEW.version = OLD.version + 1;
    ELSE
        NEW.version = OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

UPDATE jimin_schema_metadata
SET schema_version = 30
WHERE singleton = TRUE;
