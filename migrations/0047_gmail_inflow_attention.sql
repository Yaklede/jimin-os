-- A reply on an existing Gmail thread must return that thread to the top of
-- the assistant inbox even when the original candidate was created long ago.
-- Keep keyset pagination stable by pairing the latest attention timestamp with
-- the candidate UUID.
ALTER TABLE gmail_inflow_candidates
    ADD COLUMN attention_at TIMESTAMPTZ NULL;

UPDATE gmail_inflow_candidates
SET attention_at = COALESCE(updated_at, created_at);

ALTER TABLE gmail_inflow_candidates
    ALTER COLUMN attention_at SET NOT NULL,
    ALTER COLUMN attention_at SET DEFAULT NOW();

-- A promoted thread can return to pending review when a later reply arrives.
-- Keep the original task/project links so the assistant opens existing work
-- instead of creating a duplicate. The previous check tied links exclusively
-- to the `promoted` decision state, so remove only that specific constraint.
DO $$
DECLARE
    promotion_state_constraint TEXT;
BEGIN
    SELECT constraint_row.conname
    INTO promotion_state_constraint
    FROM pg_constraint AS constraint_row
    WHERE constraint_row.conrelid = 'gmail_inflow_candidates'::REGCLASS
      AND constraint_row.contype = 'c'
      AND pg_get_constraintdef(constraint_row.oid)
          LIKE '%decision_status = ''promoted''%'
      AND pg_get_constraintdef(constraint_row.oid)
          LIKE '%promoted_project_id IS NULL%'
      AND pg_get_constraintdef(constraint_row.oid)
          LIKE '%promoted_task_id IS NULL%'
    LIMIT 1;

    IF promotion_state_constraint IS NULL THEN
        RAISE EXCEPTION
            'gmail inflow promotion-state constraint was not found';
    END IF;

    EXECUTE format(
        'ALTER TABLE gmail_inflow_candidates DROP CONSTRAINT %I',
        promotion_state_constraint
    );
END
$$;

DROP INDEX gmail_inflow_candidates_attention_idx;
DROP INDEX gmail_inflow_candidates_claimable_idx;

CREATE INDEX gmail_inflow_candidates_attention_idx
    ON gmail_inflow_candidates (
        user_id, workspace_id, attention_at DESC, id DESC
    );

CREATE INDEX gmail_inflow_candidates_claimable_idx
    ON gmail_inflow_candidates (attention_at, id)
    WHERE analysis_state = 'queued'
      AND decision_status IN ('pending', 'promoted');

UPDATE jimin_schema_metadata
SET schema_version = 47
WHERE singleton = TRUE;
