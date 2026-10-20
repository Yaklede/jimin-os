-- Additive workflow metadata. Existing open/completed/cancelled lifecycle stays intact.
ALTER TABLE tasks ADD COLUMN workflow_state TEXT NOT NULL DEFAULT 'not_started';
ALTER TABLE tasks ADD COLUMN waiting_reason TEXT;
ALTER TABLE tasks ADD CONSTRAINT tasks_workflow_state_check
  CHECK (workflow_state IN ('not_started', 'in_progress', 'waiting'));
ALTER TABLE tasks ADD CONSTRAINT tasks_waiting_reason_check
  CHECK ((workflow_state = 'waiting' AND waiting_reason IS NOT NULL
          AND length(btrim(waiting_reason)) BETWEEN 1 AND 1000)
         OR (workflow_state <> 'waiting' AND waiting_reason IS NULL));
CREATE INDEX tasks_waiting_owner_idx ON tasks (user_id, project_id)
  WHERE status = 'open' AND workflow_state = 'waiting';
UPDATE jimin_schema_metadata SET schema_version = 60 WHERE singleton = TRUE;
