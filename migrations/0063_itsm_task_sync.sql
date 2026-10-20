-- No historical ticket is modified by this migration. Work is queued only
-- by a subsequent owner-authorized task mutation.
CREATE TABLE task_itsm_links (
    task_id UUID PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL,
    issue_url TEXT NULL CHECK (char_length(issue_url) <= 2048),
    issue_id BIGINT NULL CHECK (issue_id > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((enabled AND issue_url IS NOT NULL AND issue_id IS NOT NULL)
        OR (NOT enabled AND issue_url IS NULL AND issue_id IS NULL))
);
CREATE INDEX task_itsm_links_issue_idx ON task_itsm_links(issue_url) WHERE enabled;

-- One leased lane per upstream ticket prevents independent tasks/workers
-- from racing their deadline and completion writes.
CREATE TABLE itsm_issue_sync_jobs (
    id UUID PRIMARY KEY,
    issue_url TEXT NOT NULL UNIQUE CHECK (char_length(issue_url) BETWEEN 1 AND 2048),
    issue_id BIGINT NOT NULL CHECK (issue_id > 0),
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued','sending','retry_wait','failed','succeeded')),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    sync_deadline BOOLEAN NOT NULL DEFAULT FALSE,
    last_due_at TIMESTAMPTZ NULL,
    attempt_count INT NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    lease_owner TEXT NULL,
    leased_version BIGINT NULL,
    lease_until TIMESTAMPTZ NULL,
    next_attempt_at TIMESTAMPTZ NULL,
    last_error_code TEXT NULL,
    last_synced_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((status='sending' AND lease_owner IS NOT NULL AND leased_version IS NOT NULL AND lease_until IS NOT NULL)
        OR (status<>'sending' AND lease_owner IS NULL AND leased_version IS NULL AND lease_until IS NULL))
);
CREATE INDEX itsm_issue_sync_claim_idx ON itsm_issue_sync_jobs(status,next_attempt_at,updated_at);
UPDATE jimin_schema_metadata SET schema_version=63 WHERE singleton=TRUE;
