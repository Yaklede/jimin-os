-- A Jimin OS device is enrolled by an explicit, short-lived pairing token.
-- Google identity remains reserved for the later Google Calendar integration;
-- it is not the application's primary sign-in mechanism.
ALTER TABLE users
    ALTER COLUMN google_sub DROP NOT NULL,
    ALTER COLUMN email DROP NOT NULL,
    ALTER COLUMN normalized_email DROP NOT NULL,
    ADD COLUMN identity_kind TEXT NOT NULL DEFAULT 'google'
        CHECK (identity_kind IN ('google', 'local_device'));

CREATE UNIQUE INDEX users_single_local_owner_idx
    ON users (identity_kind)
    WHERE identity_kind = 'local_device';

CREATE TABLE device_pairing_tokens (
    id UUID PRIMARY KEY,
    owner_user_id UUID NOT NULL REFERENCES users (id),
    token_verifier BYTEA NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'consumed', 'revoked', 'expired')),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (status = 'consumed' AND consumed_at IS NOT NULL)
        OR (status <> 'consumed' AND consumed_at IS NULL)
    )
);

CREATE INDEX device_pairing_tokens_pending_expiry_idx
    ON device_pairing_tokens (status, expires_at);

CREATE TRIGGER device_pairing_tokens_set_updated_at
BEFORE UPDATE ON device_pairing_tokens
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 3
WHERE singleton = TRUE;
