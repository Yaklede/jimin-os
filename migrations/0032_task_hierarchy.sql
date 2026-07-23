-- Large outcomes can be split into one level of executable child tasks while
-- remaining in the same project. Deeper nesting is rejected by the service so
-- the task view stays understandable on desktop and mobile.
ALTER TABLE tasks
    ADD COLUMN parent_task_id UUID NULL REFERENCES tasks(id),
    ADD CONSTRAINT tasks_parent_not_self CHECK (
        parent_task_id IS NULL OR parent_task_id <> id
    );

CREATE INDEX tasks_parent_task_idx
    ON tasks (user_id, parent_task_id, status, due_at, id)
    WHERE parent_task_id IS NOT NULL;

UPDATE jimin_schema_metadata
SET schema_version = 32
WHERE singleton = TRUE;
