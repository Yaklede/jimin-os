-- Agent-directed chat messages use the same durable action audit as planning
-- mutations. The Rust action contract already emits send_webhook_message, so
-- both audit allowlists must accept it before the job and delivery can commit
-- atomically.
ALTER TABLE agent_jobs
    DROP CONSTRAINT agent_jobs_executed_action_type_check,
    ADD CONSTRAINT agent_jobs_executed_action_type_check CHECK (
        executed_action_type IS NULL OR executed_action_type IN (
            'create_task',
            'update_task',
            'complete_task',
            'cancel_task',
            'create_schedule',
            'update_schedule',
            'cancel_schedule',
            'create_project',
            'update_project',
            'delete_project',
            'send_webhook_message'
        )
    );

ALTER TABLE agent_job_action_executions
    DROP CONSTRAINT agent_job_action_executions_action_type_check,
    ADD CONSTRAINT agent_job_action_executions_action_type_check CHECK (
        action_type IN (
            'create_task',
            'update_task',
            'complete_task',
            'cancel_task',
            'create_schedule',
            'update_schedule',
            'cancel_schedule',
            'create_project',
            'update_project',
            'delete_project',
            'send_webhook_message'
        )
    );

UPDATE jimin_schema_metadata
SET schema_version = 25
WHERE singleton = TRUE;
