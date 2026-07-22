-- Company Google Chat is a separate workspace connection from the owner's
-- personal Google Calendar account. Multiple company identities may be linked
-- by one owner, while every source remains bound to one owned project.
CREATE TABLE google_chat_accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_subject TEXT NOT NULL CHECK (char_length(provider_subject) BETWEEN 1 AND 255),
    email TEXT NOT NULL CHECK (char_length(email) BETWEEN 3 AND 320),
    status TEXT NOT NULL CHECK (status IN ('connecting', 'active', 'reauth_required', 'revoking', 'revoked', 'error')),
    granted_scopes TEXT[] NOT NULL DEFAULT '{}',
    refresh_token_ciphertext BYTEA NULL,
    refresh_token_nonce BYTEA NULL,
    encryption_key_version INTEGER NULL,
    last_successful_sync_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, provider_subject),
    CHECK (
        (refresh_token_ciphertext IS NULL AND refresh_token_nonce IS NULL AND encryption_key_version IS NULL)
        OR (refresh_token_ciphertext IS NOT NULL AND refresh_token_nonce IS NOT NULL AND encryption_key_version IS NOT NULL)
    )
);

CREATE TABLE google_chat_oauth_authorizations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    state_verifier BYTEA NOT NULL UNIQUE,
    pkce_verifier_ciphertext BYTEA NULL,
    pkce_nonce BYTEA NULL,
    encryption_key_version INTEGER NULL,
    client_kind TEXT NOT NULL CHECK (client_kind IN ('macos', 'ios', 'android')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'exchanging', 'completed', 'failed', 'expired', 'cancelled')),
    expires_at TIMESTAMPTZ NOT NULL,
    failure_code TEXT NULL CHECK (failure_code IS NULL OR char_length(failure_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (pkce_verifier_ciphertext IS NULL AND pkce_nonce IS NULL AND encryption_key_version IS NULL)
        OR (pkce_verifier_ciphertext IS NOT NULL AND pkce_nonce IS NOT NULL AND encryption_key_version IS NOT NULL)
    )
);

CREATE INDEX google_chat_oauth_authorizations_pending_idx
    ON google_chat_oauth_authorizations (expires_at, id)
    WHERE status IN ('pending', 'exchanging');

CREATE TABLE project_google_chat_sources (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    account_id UUID NOT NULL REFERENCES google_chat_accounts (id) ON DELETE CASCADE,
    space_name TEXT NOT NULL CHECK (space_name ~ '^spaces/[A-Za-z0-9_-]{1,240}$'),
    display_name TEXT NOT NULL CHECK (char_length(btrim(display_name)) BETWEEN 1 AND 500),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    acknowledge_with_reaction BOOLEAN NOT NULL DEFAULT TRUE,
    last_provider_message_at TIMESTAMPTZ NULL,
    last_successful_sync_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    FOREIGN KEY (project_id, user_id) REFERENCES projects (id, user_id) ON DELETE CASCADE,
    UNIQUE (project_id, account_id, space_name)
);

CREATE INDEX project_google_chat_sources_sync_idx
    ON project_google_chat_sources (last_successful_sync_at NULLS FIRST, id)
    WHERE enabled = TRUE;

CREATE TABLE project_inflow_items (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources (id) ON DELETE CASCADE,
    provider_message_name TEXT NOT NULL CHECK (char_length(provider_message_name) BETWEEN 1 AND 1024),
    provider_thread_name TEXT NULL CHECK (provider_thread_name IS NULL OR char_length(provider_thread_name) <= 1024),
    sender_name TEXT NULL CHECK (sender_name IS NULL OR char_length(sender_name) <= 500),
    content_text TEXT NOT NULL CHECK (char_length(btrim(content_text)) BETWEEN 1 AND 32768),
    received_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'promoted', 'dismissed')),
    promoted_task_id UUID NULL REFERENCES tasks (id) ON DELETE SET NULL,
    acknowledged_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    FOREIGN KEY (project_id, user_id) REFERENCES projects (id, user_id) ON DELETE CASCADE,
    UNIQUE (source_id, provider_message_name),
    CHECK (
        (status = 'promoted' AND promoted_task_id IS NOT NULL)
        OR (status <> 'promoted' AND promoted_task_id IS NULL)
    )
);

CREATE INDEX project_inflow_items_project_pending_idx
    ON project_inflow_items (project_id, received_at DESC, id DESC)
    WHERE status = 'pending';

CREATE OR REPLACE FUNCTION jimin_google_chat_account_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    IF ROW(
        NEW.email, NEW.status, NEW.granted_scopes,
        NEW.refresh_token_ciphertext, NEW.refresh_token_nonce,
        NEW.encryption_key_version, NEW.last_error_code
    ) IS DISTINCT FROM ROW(
        OLD.email, OLD.status, OLD.granted_scopes,
        OLD.refresh_token_ciphertext, OLD.refresh_token_nonce,
        OLD.encryption_key_version, OLD.last_error_code
    ) THEN
        NEW.version = OLD.version + 1;
    ELSE
        NEW.version = OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION jimin_google_chat_source_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    IF ROW(
        NEW.project_id, NEW.account_id, NEW.space_name, NEW.display_name,
        NEW.enabled, NEW.acknowledge_with_reaction, NEW.last_error_code
    ) IS DISTINCT FROM ROW(
        OLD.project_id, OLD.account_id, OLD.space_name, OLD.display_name,
        OLD.enabled, OLD.acknowledge_with_reaction, OLD.last_error_code
    ) THEN
        NEW.version = OLD.version + 1;
    ELSE
        NEW.version = OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION jimin_project_inflow_item_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    IF ROW(NEW.status, NEW.promoted_task_id) IS DISTINCT FROM
       ROW(OLD.status, OLD.promoted_task_id) THEN
        NEW.version = OLD.version + 1;
    ELSE
        NEW.version = OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER google_chat_accounts_set_updated_at
BEFORE UPDATE ON google_chat_accounts
FOR EACH ROW EXECUTE FUNCTION jimin_google_chat_account_set_updated_at();

CREATE TRIGGER google_chat_oauth_authorizations_set_updated_at
BEFORE UPDATE ON google_chat_oauth_authorizations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER project_google_chat_sources_set_updated_at
BEFORE UPDATE ON project_google_chat_sources
FOR EACH ROW EXECUTE FUNCTION jimin_google_chat_source_set_updated_at();

CREATE TRIGGER project_inflow_items_set_updated_at
BEFORE UPDATE ON project_inflow_items
FOR EACH ROW EXECUTE FUNCTION jimin_project_inflow_item_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 29
WHERE singleton = TRUE;
