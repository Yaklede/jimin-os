-- Android device signals are uploaded only by an authenticated device session.
-- Call-log payloads are private user data, so rows are scoped by both user and
-- device and are never copied into the generic sync event stream.
CREATE TABLE device_signal_states (
    device_id UUID PRIMARY KEY REFERENCES devices (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    call_log_permission TEXT NOT NULL CHECK (
        call_log_permission IN ('not_determined', 'granted', 'denied', 'unavailable')
    ),
    platform_version TEXT NULL CHECK (
        platform_version IS NULL
        OR char_length(btrim(platform_version)) BETWEEN 1 AND 120
    ),
    app_version TEXT NULL CHECK (
        app_version IS NULL
        OR char_length(btrim(app_version)) BETWEEN 1 AND 120
    ),
    last_synced_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (user_id, device_id)
);

CREATE INDEX device_signal_states_user_idx
    ON device_signal_states (user_id, updated_at DESC);

CREATE TABLE device_missed_calls (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    source_event_id TEXT NOT NULL CHECK (
        char_length(btrim(source_event_id)) BETWEEN 1 AND 120
    ),
    occurred_at TIMESTAMPTZ NOT NULL,
    caller_name TEXT NULL CHECK (
        caller_name IS NULL
        OR char_length(btrim(caller_name)) BETWEEN 1 AND 120
    ),
    phone_number TEXT NULL CHECK (
        phone_number IS NULL
        OR char_length(btrim(phone_number)) BETWEEN 1 AND 64
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (device_id, source_event_id)
);

CREATE INDEX device_missed_calls_user_occurred_idx
    ON device_missed_calls (user_id, occurred_at DESC, id DESC);

CREATE TRIGGER device_signal_states_set_updated_at
BEFORE UPDATE ON device_signal_states
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 42
WHERE singleton = TRUE;
