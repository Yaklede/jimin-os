-- Assistant presentations are nullable, server-validated read models attached
-- to completed messages. Existing clients continue to use message content,
-- and rollback remains a restore of the previous image plus database backup.
ALTER TABLE messages
    ADD COLUMN presentation JSONB NULL
    CHECK (presentation IS NULL OR jsonb_typeof(presentation) = 'object');

UPDATE jimin_schema_metadata
SET schema_version = 14
WHERE singleton = TRUE;
