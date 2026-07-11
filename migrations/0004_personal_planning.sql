-- Server-owned personal planning records are available even when a Google
-- Calendar account is not connected. A later provider sync projects into this
-- same personal timeline without becoming the only source of truth.
CREATE TABLE schedule_entries (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    notes TEXT NULL CHECK (notes IS NULL OR char_length(notes) <= 10000),
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    time_zone TEXT NOT NULL CHECK (char_length(time_zone) BETWEEN 1 AND 80),
    source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'google')),
    status TEXT NOT NULL DEFAULT 'confirmed' CHECK (status IN ('confirmed', 'cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (ends_at > starts_at)
);

CREATE INDEX schedule_entries_user_time_idx
    ON schedule_entries (user_id, starts_at, ends_at)
    WHERE status = 'confirmed';

CREATE TABLE tasks (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id),
    title TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    notes TEXT NULL CHECK (notes IS NULL OR char_length(notes) <= 10000),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'completed', 'cancelled')),
    priority SMALLINT NOT NULL DEFAULT 1 CHECK (priority BETWEEN 0 AND 3),
    due_at TIMESTAMPTZ NULL,
    completed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    CHECK (
        (status = 'completed' AND completed_at IS NOT NULL)
        OR (status <> 'completed' AND completed_at IS NULL)
    )
);

CREATE INDEX tasks_user_open_due_idx
    ON tasks (user_id, priority DESC, due_at NULLS LAST, created_at)
    WHERE status = 'open';

CREATE TRIGGER schedule_entries_set_updated_at
BEFORE UPDATE ON schedule_entries
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER tasks_set_updated_at
BEFORE UPDATE ON tasks
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 4
WHERE singleton = TRUE;
