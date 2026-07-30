-- Gmail History is an account-owned provider cursor. Keeping it beside the
-- existing account/workspace sync state prevents personal and company mailbox
-- progress from being merged and lets message persistence advance the cursor
-- atomically.
ALTER TABLE gmail_sync_states
    ADD COLUMN provider_history_id TEXT NULL CHECK (
        provider_history_id IS NULL
        OR provider_history_id ~ '^[0-9]{1,64}$'
    );

UPDATE jimin_schema_metadata
SET schema_version = 46
WHERE singleton = TRUE;
