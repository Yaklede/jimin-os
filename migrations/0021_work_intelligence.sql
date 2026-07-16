-- P1 turns the existing planning CRUD into a decision loop. These tables keep
-- observed signals, assistant recommendations, explicit owner decisions, and
-- verified action outcomes separate so a task is never mistaken for advice.

CREATE TABLE goals (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NULL REFERENCES workspaces (id) ON DELETE SET NULL,
    project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    desired_outcome TEXT NOT NULL
        CHECK (char_length(btrim(desired_outcome)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'paused', 'achieved', 'cancelled')),
    target_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (id, user_id)
);

CREATE INDEX goals_user_status_target_idx
    ON goals (user_id, status, target_at NULLS LAST, updated_at DESC);

CREATE TABLE intelligence_signals (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NULL REFERENCES workspaces (id) ON DELETE SET NULL,
    project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL,
    goal_id UUID NULL REFERENCES goals (id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK (
        kind IN (
            'project_risk', 'task_deadline', 'schedule_conflict', 'workload',
            'opportunity', 'external_issue', 'custom'
        )
    ),
    severity SMALLINT NOT NULL CHECK (severity BETWEEN 0 AND 3),
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    summary TEXT NOT NULL CHECK (char_length(btrim(summary)) BETWEEN 1 AND 2000),
    source_type TEXT NOT NULL CHECK (
        source_type IN ('schedule', 'task', 'project', 'inbox', 'harness', 'system', 'manual')
    ),
    source_entity_id UUID NULL,
    fingerprint TEXT NOT NULL CHECK (char_length(btrim(fingerprint)) BETWEEN 8 AND 200),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'resolved', 'expired')),
    observed_at TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ NULL,
    resolved_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (valid_until IS NULL OR valid_until > observed_at),
    CHECK (
        (status = 'active' AND resolved_at IS NULL)
        OR (status IN ('resolved', 'expired') AND resolved_at IS NOT NULL)
    ),
    UNIQUE (id, user_id)
);

CREATE UNIQUE INDEX intelligence_signals_active_fingerprint_idx
    ON intelligence_signals (user_id, fingerprint)
    WHERE status = 'active';

CREATE INDEX intelligence_signals_user_attention_idx
    ON intelligence_signals (user_id, severity DESC, observed_at DESC, id DESC)
    WHERE status = 'active';

CREATE TABLE recommendations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NULL REFERENCES workspaces (id) ON DELETE SET NULL,
    project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL,
    goal_id UUID NULL REFERENCES goals (id) ON DELETE SET NULL,
    signal_id UUID NULL REFERENCES intelligence_signals (id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    rationale TEXT NOT NULL CHECK (char_length(btrim(rationale)) BETWEEN 1 AND 4000),
    expected_effect TEXT NOT NULL
        CHECK (char_length(btrim(expected_effect)) BETWEEN 1 AND 2000),
    risk_summary TEXT NULL CHECK (
        risk_summary IS NULL OR char_length(btrim(risk_summary)) BETWEEN 1 AND 2000
    ),
    confidence SMALLINT NOT NULL CHECK (confidence BETWEEN 0 AND 100),
    urgency SMALLINT NOT NULL CHECK (urgency BETWEEN 0 AND 3),
    impact SMALLINT NOT NULL CHECK (impact BETWEEN 0 AND 3),
    risk_level SMALLINT NOT NULL CHECK (risk_level BETWEEN 0 AND 3),
    effort_minutes INTEGER NULL CHECK (effort_minutes BETWEEN 1 AND 10080),
    suggested_action_kind TEXT NULL CHECK (
        suggested_action_kind IS NULL OR suggested_action_kind IN (
            'review', 'create_task', 'update_task', 'create_schedule',
            'update_project', 'run_webhook', 'request_analysis'
        )
    ),
    suggested_entity_id UUID NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN (
            'pending', 'approved', 'rejected', 'deferred',
            'analysis_requested', 'executing', 'executed', 'failed', 'expired'
        )
    ),
    valid_until TIMESTAMPTZ NULL,
    revisit_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (status = 'deferred' AND revisit_at IS NOT NULL)
        OR (status <> 'deferred' AND revisit_at IS NULL)
    ),
    UNIQUE (id, user_id)
);

