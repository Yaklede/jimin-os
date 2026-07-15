-- Attach server-owned schedules to the writable Google calendar and reuse the
-- existing durable Calendar mutation journal. A deterministic provider event
-- id makes a retried create converge on the same Google event.
CREATE TABLE schedule_calendar_links (
    schedule_entry_id UUID PRIMARY KEY REFERENCES schedule_entries (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id),
    account_id UUID NOT NULL REFERENCES calendar_accounts (id) ON DELETE CASCADE,
    calendar_id UUID NOT NULL REFERENCES calendars (id) ON DELETE CASCADE,
    provider_event_id TEXT NOT NULL CHECK (char_length(provider_event_id) BETWEEN 5 AND 1024),
    provider_etag TEXT NULL CHECK (provider_etag IS NULL OR char_length(provider_etag) <= 2048),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (calendar_id, provider_event_id)
);

CREATE INDEX schedule_calendar_links_owner_idx
    ON schedule_calendar_links (user_id, schedule_entry_id);

ALTER TABLE calendar_mutations
    ALTER COLUMN event_id DROP NOT NULL,
    ADD COLUMN schedule_entry_id UUID NULL REFERENCES schedule_entries (id) ON DELETE CASCADE;

ALTER TABLE calendar_mutations
    ADD CONSTRAINT calendar_mutations_single_source_check
    CHECK ((event_id IS NOT NULL) <> (schedule_entry_id IS NOT NULL));

CREATE UNIQUE INDEX calendar_mutations_schedule_version_idx
    ON calendar_mutations (schedule_entry_id, operation, expected_event_version)
    WHERE schedule_entry_id IS NOT NULL;

CREATE INDEX calendar_mutations_schedule_order_idx
    ON calendar_mutations (schedule_entry_id, created_at, id)
    WHERE schedule_entry_id IS NOT NULL
      AND status IN ('queued', 'claimed', 'sending', 'retry_wait');

CREATE TRIGGER schedule_calendar_links_set_updated_at
BEFORE UPDATE ON schedule_calendar_links
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

-- Webhook sends happen outside the database transaction. Persist the worker
-- lease so a process crash cannot leave an immutable delivery stuck in
-- `sending` forever. Rows claimed by the pre-lease worker are recovered once
-- during this migration and retain their stable delivery ID.
ALTER TABLE webhook_deliveries
    ADD COLUMN lease_owner TEXT NULL
        CHECK (lease_owner IS NULL OR char_length(lease_owner) BETWEEN 1 AND 200),
    ADD COLUMN lease_expires_at TIMESTAMPTZ NULL;

UPDATE webhook_deliveries
SET status = 'retry_wait',
    next_attempt_at = NOW(),
    last_error_code = 'webhook.worker_lease_expired'
WHERE status = 'sending';

ALTER TABLE webhook_deliveries
    ADD CONSTRAINT webhook_deliveries_lease_state_check
    CHECK (
        (status = 'sending' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR
        (status <> 'sending' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    );

CREATE INDEX webhook_deliveries_expired_lease_idx
    ON webhook_deliveries (lease_expires_at, id)
    WHERE status = 'sending';

UPDATE jimin_schema_metadata
SET schema_version = 20
WHERE singleton = TRUE;
