-- A managed Codex runtime owns the ChatGPT OAuth tokens. The application only
-- keeps the short-lived, presentable device-code login details so a paired
-- personal client can complete the official authorization flow.
CREATE TABLE agent_auth_attempts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    state TEXT NOT NULL CHECK (state IN ('requested', 'awaiting_authorization', 'ready', 'failed')),
    login_id TEXT NULL CHECK (login_id IS NULL OR char_length(login_id) BETWEEN 1 AND 300),
    verification_url TEXT NULL CHECK (verification_url IS NULL OR char_length(verification_url) BETWEEN 1 AND 2048),
    user_code TEXT NULL CHECK (user_code IS NULL OR char_length(user_code) BETWEEN 1 AND 256),
    error_code TEXT NULL CHECK (error_code IS NULL OR char_length(error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (state = 'requested'
            AND login_id IS NULL
            AND verification_url IS NULL
            AND user_code IS NULL
            AND error_code IS NULL
            AND completed_at IS NULL)
        OR (state = 'awaiting_authorization'
            AND login_id IS NOT NULL
            AND verification_url IS NOT NULL
            AND user_code IS NOT NULL
            AND error_code IS NULL
            AND completed_at IS NULL)
        OR (state = 'ready'
            AND login_id IS NOT NULL
            AND verification_url IS NULL
            AND user_code IS NULL
            AND error_code IS NULL
            AND completed_at IS NOT NULL)
        OR (state = 'failed'
            AND error_code IS NOT NULL
            AND completed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX agent_auth_attempts_one_active_user_idx
    ON agent_auth_attempts (user_id)
    WHERE state IN ('requested', 'awaiting_authorization');

CREATE INDEX agent_auth_attempts_requested_idx
    ON agent_auth_attempts (created_at, id)
    WHERE state = 'requested';

CREATE INDEX agent_auth_attempts_user_recent_idx
    ON agent_auth_attempts (user_id, created_at DESC, id DESC);

CREATE TRIGGER agent_auth_attempts_set_updated_at
BEFORE UPDATE ON agent_auth_attempts
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 6
WHERE singleton = TRUE;
