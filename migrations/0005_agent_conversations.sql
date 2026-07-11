-- Durable conversation and agent-job records are server-owned. The API can
-- enqueue work without holding a Codex App Server process handle, allowing the
-- agent container to fail independently from scheduling and task endpoints.
CREATE TABLE conversations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    title TEXT NULL CHECK (title IS NULL OR char_length(btrim(title)) BETWEEN 1 AND 200),
    codex_thread_id TEXT NULL UNIQUE CHECK (codex_thread_id IS NULL OR char_length(codex_thread_id) BETWEEN 1 AND 300),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    last_message_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE INDEX conversations_user_recent_idx
    ON conversations (user_id, last_message_at DESC NULLS LAST, created_at DESC)
    WHERE status = 'active';

CREATE TABLE messages (
    id UUID PRIMARY KEY,
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    agent_job_id UUID NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system_event')),
    content TEXT NOT NULL CHECK (char_length(content) BETWEEN 1 AND 24000),
    status TEXT NOT NULL CHECK (status IN ('pending', 'streaming', 'completed', 'failed', 'cancelled')),
    client_message_id UUID NULL,
    provider_item_id TEXT NULL CHECK (provider_item_id IS NULL OR char_length(provider_item_id) BETWEEN 1 AND 300),
    completed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (conversation_id, client_message_id)
);

CREATE INDEX messages_conversation_created_idx
    ON messages (conversation_id, created_at, id);

CREATE TABLE agent_jobs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    input_message_id UUID NOT NULL UNIQUE REFERENCES messages (id) ON DELETE RESTRICT,
    state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'claimed', 'running', 'waiting_approval', 'retry_wait', 'completed', 'failed', 'cancelled', 'declined')),
    phase TEXT NULL CHECK (phase IS NULL OR phase IN ('preparing', 'starting_turn', 'streaming', 'tool_wait', 'completing', 'interrupting')),
    codex_thread_id TEXT NULL CHECK (codex_thread_id IS NULL OR char_length(codex_thread_id) BETWEEN 1 AND 300),
    codex_turn_id TEXT NULL CHECK (codex_turn_id IS NULL OR char_length(codex_turn_id) BETWEEN 1 AND 300),
    claim_owner TEXT NULL CHECK (claim_owner IS NULL OR char_length(claim_owner) BETWEEN 1 AND 200),
    claim_expires_at TIMESTAMPTZ NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    cancel_requested_at TIMESTAMPTZ NULL,
    error_code TEXT NULL CHECK (error_code IS NULL OR char_length(error_code) <= 120),
    started_at TIMESTAMPTZ NULL,
    finished_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE UNIQUE INDEX agent_jobs_one_active_conversation_idx
    ON agent_jobs (conversation_id)
    WHERE state IN ('queued', 'claimed', 'running', 'waiting_approval', 'retry_wait');

CREATE INDEX agent_jobs_claimable_idx
    ON agent_jobs (created_at, id)
    WHERE state IN ('queued', 'retry_wait');

ALTER TABLE messages
    ADD CONSTRAINT messages_agent_job_id_fkey
    FOREIGN KEY (agent_job_id) REFERENCES agent_jobs (id) ON DELETE SET NULL;

CREATE TRIGGER conversations_set_updated_at
BEFORE UPDATE ON conversations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER messages_set_updated_at
BEFORE UPDATE ON messages
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER agent_jobs_set_updated_at
BEFORE UPDATE ON agent_jobs
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 5
WHERE singleton = TRUE;
