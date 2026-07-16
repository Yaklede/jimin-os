-- A signal produces at most one recommendation while it remains active. When
-- the underlying condition resolves, a later recurrence creates a new signal
-- and may produce a new recommendation without repeating dismissed advice.

WITH ranked_recommendations AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY signal_id
            ORDER BY created_at ASC, id ASC
        ) AS signal_rank
    FROM recommendations
    WHERE signal_id IS NOT NULL
)
UPDATE recommendations
SET signal_id = NULL
WHERE id IN (
    SELECT id
    FROM ranked_recommendations
    WHERE signal_rank > 1
);

CREATE UNIQUE INDEX recommendations_signal_once_idx
    ON recommendations (signal_id)
    WHERE signal_id IS NOT NULL;

UPDATE jimin_schema_metadata
SET schema_version = 22
WHERE singleton = TRUE;
