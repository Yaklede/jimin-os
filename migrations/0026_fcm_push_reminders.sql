-- Android push registrations are bound to an authenticated device. Tokens are
-- encrypted by the API before they reach storage and are never exposed by a
-- read endpoint.
CREATE TABLE push_registrations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT 'fcm' CHECK (provider = 'fcm'),
    token_ciphertext BYTEA NOT NULL CHECK (octet_length(token_ciphertext) BETWEEN 17 AND 8192),
    token_nonce BYTEA NOT NULL CHECK (octet_length(token_nonce) = 24),
    token_fingerprint BYTEA NOT NULL CHECK (octet_length(token_fingerprint) = 32),
    key_version INTEGER NOT NULL DEFAULT 1 CHECK (key_version > 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'invalidated')),
    last_error_code TEXT NULL CHECK (
        last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120
    ),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_delivered_at TIMESTAMPTZ NULL,
    invalidated_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (device_id)
);

CREATE INDEX push_registrations_user_active_idx
    ON push_registrations (user_id, device_id)
    WHERE status = 'active';

CREATE UNIQUE INDEX push_registrations_active_token_idx
    ON push_registrations (token_fingerprint)
    WHERE status = 'active';

-- A delivery snapshots only the user-visible reminder payload. The token stays
-- in push_registrations so token rotation and invalidation take effect without
-- rewriting queued deliveries.
CREATE TABLE push_deliveries (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    item_type TEXT NOT NULL CHECK (item_type IN ('task', 'schedule')),
    item_id UUID NOT NULL,
    item_version BIGINT NOT NULL CHECK (item_version > 0),
    destination TEXT NOT NULL CHECK (destination IN ('home', 'calendar', 'projects')),
    project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 120),
    body TEXT NOT NULL CHECK (char_length(btrim(body)) BETWEEN 1 AND 240),
    target_at TIMESTAMPTZ NOT NULL,
    notify_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (
        status IN ('queued', 'sending', 'retry_wait', 'delivered', 'failed', 'cancelled')
    ),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 8),
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner TEXT NULL CHECK (
        lease_owner IS NULL OR char_length(lease_owner) BETWEEN 1 AND 200
    ),
    lease_expires_at TIMESTAMPTZ NULL,
    response_code INTEGER NULL CHECK (
        response_code IS NULL OR response_code BETWEEN 100 AND 599
    ),
    last_error_code TEXT NULL CHECK (
        last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120
    ),
    delivered_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (device_id, item_type, item_id, item_version),
    CHECK ((destination = 'projects') = (project_id IS NOT NULL)),
    CHECK (notify_at <= target_at),
    CHECK (
        (status = 'sending' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR
        (status <> 'sending' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    )
);

CREATE INDEX push_deliveries_claimable_idx
    ON push_deliveries (COALESCE(next_attempt_at, notify_at), id)
    WHERE status IN ('queued', 'retry_wait');

CREATE INDEX push_deliveries_expired_lease_idx
    ON push_deliveries (lease_expires_at, id)
    WHERE status = 'sending';

CREATE INDEX push_deliveries_item_history_idx
    ON push_deliveries (user_id, item_type, item_id, created_at DESC);

CREATE TRIGGER push_registrations_set_updated_at
BEFORE UPDATE ON push_registrations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER push_deliveries_set_updated_at
BEFORE UPDATE ON push_deliveries
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 26
WHERE singleton = TRUE;
