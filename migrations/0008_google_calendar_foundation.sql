-- Google Calendar remains a separate provider-owned source. These tables keep
-- OAuth secrets encrypted, preserve the last confirmed read model during a
-- failed sync, and make every outbound mutation durable before Google is
-- contacted. No provider token or event body belongs in application logs.
CREATE TABLE calendar_accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL UNIQUE REFERENCES users (id),
    provider TEXT NOT NULL CHECK (provider = 'google'),
    provider_subject TEXT NOT NULL,
    email TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('connecting', 'active', 'reauth_required', 'revoking', 'revoked', 'error')),
    granted_scopes TEXT[] NOT NULL DEFAULT '{}',
    refresh_token_ciphertext BYTEA NULL,
    refresh_token_nonce BYTEA NULL,
    encryption_key_version INTEGER NULL,
    calendar_list_sync_token_ciphertext BYTEA NULL,
    calendar_list_sync_token_nonce BYTEA NULL,
    last_successful_sync_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (refresh_token_ciphertext IS NULL AND refresh_token_nonce IS NULL AND encryption_key_version IS NULL)
        OR (refresh_token_ciphertext IS NOT NULL AND refresh_token_nonce IS NOT NULL AND encryption_key_version IS NOT NULL)
    ),
    CHECK (
        (calendar_list_sync_token_ciphertext IS NULL AND calendar_list_sync_token_nonce IS NULL)
        OR (calendar_list_sync_token_ciphertext IS NOT NULL AND calendar_list_sync_token_nonce IS NOT NULL)
    )
);

CREATE TABLE calendar_oauth_authorizations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    session_id UUID NOT NULL REFERENCES sessions (id),
    device_id UUID NOT NULL REFERENCES devices (id),
    state_verifier BYTEA NOT NULL UNIQUE,
    pkce_verifier_ciphertext BYTEA NOT NULL,
    pkce_nonce BYTEA NOT NULL,
    encryption_key_version INTEGER NOT NULL,
    client_kind TEXT NOT NULL CHECK (client_kind IN ('macos', 'ios', 'android')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'exchanging', 'completed', 'failed', 'expired', 'cancelled')),
    expires_at TIMESTAMPTZ NOT NULL,
    failure_code TEXT NULL CHECK (failure_code IS NULL OR char_length(failure_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE INDEX calendar_oauth_authorizations_pending_idx
    ON calendar_oauth_authorizations (expires_at, id)
    WHERE status IN ('pending', 'exchanging');

CREATE TABLE calendars (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES calendar_accounts (id) ON DELETE CASCADE,
    provider_calendar_id TEXT NOT NULL CHECK (char_length(provider_calendar_id) BETWEEN 1 AND 1024),
    name TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 500),
    description TEXT NULL CHECK (description IS NULL OR char_length(description) <= 8192),
    time_zone TEXT NOT NULL CHECK (char_length(time_zone) BETWEEN 1 AND 80),
    color_id TEXT NULL CHECK (color_id IS NULL OR char_length(color_id) <= 120),
    access_role TEXT NOT NULL CHECK (access_role IN ('free_busy_reader', 'reader', 'writer', 'owner')),
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    provider_selected BOOLEAN NOT NULL DEFAULT FALSE,
    sync_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    provider_etag TEXT NULL CHECK (provider_etag IS NULL OR char_length(provider_etag) <= 2048),
    provider_deleted_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (account_id, provider_calendar_id),
    CHECK (NOT is_primary OR sync_enabled)
);

CREATE TABLE calendar_sync_states (
    id UUID PRIMARY KEY,
    calendar_id UUID NOT NULL UNIQUE REFERENCES calendars (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('idle', 'queued', 'claimed', 'fetching', 'applying', 'retry_wait', 'reset_required', 'failed')),
    sync_token_ciphertext BYTEA NULL,
    sync_token_nonce BYTEA NULL,
    query_fingerprint TEXT NOT NULL CHECK (char_length(query_fingerprint) BETWEEN 1 AND 255),
    last_started_at TIMESTAMPTZ NULL,
    last_successful_sync_at TIMESTAMPTZ NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner TEXT NULL CHECK (lease_owner IS NULL OR char_length(lease_owner) BETWEEN 1 AND 200),
    lease_expires_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK ((sync_token_ciphertext IS NULL AND sync_token_nonce IS NULL) OR (sync_token_ciphertext IS NOT NULL AND sync_token_nonce IS NOT NULL))
);

CREATE TABLE calendar_events (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    calendar_id UUID NOT NULL REFERENCES calendars (id) ON DELETE CASCADE,
    provider_event_id TEXT NOT NULL CHECK (char_length(provider_event_id) BETWEEN 1 AND 1024),
    provider_etag TEXT NULL CHECK (provider_etag IS NULL OR char_length(provider_etag) <= 2048),
    provider_updated_at TIMESTAMPTZ NULL,
    ical_uid TEXT NULL CHECK (ical_uid IS NULL OR char_length(ical_uid) <= 2048),
    provider_status TEXT NOT NULL CHECK (provider_status IN ('confirmed', 'tentative', 'cancelled')),
    event_type TEXT NOT NULL CHECK (event_type IN ('default', 'birthday', 'focus_time', 'from_gmail', 'out_of_office', 'working_location')),
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 300),
    description_text TEXT NULL CHECK (description_text IS NULL OR char_length(description_text) <= 8192),
    location TEXT NULL CHECK (location IS NULL OR char_length(location) <= 1024),
    time_kind TEXT NOT NULL CHECK (time_kind IN ('date', 'date_time')),
    start_at TIMESTAMPTZ NULL,
    end_at TIMESTAMPTZ NULL,
    start_date DATE NULL,
    end_date DATE NULL,
    source_time_zone TEXT NULL CHECK (source_time_zone IS NULL OR char_length(source_time_zone) BETWEEN 1 AND 80),
    recurrence JSONB NULL,
    recurring_provider_event_id TEXT NULL CHECK (recurring_provider_event_id IS NULL OR char_length(recurring_provider_event_id) <= 1024),
    original_start JSONB NULL,
    visibility TEXT NULL CHECK (visibility IS NULL OR visibility IN ('default', 'public', 'private', 'confidential')),
    transparency TEXT NULL CHECK (transparency IS NULL OR transparency IN ('opaque', 'transparent')),
    html_link TEXT NULL CHECK (html_link IS NULL OR char_length(html_link) <= 4096),
    is_editable BOOLEAN NOT NULL DEFAULT FALSE,
    sync_state TEXT NOT NULL CHECK (sync_state IN ('synced', 'pending_create', 'pending_update', 'pending_delete', 'conflict', 'failed')),
    provider_deleted_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (calendar_id, provider_event_id),
    CHECK (
        (time_kind = 'date' AND start_date IS NOT NULL AND end_date IS NOT NULL AND end_date > start_date
            AND start_at IS NULL AND end_at IS NULL AND source_time_zone IS NULL)
        OR (time_kind = 'date_time' AND start_at IS NOT NULL AND end_at IS NOT NULL AND end_at > start_at
            AND source_time_zone IS NOT NULL AND start_date IS NULL AND end_date IS NULL)
    )
);

