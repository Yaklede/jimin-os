CREATE TABLE project_webhooks (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    url TEXT NOT NULL CHECK (char_length(url) BETWEEN 8 AND 4096),
    events TEXT[] NOT NULL CHECK (cardinality(events) BETWEEN 1 AND 16),
    auth_header_ciphertext BYTEA NULL,
    auth_header_nonce BYTEA NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK ((auth_header_ciphertext IS NULL AND auth_header_nonce IS NULL)
        OR (auth_header_ciphertext IS NOT NULL AND auth_header_nonce IS NOT NULL)),
    UNIQUE (project_id, url)
);

CREATE INDEX project_webhooks_project_idx
    ON project_webhooks (project_id, created_at, id);

CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    -- Delivery rows keep an immutable destination snapshot. This lets an event
    -- that was queued immediately before a webhook or project deletion finish
    -- safely without retaining the live configuration row.
    project_id UUID NOT NULL,
    webhook_id UUID NOT NULL,
    destination_url TEXT NOT NULL CHECK (char_length(destination_url) BETWEEN 8 AND 4096),
    auth_header_ciphertext BYTEA NULL,
    auth_header_nonce BYTEA NULL,
    event_type TEXT NOT NULL CHECK (char_length(event_type) BETWEEN 3 AND 80),
    payload JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'sending', 'retry_wait', 'delivered', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 10),
    next_attempt_at TIMESTAMPTZ NULL,
    response_code INTEGER NULL CHECK (response_code IS NULL OR response_code BETWEEN 100 AND 599),
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    delivered_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK ((auth_header_ciphertext IS NULL AND auth_header_nonce IS NULL)
        OR (auth_header_ciphertext IS NOT NULL AND auth_header_nonce IS NOT NULL))
);

CREATE INDEX webhook_deliveries_claimable_idx
    ON webhook_deliveries (COALESCE(next_attempt_at, created_at), id)
    WHERE status IN ('queued', 'retry_wait');

CREATE INDEX webhook_deliveries_project_history_idx
    ON webhook_deliveries (project_id, created_at DESC, id DESC);

CREATE TRIGGER project_webhooks_set_updated_at
BEFORE UPDATE ON project_webhooks
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER webhook_deliveries_set_updated_at
BEFORE UPDATE ON webhook_deliveries
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 19
WHERE singleton = TRUE;
