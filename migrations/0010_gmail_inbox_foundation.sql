-- A read-only Gmail inbox projection shares the user's encrypted Google
-- refresh credential with Calendar. Provider message bodies and raw payloads
-- are deliberately never persisted; only the compact metadata needed by the
-- personal assistant is retained.
CREATE TABLE gmail_sync_states (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('idle', 'syncing', 'error')),
    last_successful_sync_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE TABLE gmail_messages (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_message_id TEXT NOT NULL CHECK (char_length(provider_message_id) BETWEEN 1 AND 255),
    provider_thread_id TEXT NOT NULL CHECK (char_length(provider_thread_id) BETWEEN 1 AND 255),
    received_at TIMESTAMPTZ NULL,
    sender TEXT NULL CHECK (sender IS NULL OR char_length(sender) <= 1024),
    subject TEXT NULL CHECK (subject IS NULL OR char_length(subject) <= 998),
    snippet TEXT NULL CHECK (snippet IS NULL OR char_length(snippet) <= 512),
    is_unread BOOLEAN NOT NULL DEFAULT FALSE,
    provider_deleted_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, provider_message_id)
);

CREATE INDEX gmail_messages_user_inbox_idx
    ON gmail_messages (user_id, is_unread DESC, received_at DESC NULLS LAST)
    WHERE provider_deleted_at IS NULL;

CREATE TRIGGER gmail_sync_states_set_updated_at
BEFORE UPDATE ON gmail_sync_states
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER gmail_messages_set_updated_at
BEFORE UPDATE ON gmail_messages
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 10
WHERE singleton = TRUE;
