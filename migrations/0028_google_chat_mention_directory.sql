-- Google Chat mention directories are configuration, not credentials. Keep the
-- editable name-to-user mapping on the webhook and snapshot it on each delivery
-- so retries render the same mentions even after the configuration changes.
ALTER TABLE project_webhooks
    ADD COLUMN mention_directory JSONB NOT NULL DEFAULT '{"users": {}}'::jsonb,
    ADD CONSTRAINT project_webhooks_mention_directory_shape CHECK (
        jsonb_typeof(mention_directory) = 'object'
        AND mention_directory ? 'users'
        AND jsonb_typeof(mention_directory -> 'users') = 'object'
        AND (mention_directory - 'users') = '{}'::jsonb
        AND octet_length(mention_directory::text) <= 32768
    );

ALTER TABLE webhook_deliveries
    ADD COLUMN mention_directory JSONB NOT NULL DEFAULT '{"users": {}}'::jsonb,
    ADD CONSTRAINT webhook_deliveries_mention_directory_shape CHECK (
        jsonb_typeof(mention_directory) = 'object'
        AND mention_directory ? 'users'
        AND jsonb_typeof(mention_directory -> 'users') = 'object'
        AND (mention_directory - 'users') = '{}'::jsonb
        AND octet_length(mention_directory::text) <= 32768
    );

UPDATE jimin_schema_metadata
SET schema_version = 28
WHERE singleton = TRUE;
