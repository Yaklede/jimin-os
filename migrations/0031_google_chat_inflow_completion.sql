-- A promoted Google Chat conversation must remain traceable after it becomes a
-- task. The selected source message receives a completion reaction and one
-- idempotent thread reply containing the task deadline. Provider delivery is
-- retried from durable state instead of being treated as part of the task
-- transaction.
ALTER TABLE project_inflow_items
    ADD COLUMN completion_requested_at TIMESTAMPTZ NULL,
    ADD COLUMN completion_reaction_at TIMESTAMPTZ NULL,
    ADD COLUMN completion_reply_at TIMESTAMPTZ NULL,
    ADD COLUMN completion_delivery_error_code TEXT NULL,
    ADD COLUMN completion_delivery_attempt_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN completion_delivery_next_attempt_at TIMESTAMPTZ NULL,
    ADD CONSTRAINT project_inflow_items_completion_attempt_shape CHECK (
        completion_delivery_attempt_count BETWEEN 0 AND 10000
    ),
    ADD CONSTRAINT project_inflow_items_completion_error_shape CHECK (
        completion_delivery_error_code IS NULL
        OR (
            char_length(completion_delivery_error_code) BETWEEN 1 AND 120
            AND completion_delivery_error_code ~ '^[a-z0-9._-]+$'
        )
    ),
    ADD CONSTRAINT project_inflow_items_completion_state_shape CHECK (
        (
            completion_requested_at IS NULL
            AND completion_reaction_at IS NULL
            AND completion_reply_at IS NULL
            AND completion_delivery_error_code IS NULL
            AND completion_delivery_attempt_count = 0
            AND completion_delivery_next_attempt_at IS NULL
        )
        OR (
            status = 'promoted'
            AND completion_requested_at IS NOT NULL
            AND (
                completion_reaction_at IS NULL
                OR completion_reaction_at >= completion_requested_at
            )
            AND (
                completion_reply_at IS NULL
                OR completion_reply_at >= completion_requested_at
            )
            AND (
                (
                    completion_reaction_at IS NOT NULL
                    AND completion_reply_at IS NOT NULL
                    AND completion_delivery_error_code IS NULL
                    AND completion_delivery_next_attempt_at IS NULL
                )
                OR (
                    completion_delivery_attempt_count >= 0
                    AND completion_delivery_next_attempt_at IS NOT NULL
                )
            )
        )
    );

CREATE INDEX project_inflow_items_completion_delivery_idx
    ON project_inflow_items (
        completion_delivery_next_attempt_at,
        completion_delivery_attempt_count,
        id
    )
    WHERE completion_requested_at IS NOT NULL
      AND (completion_reaction_at IS NULL OR completion_reply_at IS NULL);

CREATE OR REPLACE FUNCTION jimin_project_inflow_item_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    IF ROW(
        NEW.status, NEW.promoted_task_id, NEW.sender_name,
        NEW.sender_provider_name, NEW.acknowledged_at,
        NEW.completion_requested_at, NEW.completion_reaction_at,
        NEW.completion_reply_at, NEW.completion_delivery_error_code,
        NEW.completion_delivery_attempt_count,
        NEW.completion_delivery_next_attempt_at
    ) IS DISTINCT FROM ROW(
        OLD.status, OLD.promoted_task_id, OLD.sender_name,
        OLD.sender_provider_name, OLD.acknowledged_at,
        OLD.completion_requested_at, OLD.completion_reaction_at,
        OLD.completion_reply_at, OLD.completion_delivery_error_code,
        OLD.completion_delivery_attempt_count,
        OLD.completion_delivery_next_attempt_at
    ) THEN
        NEW.version = OLD.version + 1;
    ELSE
        NEW.version = OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

UPDATE jimin_schema_metadata
SET schema_version = 31
WHERE singleton = TRUE;
