-- Reading a conversation does not dismiss it. Newly ingested messages start
-- unread; existing task links and original text remain unchanged.
ALTER TABLE project_inflow_items
    ADD COLUMN reviewed_at TIMESTAMPTZ NULL,
    ADD COLUMN dismissal_reply_version BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN dismissal_reason TEXT NULL
        CHECK (dismissal_reason IS NULL OR char_length(dismissal_reason) BETWEEN 1 AND 2000);

CREATE TABLE project_inflow_dismissal_replies (
    id UUID PRIMARY KEY,
    inflow_id UUID NOT NULL UNIQUE REFERENCES project_inflow_items(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources(id) ON DELETE CASCADE,
    thread_name TEXT NULL,
    reason TEXT NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 1 AND 2000),
    sent_at TIMESTAMPTZ NULL,
    error_code TEXT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NULL DEFAULT NOW(),
    lease_expires_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX project_inflow_dismissal_replies_pending_idx
    ON project_inflow_dismissal_replies(source_id, next_attempt_at)
    WHERE sent_at IS NULL;

CREATE OR REPLACE FUNCTION jimin_project_inflow_item_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    IF ROW(
        NEW.status, NEW.promoted_task_id, NEW.sender_name,
        NEW.sender_provider_name, NEW.acknowledged_at,
        NEW.completion_requested_at, NEW.completion_reaction_at,
        NEW.completion_reply_at, NEW.completion_delivery_error_code,
        NEW.completion_delivery_attempt_count, NEW.completion_delivery_next_attempt_at,
        NEW.reviewed_at, NEW.dismissal_reason, NEW.dismissal_reply_version
    ) IS DISTINCT FROM ROW(
        OLD.status, OLD.promoted_task_id, OLD.sender_name,
        OLD.sender_provider_name, OLD.acknowledged_at,
        OLD.completion_requested_at, OLD.completion_reaction_at,
        OLD.completion_reply_at, OLD.completion_delivery_error_code,
        OLD.completion_delivery_attempt_count, OLD.completion_delivery_next_attempt_at,
        OLD.reviewed_at, OLD.dismissal_reason, OLD.dismissal_reply_version
    ) THEN
        NEW.version = OLD.version + 1;
    ELSE
        NEW.version = OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

UPDATE jimin_schema_metadata SET schema_version = 57 WHERE singleton = TRUE;
