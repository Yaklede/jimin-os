CREATE TABLE jimin_schema_metadata (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE,
    schema_version BIGINT NOT NULL CHECK (schema_version >= 1),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT jimin_schema_metadata_singleton CHECK (singleton = TRUE)
);

INSERT INTO jimin_schema_metadata (singleton, schema_version)
VALUES (TRUE, 1);
