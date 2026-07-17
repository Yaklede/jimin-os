DELETE FROM webhook_deliveries
WHERE provider = 'legacy';

DELETE FROM project_webhooks
WHERE provider = 'legacy';

DROP INDEX project_webhooks_project_provider_idx;

ALTER TABLE project_webhooks
    DROP CONSTRAINT project_webhooks_provider_check,
    DROP CONSTRAINT project_webhooks_destination_secret_pair,
    DROP CONSTRAINT project_webhooks_typed_destination_secret,
    DROP COLUMN url,
    DROP COLUMN auth_header_ciphertext,
    DROP COLUMN auth_header_nonce,
    ALTER COLUMN provider DROP DEFAULT,
    ALTER COLUMN destination_ciphertext SET NOT NULL,
    ALTER COLUMN destination_nonce SET NOT NULL,
    ALTER COLUMN destination_hint SET NOT NULL,
    ADD CONSTRAINT project_webhooks_provider_check
        CHECK (provider IN ('google_chat', 'discord'));

ALTER TABLE webhook_deliveries
    DROP CONSTRAINT webhook_deliveries_provider_check,
    DROP CONSTRAINT webhook_deliveries_destination_secret_pair,
    DROP CONSTRAINT webhook_deliveries_typed_destination_secret,
    DROP COLUMN destination_url,
    DROP COLUMN auth_header_ciphertext,
    DROP COLUMN auth_header_nonce,
    ALTER COLUMN provider DROP DEFAULT,
    ALTER COLUMN destination_ciphertext SET NOT NULL,
    ALTER COLUMN destination_nonce SET NOT NULL,
    ADD CONSTRAINT webhook_deliveries_provider_check
        CHECK (provider IN ('google_chat', 'discord'));

CREATE INDEX project_webhooks_project_provider_idx
    ON project_webhooks (project_id, provider);

UPDATE jimin_schema_metadata
SET schema_version = 24
WHERE singleton = TRUE;
