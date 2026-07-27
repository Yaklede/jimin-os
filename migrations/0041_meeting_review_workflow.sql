-- Meeting review metadata and assignee ownership turn extracted meeting notes
-- into editable, approval-gated planning actions.
ALTER TABLE meetings
    ADD COLUMN purpose TEXT NULL CHECK (
        purpose IS NULL OR char_length(btrim(purpose)) BETWEEN 1 AND 2000
    ),
    ADD COLUMN participants TEXT[] NOT NULL DEFAULT '{}';

ALTER TABLE meetings
    ADD CONSTRAINT meetings_participants_count_check
        CHECK (cardinality(participants) <= 100),
    ADD CONSTRAINT meetings_participants_value_check
        CHECK (array_position(participants, NULL) IS NULL);

ALTER TABLE meeting_action_items
    ADD COLUMN assignee_name TEXT NULL CHECK (
        assignee_name IS NULL
        OR char_length(btrim(assignee_name)) BETWEEN 1 AND 120
    );

UPDATE jimin_schema_metadata
SET schema_version = 41
WHERE singleton = TRUE;