CREATE INDEX recommendations_user_inbox_idx
    ON recommendations (
        user_id, urgency DESC, impact DESC, confidence DESC, created_at DESC, id DESC
    )
    WHERE status IN ('pending', 'deferred', 'analysis_requested');

CREATE TABLE recommendation_decisions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    recommendation_id UUID NOT NULL REFERENCES recommendations (id) ON DELETE CASCADE,
    decision TEXT NOT NULL
        CHECK (decision IN ('approve', 'reject', 'defer', 'request_analysis')),
    reason TEXT NULL CHECK (reason IS NULL OR char_length(reason) <= 2000),
    revisit_at TIMESTAMPTZ NULL,
    recommendation_version BIGINT NOT NULL CHECK (recommendation_version > 0),
    decided_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (
        (decision = 'defer' AND revisit_at IS NOT NULL)
        OR (decision <> 'defer' AND revisit_at IS NULL)
    ),
    UNIQUE (user_id, id)
);

CREATE INDEX recommendation_decisions_recommendation_history_idx
    ON recommendation_decisions (recommendation_id, decided_at DESC, id DESC);

CREATE TABLE recommendation_action_results (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    recommendation_id UUID NOT NULL REFERENCES recommendations (id) ON DELETE CASCADE,
    action_type TEXT NOT NULL CHECK (char_length(action_type) BETWEEN 3 AND 80),
    entity_id UUID NULL,
    status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed', 'partial', 'cancelled')),
    summary TEXT NOT NULL CHECK (char_length(btrim(summary)) BETWEEN 1 AND 2000),
    expected_effect TEXT NULL CHECK (expected_effect IS NULL OR char_length(expected_effect) <= 2000),
    actual_effect TEXT NULL CHECK (actual_effect IS NULL OR char_length(actual_effect) <= 2000),
    error_code TEXT NULL CHECK (error_code IS NULL OR char_length(error_code) BETWEEN 1 AND 120),
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (completed_at >= started_at)
);

CREATE INDEX recommendation_action_results_history_idx
    ON recommendation_action_results (recommendation_id, completed_at DESC, id DESC);

CREATE TABLE brief_runs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('daily', 'weekly', 'monthly', 'on_demand')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    as_of TIMESTAMPTZ NOT NULL,
    title TEXT NULL CHECK (title IS NULL OR char_length(btrim(title)) BETWEEN 1 AND 200),
    summary TEXT NULL CHECK (summary IS NULL OR char_length(summary) <= 10000),
    error_code TEXT NULL CHECK (error_code IS NULL OR char_length(error_code) BETWEEN 1 AND 120),
    started_at TIMESTAMPTZ NULL,
    finished_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (finished_at IS NULL OR started_at IS NOT NULL),
    CHECK (finished_at IS NULL OR finished_at >= started_at),
    CHECK (
        (status = 'queued' AND started_at IS NULL AND finished_at IS NULL)
        OR (status = 'running' AND started_at IS NOT NULL AND finished_at IS NULL)
        OR (status IN ('completed', 'failed') AND started_at IS NOT NULL AND finished_at IS NOT NULL)
    )
);

CREATE INDEX brief_runs_user_kind_history_idx
    ON brief_runs (user_id, kind, as_of DESC, id DESC);

CREATE TRIGGER goals_set_updated_at
BEFORE UPDATE ON goals
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER intelligence_signals_set_updated_at
BEFORE UPDATE ON intelligence_signals
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER recommendations_set_updated_at
BEFORE UPDATE ON recommendations
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER brief_runs_set_updated_at
BEFORE UPDATE ON brief_runs
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 21
WHERE singleton = TRUE;
