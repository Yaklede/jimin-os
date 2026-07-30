-- Preserve the bounded, read-only source snapshots used by inflow analysis.
-- Provider messages remain immutable evidence; these documents only enrich
-- the structured task candidate with the trusted issue tracker's source text.
ALTER TABLE project_inflow_analyses
    ADD COLUMN reference_documents JSONB NOT NULL DEFAULT '[]'::JSONB,
    ADD CONSTRAINT project_inflow_analyses_reference_documents_check CHECK (
        jsonb_typeof(reference_documents) = 'array'
        AND jsonb_array_length(reference_documents) <= 4
    );

UPDATE jimin_schema_metadata
SET schema_version = 49
WHERE singleton = TRUE;
