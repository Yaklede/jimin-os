UPDATE projects
SET due_at = NULL,
    updated_at = NOW()
WHERE management_mode = 'operation'
  AND due_at IS NOT NULL;

ALTER TABLE projects
    ADD CONSTRAINT projects_operation_without_due_date
    CHECK (management_mode <> 'operation' OR due_at IS NULL);
