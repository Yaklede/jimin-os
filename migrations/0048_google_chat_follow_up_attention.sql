-- Replies that arrive after a Google Chat thread was promoted belong to the
-- existing task. Return them to pending attention without losing that link.
-- The original constraint required every non-promoted row to have no task
-- link, so remove only that promotion-state constraint.
DO $$
DECLARE
    promotion_state_constraint TEXT;
BEGIN
    SELECT constraint_row.conname
    INTO promotion_state_constraint
    FROM pg_constraint AS constraint_row
    WHERE constraint_row.conrelid = 'project_inflow_items'::REGCLASS
      AND constraint_row.contype = 'c'
      AND pg_get_constraintdef(constraint_row.oid)
          LIKE '%status = ''promoted''%'
      AND pg_get_constraintdef(constraint_row.oid)
          LIKE '%promoted_task_id IS NULL%'
    LIMIT 1;

    IF promotion_state_constraint IS NULL THEN
        RAISE EXCEPTION
            'Google Chat inflow promotion-state constraint was not found';
    END IF;

    EXECUTE format(
        'ALTER TABLE project_inflow_items DROP CONSTRAINT %I',
        promotion_state_constraint
    );
END
$$;

ALTER TABLE project_inflow_items
    ADD CONSTRAINT project_inflow_items_promoted_task_link_check CHECK (
        status <> 'promoted' OR promoted_task_id IS NOT NULL
    );

UPDATE jimin_schema_metadata
SET schema_version = 48
WHERE singleton = TRUE;
