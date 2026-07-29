-- Gmail credentials no longer piggyback on the single Calendar account.
-- Every mailbox is attached to exactly one personal or company workspace so
-- provider data and assistant context cannot cross that boundary implicitly.

ALTER TABLE workspaces
    ADD CONSTRAINT workspaces_id_user_unique UNIQUE (id, user_id);

CREATE TABLE gmail_accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL,
    provider TEXT NOT NULL DEFAULT 'google' CHECK (provider = 'google'),
    provider_subject TEXT NOT NULL CHECK (char_length(provider_subject) BETWEEN 1 AND 255),
    email TEXT NOT NULL CHECK (char_length(email) BETWEEN 3 AND 320),
    status TEXT NOT NULL CHECK (
        status IN ('connecting', 'active', 'reauth_required', 'revoking', 'revoked', 'error')
    ),
    granted_scopes TEXT[] NOT NULL DEFAULT '{}',
    refresh_token_ciphertext BYTEA NULL,
    refresh_token_nonce BYTEA NULL,
    encryption_key_version INTEGER NULL,
    last_successful_sync_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (
        last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, provider_subject),
    UNIQUE (id, workspace_id),
    UNIQUE (id, user_id, workspace_id),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspaces (id, user_id) ON DELETE RESTRICT,
    CHECK (
        (refresh_token_ciphertext IS NULL
            AND refresh_token_nonce IS NULL
            AND encryption_key_version IS NULL)
        OR (refresh_token_ciphertext IS NOT NULL
            AND refresh_token_nonce IS NOT NULL
            AND encryption_key_version IS NOT NULL)
    )
);

CREATE INDEX gmail_accounts_user_workspace_idx
    ON gmail_accounts (user_id, workspace_id, status, email);

