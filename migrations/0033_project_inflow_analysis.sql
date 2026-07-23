-- Google Chat messages remain immutable evidence. One durable AI analysis is
-- maintained per source conversation so UI and task creation never need to
-- promote raw provider text.
CREATE TABLE project_inflow_analyses (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources (id) ON DELETE CASCADE,
    conversation_key TEXT NOT NULL CHECK (
        char_length(conversation_key) BETWEEN 1 AND 2048
    ),
    representative_item_id UUID NOT NULL REFERENCES project_inflow_items (id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'queued' CHECK (
        state IN ('queued', 'claimed', 'running', 'ready', 'failed')
    ),
    classification TEXT NULL CHECK (
        classification IS NULL OR classification IN (
            'new_task', 'follow_up', 'question', 'status_update', 'noise', 'duplicate'
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
    linked_task_id UUID NULL REFERENCES tasks (id) ON DELETE SET NULL,
    analysis_model_id TEXT NULL CHECK (
        analysis_model_id IS NULL OR char_length(analysis_model_id) BETWEEN 1 AND 128
    ),
    analysis_version TEXT NULL CHECK (
        analysis_version IS NULL OR char_length(analysis_version) BETWEEN 1 AND 64
    ),
    source_revision INTEGER NOT NULL DEFAULT 1 CHECK (source_revision > 0),
    analyzed_revision INTEGER NULL CHECK (analyzed_revision > 0),
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
    FOREIGN KEY (project_id, user_id) REFERENCES projects (id, user_id) ON DELETE CASCADE,
    UNIQUE (source_id, conversation_key),
    CHECK (
        (state IN ('claimed', 'running') AND claim_owner IS NOT NULL
            AND claim_expires_at IS NOT NULL)
        OR
        (state NOT IN ('claimed', 'running') AND claim_owner IS NULL
            AND claim_expires_at IS NULL)
    ),
    CHECK (
        state <> 'ready'
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
    )
);

CREATE INDEX project_inflow_analyses_claimable_idx
    ON project_inflow_analyses (created_at, id)
    WHERE state = 'queued';

CREATE INDEX project_inflow_analyses_expired_lease_idx
    ON project_inflow_analyses (claim_expires_at, id)
    WHERE state IN ('claimed', 'running');

CREATE INDEX project_inflow_analyses_user_attention_idx
    ON project_inflow_analyses (user_id, state, updated_at DESC, id DESC);

CREATE TRIGGER project_inflow_analyses_set_updated_at
BEFORE UPDATE ON project_inflow_analyses
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

-- Existing pending conversations are queued once. New messages are queued by
-- the ingestion transaction after this migration is active.
WITH pending_conversations AS (
    SELECT DISTINCT ON (
        item.source_id,
        COALESCE(
            'thread:' || item.provider_thread_name,
            'message:' || item.provider_message_name
        )
    )
        item.user_id,
        item.project_id,
        item.source_id,
        COALESCE(
            'thread:' || item.provider_thread_name,
            'message:' || item.provider_message_name
        ) AS conversation_key,
        item.id AS representative_item_id
    FROM project_inflow_items AS item
    WHERE item.status = 'pending'
    ORDER BY
        item.source_id,
        COALESCE(
            'thread:' || item.provider_thread_name,
            'message:' || item.provider_message_name
        ),
        item.received_at DESC,
        item.id DESC
)
INSERT INTO project_inflow_analyses (
    id, user_id, project_id, source_id, conversation_key,
    representative_item_id
)
SELECT
    representative_item_id, user_id, project_id, source_id, conversation_key,
    representative_item_id
FROM pending_conversations;

UPDATE jimin_schema_metadata
SET schema_version = 33
WHERE singleton = TRUE;
