-- Project deletion is a server-validated agent action. Extend both the
-- legacy single-action audit and the ordered batch audit without weakening
-- the existing allowlist constraints.
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
            'delete_project'
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
            'delete_project'
        )
    );

UPDATE jimin_schema_metadata
SET schema_version = 18
WHERE singleton = TRUE;
