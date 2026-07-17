ALTER TABLE project_webhooks
    ADD COLUMN provider TEXT NOT NULL DEFAULT 'legacy'
        CHECK (provider IN ('legacy', 'google_chat', 'discord')),
    ADD COLUMN destination_ciphertext BYTEA NULL,
    ADD COLUMN destination_nonce BYTEA NULL,
    ADD COLUMN destination_hint TEXT NULL
        CHECK (destination_hint IS NULL OR char_length(destination_hint) BETWEEN 1 AND 120),
    ADD CONSTRAINT project_webhooks_destination_secret_pair CHECK (
        (destination_ciphertext IS NULL AND destination_nonce IS NULL)
        OR (destination_ciphertext IS NOT NULL AND destination_nonce IS NOT NULL)
    ),
    ADD CONSTRAINT project_webhooks_typed_destination_secret CHECK (
        provider = 'legacy'
        OR (destination_ciphertext IS NOT NULL AND destination_nonce IS NOT NULL AND destination_hint IS NOT NULL)
    );

ALTER TABLE webhook_deliveries
    ADD COLUMN provider TEXT NOT NULL DEFAULT 'legacy'
        CHECK (provider IN ('legacy', 'google_chat', 'discord')),
    ADD COLUMN destination_ciphertext BYTEA NULL,
    ADD COLUMN destination_nonce BYTEA NULL,
    ADD CONSTRAINT webhook_deliveries_destination_secret_pair CHECK (
        (destination_ciphertext IS NULL AND destination_nonce IS NULL)
        OR (destination_ciphertext IS NOT NULL AND destination_nonce IS NOT NULL)
    ),
    ADD CONSTRAINT webhook_deliveries_typed_destination_secret CHECK (
        provider = 'legacy'
        OR (destination_ciphertext IS NOT NULL AND destination_nonce IS NOT NULL)
    );

ALTER TABLE project_webhooks
    DROP CONSTRAINT project_webhooks_project_id_url_key;

CREATE INDEX project_webhooks_project_provider_idx
    ON project_webhooks (project_id, provider)
    WHERE provider <> 'legacy';

UPDATE jimin_schema_metadata
SET schema_version = 23
WHERE singleton = TRUE;
