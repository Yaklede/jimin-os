-- A covered empty month is distinct from a month we could not fetch.
CREATE TABLE holiday_calendar_months (
    year INTEGER NOT NULL CHECK (year BETWEEN 2000 AND 2200),
    month INTEGER NOT NULL CHECK (month BETWEEN 1 AND 12),
    fetched_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (year, month)
);
CREATE TABLE holiday_calendar_dates (
    date DATE PRIMARY KEY,
    name TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 200)
);
UPDATE jimin_schema_metadata SET schema_version = 59 WHERE singleton = TRUE;
