-- A conversational request may propose a local planning change, but it must
-- remain inert until its owner explicitly approves it. Keeping the proposal
-- on the durable agent job lets every signed-in personal client render the
-- same confirmation state after reconnecting.
ALTER TABLE agent_jobs
    ADD COLUMN pending_action_type TEXT NULL
        CHECK (pending_action_type IS NULL OR pending_action_type IN ('create_task', 'create_schedule')),
    ADD COLUMN pending_action_title TEXT NULL
        CHECK (pending_action_title IS NULL OR char_length(btrim(pending_action_title)) BETWEEN 1 AND 200),
    ADD COLUMN pending_action_starts_at TIMESTAMPTZ NULL,
    ADD COLUMN pending_action_ends_at TIMESTAMPTZ NULL,
    ADD COLUMN pending_action_time_zone TEXT NULL
        CHECK (pending_action_time_zone IS NULL OR char_length(pending_action_time_zone) BETWEEN 1 AND 80),
    ADD CONSTRAINT agent_jobs_pending_action_shape CHECK (
        (pending_action_type IS NULL
            AND pending_action_title IS NULL
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
            AND pending_action_starts_at IS NOT NULL
            AND pending_action_ends_at IS NOT NULL
            AND pending_action_time_zone IS NOT NULL
            AND pending_action_ends_at > pending_action_starts_at)
    );

UPDATE jimin_schema_metadata
SET schema_version = 7
WHERE singleton = TRUE;
