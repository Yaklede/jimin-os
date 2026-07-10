CREATE OR REPLACE FUNCTION jimin_set_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.version = OLD.version + 1;
    RETURN NEW;
END;
$$;

CREATE TABLE users (
    id UUID PRIMARY KEY,
    google_sub TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL,
    normalized_email TEXT NOT NULL,
    display_name TEXT NULL,
    time_zone TEXT NOT NULL DEFAULT 'Asia/Seoul',
    status TEXT NOT NULL CHECK (status IN ('active', 'disabled')),
    last_login_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE UNIQUE INDEX users_normalized_email_idx ON users (normalized_email);

CREATE TABLE devices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    installation_id UUID NOT NULL,
    platform TEXT NOT NULL CHECK (platform IN ('macos', 'ios', 'android')),
    name TEXT NOT NULL,
    app_version TEXT NOT NULL,
    os_version TEXT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
    last_seen_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, installation_id)
);

CREATE INDEX devices_user_id_idx ON devices (user_id, status);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    device_id UUID NOT NULL REFERENCES devices (id),
    family_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked', 'compromised', 'expired')),
    expires_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ NULL,
    revocation_reason TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id, status);
CREATE INDEX sessions_device_id_idx ON sessions (device_id, status);
CREATE INDEX sessions_family_id_idx ON sessions (family_id);

CREATE TABLE session_refresh_tokens (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions (id),
    token_verifier BYTEA NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('active', 'rotated', 'revoked', 'reused')),
    expires_at TIMESTAMPTZ NOT NULL,
    rotated_to_id UUID NULL REFERENCES session_refresh_tokens (id),
    used_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE INDEX session_refresh_tokens_session_id_idx ON session_refresh_tokens (session_id, status);

CREATE TABLE sync_changes (
    sequence BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
    entity_version BIGINT NOT NULL CHECK (entity_version > 0),
    changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX sync_changes_user_sequence_idx ON sync_changes (user_id, sequence);
CREATE INDEX sync_changes_entity_history_idx ON sync_changes (user_id, entity_type, entity_id, sequence DESC);

CREATE TABLE idempotency_records (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    idempotency_key TEXT NOT NULL,
    operation TEXT NOT NULL,
    request_hash BYTEA NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'completed', 'failed')),
    locked_until TIMESTAMPTZ NULL,
    response_status SMALLINT NULL,
    response_body JSONB NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, idempotency_key, operation)
);

CREATE INDEX idempotency_records_pending_idx ON idempotency_records (state, locked_until);

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_user_id UUID NULL REFERENCES users (id),
    actor_device_id UUID NULL REFERENCES devices (id),
    action TEXT NOT NULL,
    target_type TEXT NULL,
    target_id UUID NULL,
    outcome TEXT NOT NULL,
    request_id UUID NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB
);

CREATE INDEX audit_logs_occurred_at_idx ON audit_logs (occurred_at DESC);
CREATE INDEX audit_logs_actor_user_id_idx ON audit_logs (actor_user_id, occurred_at DESC);

CREATE TRIGGER users_set_updated_at
BEFORE UPDATE ON users
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER devices_set_updated_at
BEFORE UPDATE ON devices
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER sessions_set_updated_at
BEFORE UPDATE ON sessions
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER session_refresh_tokens_set_updated_at
BEFORE UPDATE ON session_refresh_tokens
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER idempotency_records_set_updated_at
BEFORE UPDATE ON idempotency_records
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 2
WHERE singleton = TRUE;
