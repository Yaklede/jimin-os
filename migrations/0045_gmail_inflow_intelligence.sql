-- Workspace-scoped Gmail metadata becomes a durable assistant inbox. Provider
-- messages remain immutable evidence while one revisioned analysis is kept per
-- account/thread. Decisions are independent from the mailbox read state.
ALTER TABLE gmail_messages
    ADD COLUMN body_text TEXT NULL CHECK (
        body_text IS NULL OR char_length(body_text) <= 12000
    ),
    ADD COLUMN reference_links TEXT[] NOT NULL DEFAULT '{}' CHECK (
        cardinality(reference_links) <= 16
        AND array_position(reference_links, NULL) IS NULL
        AND char_length(array_to_string(reference_links, '')) <= 32768
    ),
    ADD COLUMN list_id TEXT NULL CHECK (
        list_id IS NULL OR char_length(list_id) <= 1024
    ),
    ADD COLUMN list_unsubscribe BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN precedence TEXT NULL CHECK (
        precedence IS NULL OR char_length(precedence) <= 80
    ),
    ADD COLUMN auto_submitted TEXT NULL CHECK (
        auto_submitted IS NULL OR char_length(auto_submitted) <= 80
    ),
    ADD CONSTRAINT gmail_messages_id_account_workspace_unique
        UNIQUE (id, account_id, workspace_id);

ALTER TABLE projects
    ADD CONSTRAINT projects_id_user_workspace_unique
        UNIQUE (id, user_id, workspace_id);

