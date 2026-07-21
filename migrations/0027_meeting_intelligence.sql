-- Meeting intelligence keeps the source transcript, AI review, and approved
-- execution separate. Extracted actions never mutate planning data until the
-- owner explicitly approves them.
CREATE TABLE meetings (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NULL REFERENCES workspaces (id) ON DELETE SET NULL,
    project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    transcript TEXT NOT NULL CHECK (char_length(btrim(transcript)) BETWEEN 1 AND 120000),
    started_at TIMESTAMPTZ NULL,
    duration_seconds INTEGER NULL CHECK (duration_seconds BETWEEN 1 AND 43200),
    status TEXT NOT NULL DEFAULT 'queued' CHECK (
        status IN ('queued', 'analyzing', 'review_ready', 'applied', 'failed')
    ),
    summary TEXT NULL CHECK (
        summary IS NULL OR char_length(btrim(summary)) BETWEEN 1 AND 20000
    ),
    topics TEXT[] NOT NULL DEFAULT '{}',
    risks TEXT[] NOT NULL DEFAULT '{}',
    follow_up TEXT NULL CHECK (
        follow_up IS NULL OR char_length(btrim(follow_up)) BETWEEN 1 AND 4000
    ),
    analyzed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    UNIQUE (id, user_id)
);

CREATE INDEX meetings_user_recent_idx
    ON meetings (user_id, created_at DESC, id DESC);

CREATE INDEX meetings_user_attention_idx
    ON meetings (user_id, status, updated_at DESC, id DESC)
    WHERE status IN ('queued', 'analyzing', 'review_ready', 'failed');

CREATE TABLE meeting_decisions (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    content TEXT NOT NULL CHECK (char_length(btrim(content)) BETWEEN 1 AND 2000),
    rationale TEXT NULL CHECK (
        rationale IS NULL OR char_length(btrim(rationale)) BETWEEN 1 AND 2000
    ),
    source_excerpt TEXT NOT NULL CHECK (
        char_length(btrim(source_excerpt)) BETWEEN 1 AND 2000
    ),
    source_timestamp_seconds INTEGER NULL CHECK (source_timestamp_seconds >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX meeting_decisions_meeting_idx
    ON meeting_decisions (meeting_id, created_at, id);

CREATE TABLE meeting_action_items (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('task', 'schedule')),
    project_id UUID NULL REFERENCES projects (id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    notes TEXT NULL CHECK (
        notes IS NULL OR char_length(btrim(notes)) BETWEEN 1 AND 4000
    ),
    priority SMALLINT NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 3),
    due_at TIMESTAMPTZ NULL,
    starts_at TIMESTAMPTZ NULL,
    ends_at TIMESTAMPTZ NULL,
    time_zone TEXT NULL CHECK (
        time_zone IS NULL OR char_length(btrim(time_zone)) BETWEEN 1 AND 100
    ),
    source_excerpt TEXT NOT NULL CHECK (
        char_length(btrim(source_excerpt)) BETWEEN 1 AND 2000
    ),
    confidence SMALLINT NOT NULL CHECK (confidence BETWEEN 0 AND 100),
    status TEXT NOT NULL DEFAULT 'suggested' CHECK (
        status IN ('suggested', 'applied', 'rejected')
    ),
    target_entity_id UUID NOT NULL UNIQUE,
    applied_at TIMESTAMPTZ NULL,
    rejected_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (kind = 'task' AND starts_at IS NULL AND ends_at IS NULL AND time_zone IS NULL)
        OR
        (kind = 'schedule' AND starts_at IS NOT NULL AND ends_at IS NOT NULL
            AND ends_at > starts_at AND time_zone IS NOT NULL)
    ),
    CHECK (
        (status = 'suggested' AND applied_at IS NULL AND rejected_at IS NULL)
        OR (status = 'applied' AND applied_at IS NOT NULL AND rejected_at IS NULL)
        OR (status = 'rejected' AND applied_at IS NULL AND rejected_at IS NOT NULL)
    )
);

CREATE INDEX meeting_action_items_review_idx
    ON meeting_action_items (meeting_id, status, created_at, id);

CREATE TABLE meeting_analysis_jobs (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL UNIQUE REFERENCES meetings (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'queued' CHECK (
        state IN ('queued', 'claimed', 'running', 'completed', 'failed')
    ),
    claim_owner TEXT NULL CHECK (
        claim_owner IS NULL OR char_length(claim_owner) BETWEEN 1 AND 200
    ),
    claim_expires_at TIMESTAMPTZ NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 8),
    error_code TEXT NULL CHECK (
        error_code IS NULL OR char_length(error_code) BETWEEN 1 AND 120
    ),
    started_at TIMESTAMPTZ NULL,
    finished_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (state IN ('claimed', 'running') AND claim_owner IS NOT NULL
            AND claim_expires_at IS NOT NULL)
        OR
        (state NOT IN ('claimed', 'running') AND claim_owner IS NULL
            AND claim_expires_at IS NULL)
    )
);

CREATE INDEX meeting_analysis_jobs_claimable_idx
    ON meeting_analysis_jobs (created_at, id)
    WHERE state = 'queued';

CREATE INDEX meeting_analysis_jobs_expired_lease_idx
    ON meeting_analysis_jobs (claim_expires_at, id)
    WHERE state IN ('claimed', 'running');

CREATE TRIGGER meetings_set_updated_at
BEFORE UPDATE ON meetings
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER meeting_action_items_set_updated_at
BEFORE UPDATE ON meeting_action_items
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER meeting_analysis_jobs_set_updated_at
BEFORE UPDATE ON meeting_analysis_jobs
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 27
WHERE singleton = TRUE;