CREATE INDEX calendar_events_user_range_idx
    ON calendar_events (user_id, start_at, end_at)
    WHERE time_kind = 'date_time' AND provider_deleted_at IS NULL;

CREATE INDEX calendar_events_calendar_date_idx
    ON calendar_events (calendar_id, start_date, end_date)
    WHERE time_kind = 'date' AND provider_deleted_at IS NULL;

CREATE TABLE calendar_sync_runs (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES calendar_accounts (id) ON DELETE CASCADE,
    calendar_id UUID NULL REFERENCES calendars (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('full', 'incremental')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'claimed', 'fetching', 'applying', 'completed', 'retry_wait', 'failed', 'cancelled')),
    base_sync_token_fingerprint BYTEA NULL,
    next_sync_token_ciphertext BYTEA NULL,
    next_sync_token_nonce BYTEA NULL,
    encryption_key_version INTEGER NULL,
    page_count INTEGER NOT NULL DEFAULT 0 CHECK (page_count >= 0),
    item_count INTEGER NOT NULL DEFAULT 0 CHECK (item_count >= 0),
    lease_owner TEXT NULL CHECK (lease_owner IS NULL OR char_length(lease_owner) BETWEEN 1 AND 200),
    lease_expires_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK ((next_sync_token_ciphertext IS NULL AND next_sync_token_nonce IS NULL AND encryption_key_version IS NULL)
        OR (next_sync_token_ciphertext IS NOT NULL AND next_sync_token_nonce IS NOT NULL AND encryption_key_version IS NOT NULL))
);

CREATE UNIQUE INDEX calendar_sync_runs_one_active_idx
    ON calendar_sync_runs (calendar_id)
    WHERE calendar_id IS NOT NULL AND status IN ('queued', 'claimed', 'fetching', 'applying', 'retry_wait');

CREATE TABLE calendar_sync_staging_events (
    run_id UUID NOT NULL REFERENCES calendar_sync_runs (id) ON DELETE CASCADE,
    provider_event_id TEXT NOT NULL CHECK (char_length(provider_event_id) BETWEEN 1 AND 1024),
    normalized_payload JSONB NOT NULL,
    provider_status TEXT NOT NULL CHECK (provider_status IN ('confirmed', 'tentative', 'cancelled')),
    PRIMARY KEY (run_id, provider_event_id)
);

CREATE TABLE calendar_mutations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    event_id UUID NOT NULL REFERENCES calendar_events (id),
    operation TEXT NOT NULL CHECK (operation IN ('create', 'update', 'delete')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'claimed', 'sending', 'completed', 'retry_wait', 'conflict', 'failed', 'cancelled')),
    idempotency_record_id UUID NOT NULL REFERENCES idempotency_records (id),
    desired_payload JSONB NOT NULL,
    expected_event_version BIGINT NOT NULL CHECK (expected_event_version > 0),
    expected_provider_etag TEXT NULL CHECK (expected_provider_etag IS NULL OR char_length(expected_provider_etag) <= 2048),
    provider_event_id TEXT NOT NULL CHECK (char_length(provider_event_id) BETWEEN 1 AND 1024),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NULL,
    lease_owner TEXT NULL CHECK (lease_owner IS NULL OR char_length(lease_owner) BETWEEN 1 AND 200),
    lease_expires_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR char_length(last_error_code) BETWEEN 1 AND 120),
    resolved_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0)
);

CREATE INDEX calendar_mutations_claimable_idx
    ON calendar_mutations (created_at, id)
    WHERE status IN ('queued', 'retry_wait');

CREATE TRIGGER calendar_accounts_set_updated_at
BEFORE UPDATE ON calendar_accounts
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER calendar_oauth_authorizations_set_updated_at
BEFORE UPDATE ON calendar_oauth_authorizations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER calendars_set_updated_at
BEFORE UPDATE ON calendars
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER calendar_sync_states_set_updated_at
BEFORE UPDATE ON calendar_sync_states
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER calendar_events_set_updated_at
BEFORE UPDATE ON calendar_events
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER calendar_sync_runs_set_updated_at
BEFORE UPDATE ON calendar_sync_runs
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER calendar_mutations_set_updated_at
BEFORE UPDATE ON calendar_mutations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 8
WHERE singleton = TRUE;
