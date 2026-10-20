-- Keep the explicitly reviewed outbound description separate from internal
-- task notes and the original AI analysis. NULL preserves legacy delivery.
ALTER TABLE task_assignment_public_details
ADD COLUMN reviewed_content TEXT NULL CHECK (
    reviewed_content IS NULL OR char_length(reviewed_content) <= 10000
);

UPDATE jimin_schema_metadata SET schema_version = 62 WHERE singleton = TRUE;