CREATE TABLE gmail_inflow_candidates (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL,
    account_id UUID NOT NULL,
    provider_thread_id TEXT NOT NULL CHECK (
        char_length(provider_thread_id) BETWEEN 1 AND 255
        AND provider_thread_id ~ '^[A-Za-z0-9_-]+$'
    ),
    representative_message_id UUID NOT NULL,
    analysis_state TEXT NOT NULL DEFAULT 'queued' CHECK (
        analysis_state IN ('queued', 'claimed', 'running', 'ready', 'failed')
    ),
    classification TEXT NULL CHECK (
        classification IS NULL OR classification IN (
            'new_task', 'follow_up', 'question', 'status_update',
            'automated', 'newsletter', 'marketing', 'noise', 'duplicate'
        )
    ),
    confidence SMALLINT NULL CHECK (confidence BETWEEN 0 AND 100),
    summary TEXT NULL CHECK (
        summary IS NULL OR char_length(btrim(summary)) BETWEEN 1 AND 2000
    ),
    suggested_task_title TEXT NULL CHECK (
        suggested_task_title IS NULL
        OR char_length(btrim(suggested_task_title)) BETWEEN 1 AND 200
    ),
    suggested_action_items TEXT[] NOT NULL DEFAULT '{}',
    suggested_completion_criteria TEXT NULL CHECK (
        suggested_completion_criteria IS NULL
        OR char_length(btrim(suggested_completion_criteria)) BETWEEN 1 AND 2000
    ),
    suggested_assignee_name TEXT NULL CHECK (
        suggested_assignee_name IS NULL
        OR char_length(btrim(suggested_assignee_name)) BETWEEN 1 AND 80
    ),
    suggested_due_at TIMESTAMPTZ NULL,
    suggested_priority SMALLINT NULL CHECK (suggested_priority BETWEEN 0 AND 3),
    decision_status TEXT NOT NULL DEFAULT 'pending' CHECK (
        decision_status IN ('pending', 'promoted', 'dismissed', 'deferred')
    ),
    promoted_project_id UUID NULL,
    promoted_task_id UUID NULL REFERENCES tasks (id) ON DELETE SET NULL,
    deferred_until TIMESTAMPTZ NULL,
    source_revision INTEGER NOT NULL DEFAULT 1 CHECK (source_revision > 0),
    analyzed_revision INTEGER NULL CHECK (analyzed_revision > 0),
    analysis_model_id TEXT NULL CHECK (
        analysis_model_id IS NULL OR char_length(analysis_model_id) BETWEEN 1 AND 128
    ),
    analysis_version TEXT NULL CHECK (
        analysis_version IS NULL OR char_length(analysis_version) BETWEEN 1 AND 64
    ),
    claim_owner TEXT NULL CHECK (
        claim_owner IS NULL OR char_length(claim_owner) BETWEEN 1 AND 200
    ),
    claim_expires_at TIMESTAMPTZ NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 8),
    error_code TEXT NULL CHECK (
        error_code IS NULL OR char_length(error_code) BETWEEN 1 AND 120
    ),
    analyzed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspaces (id, user_id) ON DELETE RESTRICT,
    FOREIGN KEY (account_id, user_id, workspace_id)
        REFERENCES gmail_accounts (id, user_id, workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (representative_message_id, account_id, workspace_id)
        REFERENCES gmail_messages (id, account_id, workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (promoted_project_id, user_id, workspace_id)
        REFERENCES projects (id, user_id, workspace_id)
        ON DELETE SET NULL (promoted_project_id),
    UNIQUE (account_id, provider_thread_id),
    CHECK (
        (analysis_state IN ('claimed', 'running')
            AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
        OR
        (analysis_state NOT IN ('claimed', 'running')
            AND claim_owner IS NULL AND claim_expires_at IS NULL)
    ),
    CHECK (
        analysis_state <> 'ready'
        OR (
            classification IS NOT NULL
            AND confidence IS NOT NULL
            AND summary IS NOT NULL
            AND analyzed_revision = source_revision
            AND analyzed_at IS NOT NULL
        )
    ),
    CHECK (
        classification IS DISTINCT FROM 'new_task'
        OR (
            suggested_task_title IS NOT NULL
            AND cardinality(suggested_action_items) BETWEEN 1 AND 8
            AND suggested_completion_criteria IS NOT NULL
            AND suggested_priority IS NOT NULL
        )
    ),
    CHECK (
        decision_status = 'promoted'
        OR (promoted_project_id IS NULL AND promoted_task_id IS NULL)
    ),
    CHECK (
        (decision_status = 'deferred' AND deferred_until IS NOT NULL)
        OR
        (decision_status <> 'deferred' AND deferred_until IS NULL)
    )
);

CREATE INDEX gmail_inflow_candidates_claimable_idx
    ON gmail_inflow_candidates (created_at, id)
    WHERE analysis_state = 'queued'
      AND decision_status IN ('pending', 'promoted');

CREATE INDEX gmail_inflow_candidates_attention_idx
    ON gmail_inflow_candidates (
        user_id, workspace_id, decision_status, created_at DESC, id DESC
    );

CREATE INDEX gmail_inflow_candidates_expired_lease_idx
    ON gmail_inflow_candidates (claim_expires_at, id)
    WHERE analysis_state IN ('claimed', 'running');

CREATE TRIGGER gmail_inflow_candidates_set_updated_at
BEFORE UPDATE ON gmail_inflow_candidates
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

-- Queue one candidate per currently cached thread. Subsequent syncs use the
-- same account/thread key and only advance the source revision for new mail.
INSERT INTO gmail_inflow_candidates (
    id, user_id, workspace_id, account_id, provider_thread_id,
    representative_message_id
)
SELECT DISTINCT ON (message.account_id, message.provider_thread_id)
    overlay(
        overlay(gen_random_uuid()::TEXT placing '7' from 15 for 1)
        placing '8' from 20 for 1
    )::UUID,
    account.user_id,
    message.workspace_id,
    message.account_id,
    message.provider_thread_id,
    message.id
FROM gmail_messages AS message
JOIN gmail_accounts AS account
  ON account.id = message.account_id
 AND account.workspace_id = message.workspace_id
WHERE message.provider_deleted_at IS NULL
ORDER BY message.account_id, message.provider_thread_id,
    message.received_at DESC NULLS LAST, message.id DESC;

UPDATE jimin_schema_metadata
SET schema_version = 45
WHERE singleton = TRUE;
