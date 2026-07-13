-- One assistant turn can execute several validated planning actions. Keep the
-- original single-action columns for backward compatibility and store the
-- complete ordered audit in a child table.
CREATE TABLE agent_job_action_executions (
    job_id UUID NOT NULL REFERENCES agent_jobs(id) ON DELETE CASCADE,
    action_index SMALLINT NOT NULL CHECK (action_index >= 0 AND action_index < 32),
    action_type TEXT NOT NULL CHECK (
        action_type IN (
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
    entity_id UUID NOT NULL,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (job_id, action_index),
    UNIQUE (job_id, entity_id)
);

INSERT INTO agent_job_action_executions (
    job_id, action_index, action_type, entity_id, executed_at
)
SELECT id, 0, executed_action_type, executed_entity_id, executed_at
FROM agent_jobs
WHERE executed_action_type IS NOT NULL;

ALTER TABLE agent_jobs
    ADD COLUMN executed_action_count SMALLINT NOT NULL DEFAULT 0
        CHECK (executed_action_count >= 0 AND executed_action_count <= 32);

UPDATE agent_jobs
SET executed_action_count = 1
WHERE executed_action_type IS NOT NULL;

UPDATE jimin_schema_metadata
SET schema_version = 17
WHERE singleton = TRUE;
