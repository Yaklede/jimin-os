-- Existing company OAuth only; no public callback or additional app credentials.
ALTER TABLE webhook_deliveries ADD COLUMN google_chat_message_name TEXT NULL;
ALTER TABLE webhook_deliveries ADD COLUMN google_chat_thread_name TEXT NULL;
ALTER TABLE webhook_deliveries ADD COLUMN google_chat_replies_reconciled_at TIMESTAMPTZ NULL;
CREATE TABLE scheduled_chat_reply_commands (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources(id) ON DELETE CASCADE,
    delivery_id UUID REFERENCES webhook_deliveries(id) ON DELETE SET NULL,
    provider_message_name TEXT NOT NULL,
    thread_name TEXT NOT NULL,
    actor_name TEXT NULL,
    task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    outcome TEXT NOT NULL CHECK(outcome IN ('completed','already_completed','stale','not_assigned','invalid','unknown_task','blocked')),
    feedback_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','sending','retry_wait','sent','failed')),
    attempt_count INT NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 5),
    lease_owner TEXT NULL,
    lease_expires_at TIMESTAMPTZ NULL,
    next_attempt_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL,
    sent_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(source_id,provider_message_name),
    CHECK((status='sending' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR (status<>'sending' AND lease_owner IS NULL AND lease_expires_at IS NULL))
);
CREATE INDEX scheduled_chat_reply_feedback_idx ON scheduled_chat_reply_commands(source_id,status,next_attempt_at);
UPDATE jimin_schema_metadata SET schema_version=64 WHERE singleton=TRUE;
