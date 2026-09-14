-- Durable, owner-scoped recurring work. Execution slots survive app/server restarts.
CREATE TABLE scheduled_work (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    definition JSONB NOT NULL CHECK (jsonb_typeof(definition) = 'object'),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    next_run_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX scheduled_work_due_idx ON scheduled_work(next_run_at) WHERE enabled;
CREATE INDEX scheduled_work_owner_idx ON scheduled_work(user_id, workspace_id);

CREATE TABLE scheduled_work_runs (
    id UUID PRIMARY KEY,
    scheduled_work_id UUID NOT NULL REFERENCES scheduled_work(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scheduled_for TIMESTAMPTZ NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('primary', 'followup', 'manual')),
    status TEXT NOT NULL CHECK (status IN ('delivering', 'completed', 'skipped', 'failed')),
    reason TEXT,
    task_ids UUID[] NOT NULL DEFAULT '{}',
    messages JSONB NOT NULL DEFAULT '[]',
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX scheduled_work_slot_idx ON scheduled_work_runs(scheduled_work_id, scheduled_for, kind) WHERE kind <> 'manual';
CREATE INDEX scheduled_work_runs_owner_idx ON scheduled_work_runs(user_id, started_at DESC);
CREATE TABLE scheduled_work_run_deliveries (
    run_id UUID NOT NULL REFERENCES scheduled_work_runs(id) ON DELETE CASCADE,
    delivery_id UUID NOT NULL REFERENCES webhook_deliveries(id) ON DELETE CASCADE,
    PRIMARY KEY(run_id, delivery_id)
);

ALTER TABLE push_deliveries DROP CONSTRAINT push_deliveries_item_type_check;
ALTER TABLE push_deliveries ADD CONSTRAINT push_deliveries_item_type_check
    CHECK (item_type IN ('task', 'schedule', 'brief', 'weekly_report', 'google_chat_inflow', 'gmail_inflow', 'scheduled_work'));

UPDATE jimin_schema_metadata SET schema_version = 56 WHERE singleton = TRUE;

ALTER TABLE agent_jobs DROP CONSTRAINT agent_jobs_executed_action_type_check;
ALTER TABLE agent_jobs ADD CONSTRAINT agent_jobs_executed_action_type_check CHECK (
    executed_action_type IS NULL OR executed_action_type IN ('create_task','update_task','complete_task','cancel_task','create_schedule','update_schedule','cancel_schedule','create_project','update_project','delete_project','send_webhook_message','approve_recommendation','reject_recommendation','defer_recommendation','create_scheduled_work')
);
ALTER TABLE agent_job_action_executions DROP CONSTRAINT agent_job_action_executions_action_type_check;
ALTER TABLE agent_job_action_executions ADD CONSTRAINT agent_job_action_executions_action_type_check CHECK (
    action_type IN ('create_task','update_task','complete_task','cancel_task','create_schedule','update_schedule','cancel_schedule','create_project','update_project','delete_project','send_webhook_message','approve_recommendation','reject_recommendation','defer_recommendation','create_scheduled_work')
);
