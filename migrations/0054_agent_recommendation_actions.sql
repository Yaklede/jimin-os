-- Recommendation decisions can be executed by the authenticated assistant
-- inside the same transaction as the final response and job audit.
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
            'send_webhook_message',
            'approve_recommendation',
            'reject_recommendation',
            'defer_recommendation'
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
            'send_webhook_message',
            'approve_recommendation',
            'reject_recommendation',
            'defer_recommendation'
        )
    );

UPDATE jimin_schema_metadata
SET schema_version = 54
WHERE singleton = TRUE;
