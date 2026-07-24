-- Weekly operating reports need a durable source for week-over-week review
-- and one-time Friday notifications. The live report remains the current
-- source of truth; this table stores a bounded snapshot for each workspace
-- and week without copying task or message contents.
CREATE TABLE weekly_report_snapshots (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    created_task_count BIGINT NOT NULL CHECK (created_task_count >= 0),
    completed_task_count BIGINT NOT NULL CHECK (completed_task_count >= 0),
    backlog_start_count BIGINT NOT NULL CHECK (backlog_start_count >= 0),
    backlog_end_count BIGINT NOT NULL CHECK (backlog_end_count >= 0),
    overdue_task_count BIGINT NOT NULL CHECK (overdue_task_count >= 0),
    stale_task_count BIGINT NOT NULL CHECK (stale_task_count >= 0),
    unassigned_task_count BIGINT NOT NULL CHECK (unassigned_task_count >= 0),
    projects JSONB NOT NULL CHECK (jsonb_typeof(projects) = 'array'),
    generated_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version = 1),
    UNIQUE (user_id, workspace_id, period_start),
    CHECK (period_start < period_end),
    CHECK (generated_at >= period_start)
);

CREATE INDEX weekly_report_snapshots_history_idx
    ON weekly_report_snapshots (user_id, workspace_id, period_start DESC);

-- Weekly report notifications share the existing durable FCM queue. A stable
-- snapshot version makes each workspace/week notification idempotent even
-- while the current week's counters continue to refresh.
ALTER TABLE push_deliveries
DROP CONSTRAINT IF EXISTS push_deliveries_item_type_check;

ALTER TABLE push_deliveries
ADD CONSTRAINT push_deliveries_item_type_check
CHECK (item_type IN ('task', 'schedule', 'brief', 'weekly_report'));

UPDATE jimin_schema_metadata
SET schema_version = 38
WHERE singleton = TRUE;
