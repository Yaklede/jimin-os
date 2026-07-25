-- Give the home assistant one durable conversation identity that can be
-- resumed from every trusted client. Existing installations keep the most
-- recently used active conversation as the initial home context.
ALTER TABLE conversations
    ADD COLUMN surface TEXT NOT NULL DEFAULT 'chat'
    CHECK (surface IN ('home', 'chat'));

WITH latest_active AS (
    SELECT DISTINCT ON (user_id) id
    FROM conversations
    WHERE status = 'active'
    ORDER BY user_id, last_message_at DESC NULLS LAST, created_at DESC, id DESC
)
UPDATE conversations
SET surface = 'home'
WHERE id IN (SELECT id FROM latest_active);

CREATE UNIQUE INDEX conversations_one_active_home_idx
    ON conversations (user_id)
    WHERE status = 'active' AND surface = 'home';

UPDATE jimin_schema_metadata
SET schema_version = 39
WHERE singleton = TRUE;
