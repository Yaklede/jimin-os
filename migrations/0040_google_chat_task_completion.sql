-- Completing a task promoted from Google Chat must notify the original CS
-- thread. Keep every completion cycle as a durable, version-fenced delivery so
-- retries are idempotent and restoring a task cancels an unsent stale reply.
CREATE TABLE google_chat_task_completion_deliveries (
    task_id UUID NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    task_version BIGINT NOT NULL,
    inflow_id UUID NOT NULL REFERENCES project_inflow_items (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources (id) ON DELETE CASCADE,
    task_title TEXT NOT NULL CHECK (
        char_length(btrim(task_title)) BETWEEN 1 AND 200
    ),
    assignee_name TEXT NULL CHECK (
        assignee_name IS NULL OR char_length(btrim(assignee_name)) BETWEEN 1 AND 80
    ),
    completed_at TIMESTAMPTZ NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reply_at TIMESTAMPTZ NULL,
    cancelled_at TIMESTAMPTZ NULL,
    delivery_error_code TEXT NULL,
    delivery_attempt_count INTEGER NOT NULL DEFAULT 0,
    delivery_next_attempt_at TIMESTAMPTZ NULL DEFAULT NOW(),
    PRIMARY KEY (task_id, task_version),
    CONSTRAINT google_chat_task_completion_task_version_positive CHECK (
        task_version > 0
    ),
    CONSTRAINT google_chat_task_completion_attempt_shape CHECK (
        delivery_attempt_count BETWEEN 0 AND 10000
    ),
    CONSTRAINT google_chat_task_completion_error_shape CHECK (
        delivery_error_code IS NULL
        OR (
            char_length(delivery_error_code) BETWEEN 1 AND 120
            AND delivery_error_code ~ '^[a-z0-9._-]+$'
        )
    ),
    CONSTRAINT google_chat_task_completion_state_shape CHECK (
        (
            reply_at IS NULL
            AND cancelled_at IS NULL
            AND delivery_next_attempt_at IS NOT NULL
        )
        OR (
            reply_at IS NOT NULL
            AND cancelled_at IS NULL
            AND delivery_error_code IS NULL
            AND delivery_next_attempt_at IS NULL
            AND reply_at >= requested_at
        )
        OR (
            reply_at IS NULL
            AND cancelled_at IS NOT NULL
            AND delivery_error_code IS NULL
            AND delivery_next_attempt_at IS NULL
            AND cancelled_at >= requested_at
        )
    )
);

CREATE INDEX google_chat_task_completion_delivery_pending_idx
    ON google_chat_task_completion_deliveries (
        source_id,
        delivery_next_attempt_at,
        delivery_attempt_count,
        task_id,
        task_version
    )
    WHERE reply_at IS NULL AND cancelled_at IS NULL;

UPDATE jimin_schema_metadata
SET schema_version = 40
WHERE singleton = TRUE;
