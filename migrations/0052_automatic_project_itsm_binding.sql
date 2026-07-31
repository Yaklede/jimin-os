-- Project owners opt in to ITSM enrichment without knowing the upstream
-- Redmine project identifier. A successfully fetched, project-scoped issue
-- may propose a bounded candidate, but only the owner can confirm that
-- candidate as the enforced boundary. Existing identifiers remain confirmed.
ALTER TABLE project_itsm_connections
    ALTER COLUMN itsm_project_id DROP NOT NULL,
    ADD COLUMN candidate_itsm_project_id TEXT NULL CHECK (
        char_length(candidate_itsm_project_id) BETWEEN 1 AND 20
        AND candidate_itsm_project_id ~ '^[1-9][0-9]{0,19}$'
    ),
    ADD COLUMN candidate_itsm_project_name TEXT NULL CHECK (
        char_length(candidate_itsm_project_name) BETWEEN 1 AND 160
        AND candidate_itsm_project_name = btrim(candidate_itsm_project_name)
    ),
    ADD CONSTRAINT project_itsm_connections_candidate_pair_check CHECK (
        (candidate_itsm_project_id IS NULL) =
        (candidate_itsm_project_name IS NULL)
    ),
    ADD CONSTRAINT project_itsm_connections_binding_state_check CHECK (
        itsm_project_id IS NULL
        OR (
            candidate_itsm_project_id IS NULL
            AND candidate_itsm_project_name IS NULL
        )
    );

COMMENT ON COLUMN project_itsm_connections.itsm_project_id IS
    'Owner-confirmed upstream project identifier; NULL until a detected candidate is confirmed';

COMMENT ON COLUMN project_itsm_connections.candidate_itsm_project_id IS
    'Agent-detected upstream project candidate; never exposed through the public API';

COMMENT ON COLUMN project_itsm_connections.candidate_itsm_project_name IS
    'Bounded owner-visible label for the detected upstream project candidate';

UPDATE jimin_schema_metadata
SET schema_version = 52
WHERE singleton = TRUE;
