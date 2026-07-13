-- Task actions can carry an optional due date from deterministic relative-day
-- requests such as "내일 할 일에 ... 추가해 줘". Existing task and schedule
-- actions remain valid. Rollback uses the previous image with a verified
-- database restore, following the forward-only migration policy.
ALTER TABLE agent_jobs
    ADD COLUMN pending_action_due_at TIMESTAMPTZ NULL;

ALTER TABLE agent_jobs
    DROP CONSTRAINT agent_jobs_pending_action_shape,
    ADD CONSTRAINT agent_jobs_pending_action_shape CHECK (
        (pending_action_type IS NULL
            AND pending_action_title IS NULL
            AND pending_action_due_at IS NULL
            AND pending_action_starts_at IS NULL
            AND pending_action_ends_at IS NULL
            AND pending_action_time_zone IS NULL)
        OR (pending_action_type = 'create_task'
            AND pending_action_title IS NOT NULL
            AND pending_action_starts_at IS NULL
            AND pending_action_ends_at IS NULL
            AND pending_action_time_zone IS NULL)
        OR (pending_action_type = 'create_schedule'
            AND pending_action_title IS NOT NULL
            AND pending_action_due_at IS NULL
            AND pending_action_starts_at IS NOT NULL
            AND pending_action_ends_at IS NOT NULL
            AND pending_action_time_zone IS NOT NULL
            AND pending_action_ends_at > pending_action_starts_at)
    );

UPDATE jimin_schema_metadata
SET schema_version = 15
WHERE singleton = TRUE;
