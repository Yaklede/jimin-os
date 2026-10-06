-- Keep legacy tasks unclassified; never infer a user's intended work type.
ALTER TABLE tasks
    ADD COLUMN work_kind TEXT NOT NULL DEFAULT 'general'
        CHECK (work_kind IN ('general', 'verification', 'development')),
    ADD COLUMN completion_note TEXT NULL
        CHECK (completion_note IS NULL OR char_length(btrim(completion_note)) BETWEEN 1 AND 2000);

-- Snapshot the result together with the completion version for idempotent replies.
ALTER TABLE google_chat_task_completion_deliveries
    ADD COLUMN work_kind TEXT NOT NULL DEFAULT 'general'
        CHECK (work_kind IN ('general', 'verification', 'development')),
    ADD COLUMN completion_note TEXT NULL
        CHECK (completion_note IS NULL OR char_length(btrim(completion_note)) BETWEEN 1 AND 2000);

UPDATE jimin_schema_metadata SET schema_version = 58 WHERE singleton = TRUE;
