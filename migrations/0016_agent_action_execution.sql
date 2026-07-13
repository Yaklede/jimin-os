-- Agent turns can now finish with one server-validated planning action. The
-- action and its assistant result are committed together, while these audit
-- columns retain only the action type and affected entity ID (never user text).
ALTER TABLE agent_jobs
    ADD COLUMN executed_action_type TEXT NULL
        CHECK (
            executed_action_type IS NULL OR executed_action_type IN (
                'create_task',
                'update_task',
                'complete_task',
                'cancel_task',
                'create_schedule',
                'update_schedule',
                'cancel_schedule',
                'create_project',
                'update_project'
            )
        ),
    ADD COLUMN executed_entity_id UUID NULL,
    ADD COLUMN executed_at TIMESTAMPTZ NULL,
    ADD CONSTRAINT agent_jobs_executed_action_shape CHECK (
        (executed_action_type IS NULL AND executed_entity_id IS NULL AND executed_at IS NULL)
        OR
        (executed_action_type IS NOT NULL AND executed_entity_id IS NOT NULL AND executed_at IS NOT NULL)
    );

UPDATE jimin_schema_metadata
SET schema_version = 16
WHERE singleton = TRUE;
