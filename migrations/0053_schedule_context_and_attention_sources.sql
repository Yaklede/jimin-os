-- Schedule entries can now point at the project and task they reserve time
-- for. Both links stay owner-scoped and become null when the referenced work
-- is removed, so existing calendar rows remain readable.
ALTER TABLE tasks
    ADD CONSTRAINT tasks_id_user_unique UNIQUE (id, user_id);

ALTER TABLE schedule_entries
    ADD COLUMN project_id UUID NULL,
    ADD COLUMN task_id UUID NULL,
    ADD CONSTRAINT schedule_entries_project_owner_fk
        FOREIGN KEY (project_id, user_id)
        REFERENCES projects (id, user_id)
        ON DELETE SET NULL (project_id),
    ADD CONSTRAINT schedule_entries_task_owner_fk
        FOREIGN KEY (task_id, user_id)
        REFERENCES tasks (id, user_id)
        ON DELETE SET NULL (task_id);

CREATE INDEX schedule_entries_project_time_idx
    ON schedule_entries (project_id, starts_at, ends_at)
    WHERE status = 'confirmed' AND project_id IS NOT NULL;

CREATE INDEX schedule_entries_task_idx
    ON schedule_entries (task_id)
    WHERE task_id IS NOT NULL;

-- Weekly snapshots persist the number of analyzed inflows that still require
-- an explicit task decision. Defaults keep all existing snapshots compatible.
ALTER TABLE weekly_report_snapshots
    ADD COLUMN actionable_chat_inflow_count BIGINT NOT NULL DEFAULT 0
        CHECK (actionable_chat_inflow_count >= 0),
    ADD COLUMN actionable_gmail_inflow_count BIGINT NOT NULL DEFAULT 0
        CHECK (actionable_gmail_inflow_count >= 0);

-- Actionable analyzed inflows share the durable per-device push queue. Their
-- source version is the idempotency boundary, so a later reply may notify once
-- more while an unchanged candidate never duplicates a delivery.
ALTER TABLE push_deliveries
DROP CONSTRAINT IF EXISTS push_deliveries_item_type_check;

ALTER TABLE push_deliveries
ADD CONSTRAINT push_deliveries_item_type_check
CHECK (
    item_type IN (
        'task', 'schedule', 'brief', 'weekly_report',
        'google_chat_inflow', 'gmail_inflow'
    )
);

UPDATE jimin_schema_metadata
SET schema_version = 53
WHERE singleton = TRUE;