CREATE TABLE gmail_oauth_authorizations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL,
    reconnect_account_id UUID NULL,
    session_id UUID NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    state_verifier BYTEA NOT NULL UNIQUE,
    pkce_verifier_ciphertext BYTEA NULL,
    pkce_nonce BYTEA NULL,
    encryption_key_version INTEGER NULL,
    client_kind TEXT NOT NULL CHECK (client_kind IN ('macos', 'ios', 'android')),
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'exchanging', 'completed', 'failed', 'expired', 'cancelled')
    ),
    expires_at TIMESTAMPTZ NOT NULL,
    failure_code TEXT NULL CHECK (
        failure_code IS NULL OR char_length(failure_code) BETWEEN 1 AND 120
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspaces (id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (reconnect_account_id, user_id, workspace_id)
        REFERENCES gmail_accounts (id, user_id, workspace_id) ON DELETE CASCADE,
    CHECK (
        (pkce_verifier_ciphertext IS NULL
            AND pkce_nonce IS NULL
            AND encryption_key_version IS NULL)
        OR (pkce_verifier_ciphertext IS NOT NULL
            AND pkce_nonce IS NOT NULL
            AND encryption_key_version IS NOT NULL)
    )
);

CREATE INDEX gmail_oauth_authorizations_pending_idx
    ON gmail_oauth_authorizations (expires_at, id)
    WHERE status IN ('pending', 'exchanging');

-- Older installations may not have opened the project workspace yet. Create
-- the personal boundary before projecting the legacy Calendar-backed cache.
INSERT INTO workspaces (id, user_id, scope, name)
SELECT
    overlay(
        overlay(gen_random_uuid()::TEXT placing '7' from 15 for 1)
        placing '8' from 20 for 1
    )::UUID,
    users.id,
    'personal',
    '개인'
FROM users
WHERE NOT EXISTS (
    SELECT 1
    FROM workspaces
    WHERE workspaces.user_id = users.id
      AND workspaces.scope = 'personal'
)
ON CONFLICT (user_id, scope) DO NOTHING;

-- Calendar-derived Gmail metadata remains visible but is deliberately marked
-- for reauthorization. Calendar credentials are never copied or invalidated.
INSERT INTO gmail_accounts (
    id, user_id, workspace_id, provider_subject, email, status,
    granted_scopes, last_successful_sync_at, last_error_code
)
SELECT
    overlay(
        overlay(gen_random_uuid()::TEXT placing '7' from 15 for 1)
        placing '8' from 20 for 1
    )::UUID,
    users.id,
    personal.id,
    COALESCE(calendar_account.provider_subject, users.google_sub),
    COALESCE(calendar_account.email, users.email),
    'reauth_required',
    COALESCE(calendar_account.granted_scopes, ARRAY[]::TEXT[]),
    gmail_sync_states.last_successful_sync_at,
    'gmail.reauthorization_required'
FROM users
JOIN workspaces AS personal
  ON personal.user_id = users.id
 AND personal.scope = 'personal'
LEFT JOIN calendar_accounts AS calendar_account
  ON calendar_account.user_id = users.id
LEFT JOIN gmail_sync_states
  ON gmail_sync_states.user_id = users.id
WHERE gmail_sync_states.user_id IS NOT NULL
   OR EXISTS (
       SELECT 1 FROM gmail_messages WHERE gmail_messages.user_id = users.id
   )
   OR (
       calendar_account.id IS NOT NULL
       AND 'https://www.googleapis.com/auth/gmail.readonly'
           = ANY(calendar_account.granted_scopes)
   )
ON CONFLICT (user_id, provider_subject) DO NOTHING;

ALTER TABLE gmail_sync_states
    ADD COLUMN account_id UUID NULL REFERENCES gmail_accounts (id) ON DELETE CASCADE,
    ADD COLUMN workspace_id UUID NULL REFERENCES workspaces (id) ON DELETE RESTRICT;

UPDATE gmail_sync_states AS state
SET account_id = account.id,
    workspace_id = account.workspace_id
FROM gmail_accounts AS account
WHERE account.user_id = state.user_id;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM gmail_sync_states
        WHERE account_id IS NULL OR workspace_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'gmail legacy sync state could not be mapped to a personal account';
    END IF;
END;
$$;

ALTER TABLE gmail_sync_states
    DROP CONSTRAINT gmail_sync_states_pkey,
    DROP COLUMN user_id,
    ALTER COLUMN account_id SET NOT NULL,
    ALTER COLUMN workspace_id SET NOT NULL,
    ADD PRIMARY KEY (account_id),
    ADD CONSTRAINT gmail_sync_states_account_workspace_unique
        UNIQUE (account_id, workspace_id),
    ADD CONSTRAINT gmail_sync_states_account_workspace_fk
        FOREIGN KEY (account_id, workspace_id)
        REFERENCES gmail_accounts (id, workspace_id) ON DELETE CASCADE;

ALTER TABLE gmail_messages
    ADD COLUMN account_id UUID NULL REFERENCES gmail_accounts (id) ON DELETE CASCADE,
    ADD COLUMN workspace_id UUID NULL REFERENCES workspaces (id) ON DELETE RESTRICT;

UPDATE gmail_messages AS message
SET account_id = account.id,
    workspace_id = account.workspace_id
FROM gmail_accounts AS account
WHERE account.user_id = message.user_id;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM gmail_messages
        WHERE account_id IS NULL OR workspace_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'gmail legacy message could not be mapped to a personal account';
    END IF;
END;
$$;

DROP INDEX gmail_messages_user_inbox_idx;

ALTER TABLE gmail_messages
    DROP CONSTRAINT gmail_messages_user_id_provider_message_id_key,
    DROP COLUMN user_id,
    ALTER COLUMN account_id SET NOT NULL,
    ALTER COLUMN workspace_id SET NOT NULL,
    ADD CONSTRAINT gmail_messages_account_provider_message_unique
        UNIQUE (account_id, provider_message_id),
    ADD CONSTRAINT gmail_messages_account_workspace_fk
        FOREIGN KEY (account_id, workspace_id)
        REFERENCES gmail_accounts (id, workspace_id) ON DELETE CASCADE;

CREATE INDEX gmail_messages_workspace_inbox_idx
    ON gmail_messages (
        workspace_id,
        account_id,
        is_unread DESC,
        received_at DESC NULLS LAST
    )
    WHERE provider_deleted_at IS NULL;

CREATE TRIGGER gmail_accounts_set_updated_at
BEFORE UPDATE ON gmail_accounts
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER gmail_oauth_authorizations_set_updated_at
BEFORE UPDATE ON gmail_oauth_authorizations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 44
WHERE singleton = TRUE;
