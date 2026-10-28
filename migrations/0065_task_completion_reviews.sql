-- Completion reports are not completion. Only the authenticated owner can decide.
CREATE TABLE task_completion_reviews (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    command_id UUID NULL REFERENCES scheduled_chat_reply_commands(id) ON DELETE SET NULL,
    reported_task_version BIGINT NOT NULL CHECK (reported_task_version > 0),
    reporter_name TEXT NOT NULL CHECK (char_length(reporter_name) BETWEEN 1 AND 80),
    completion_note TEXT NULL CHECK (char_length(completion_note) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','superseded')),
    owner_note TEXT NULL CHECK (char_length(owner_note) BETWEEN 1 AND 2000),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ NULL,
    CHECK ((status='pending') = (resolved_at IS NULL))
);
CREATE UNIQUE INDEX task_completion_one_pending_idx ON task_completion_reviews(task_id) WHERE status='pending';
CREATE INDEX task_completion_owner_idx ON task_completion_reviews(user_id,id DESC) WHERE status='pending';
ALTER TABLE scheduled_chat_reply_commands DROP CONSTRAINT scheduled_chat_reply_commands_outcome_check;
ALTER TABLE scheduled_chat_reply_commands ADD CONSTRAINT scheduled_chat_reply_commands_outcome_check
    CHECK(outcome IN ('completed','already_completed','stale','not_assigned','invalid','unknown_task','blocked','review_pending','already_pending','review_approved','review_rejected'));
ALTER TABLE push_deliveries DROP CONSTRAINT push_deliveries_item_type_check;
ALTER TABLE push_deliveries ADD CONSTRAINT push_deliveries_item_type_check
    CHECK (item_type IN ('task','schedule','brief','weekly_report','google_chat_inflow','gmail_inflow','scheduled_work','task_completion_review'));
UPDATE jimin_schema_metadata SET schema_version=65 WHERE singleton=TRUE;
