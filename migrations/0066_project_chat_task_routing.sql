-- Existing collection stays enabled. Already opted-in reminder sources retain replies.
ALTER TABLE project_google_chat_sources ADD COLUMN purpose TEXT NOT NULL DEFAULT 'inflow'
    CHECK (purpose IN ('inflow','delivery','both'));
UPDATE project_google_chat_sources s SET purpose='both'
WHERE EXISTS (SELECT 1 FROM scheduled_work w WHERE w.user_id=s.user_id
    AND w.project_id=s.project_id AND w.definition->>'replyCompletionSourceId'=s.id::text);

CREATE OR REPLACE FUNCTION jimin_google_chat_source_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at=NOW();
    IF ROW(NEW.project_id,NEW.account_id,NEW.space_name,NEW.display_name,
        NEW.enabled,NEW.acknowledge_with_reaction,NEW.last_error_code,NEW.purpose)
        IS DISTINCT FROM ROW(OLD.project_id,OLD.account_id,OLD.space_name,OLD.display_name,
        OLD.enabled,OLD.acknowledge_with_reaction,OLD.last_error_code,OLD.purpose) THEN
        NEW.version=OLD.version+1;
    ELSE
        NEW.version=OLD.version;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE SEQUENCE project_task_chat_route_versions;
CREATE TABLE project_task_chat_routes (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources(id) ON DELETE CASCADE,
    directory_webhook_id UUID NOT NULL REFERENCES project_webhooks(id) ON DELETE CASCADE,
    version BIGINT NOT NULL DEFAULT nextval('project_task_chat_route_versions') CHECK (version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
ALTER SEQUENCE project_task_chat_route_versions OWNED BY project_task_chat_routes.version;
CREATE INDEX project_task_chat_routes_owner_idx ON project_task_chat_routes(user_id,project_id);

CREATE TABLE task_chat_deliveries (
    delivery_id UUID PRIMARY KEY REFERENCES webhook_deliveries(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_id UUID NOT NULL REFERENCES project_google_chat_sources(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    task_version BIGINT NOT NULL CHECK (task_version > 0),
    kind TEXT NOT NULL CHECK (kind IN ('assignment','completion')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(task_id,task_version,kind)
);
CREATE INDEX task_chat_deliveries_source_idx ON task_chat_deliveries(user_id,source_id,delivery_id);
UPDATE jimin_schema_metadata SET schema_version=66 WHERE singleton=TRUE;
