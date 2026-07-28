-- Meeting recordings keep resumable audio chunks and live notes separate from
-- the finalized, speaker-attributed transcript. Recording and transcription
-- are explicit states so the existing meeting analysis job never runs against
-- an incomplete transcript.
ALTER TABLE meetings
    DROP CONSTRAINT meetings_status_check,
    DROP CONSTRAINT meetings_transcript_check;

ALTER TABLE meetings
    ADD CONSTRAINT meetings_status_check CHECK (
        status IN (
            'recording',
            'transcribing',
            'queued',
            'analyzing',
            'review_ready',
            'applied',
            'failed'
        )
    ),
    ADD CONSTRAINT meetings_transcript_check CHECK (
        char_length(transcript) <= 120000
        AND (
            status IN ('recording', 'transcribing')
            OR char_length(btrim(transcript)) >= 1
        )
    );

CREATE TABLE meeting_recordings (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL UNIQUE REFERENCES meetings (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'recording' CHECK (
        state IN (
            'recording',
            'queued',
            'claimed',
            'running',
            'completed',
            'failed',
            'cancelled'
        )
    ),
    mime_type TEXT NULL CHECK (
        mime_type IS NULL OR char_length(btrim(mime_type)) BETWEEN 1 AND 120
    ),
    notes TEXT NOT NULL DEFAULT '' CHECK (char_length(notes) <= 40000),
    duration_milliseconds BIGINT NULL CHECK (
        duration_milliseconds IS NULL
        OR duration_milliseconds BETWEEN 1 AND 43200000
    ),
    chunk_count INTEGER NOT NULL DEFAULT 0 CHECK (chunk_count BETWEEN 0 AND 10000),
    byte_length BIGINT NOT NULL DEFAULT 0 CHECK (
        byte_length BETWEEN 0 AND 536870912
    ),
    claim_owner TEXT NULL CHECK (
        claim_owner IS NULL OR char_length(claim_owner) BETWEEN 1 AND 200
    ),
    claim_expires_at TIMESTAMPTZ NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 8),
    error_code TEXT NULL CHECK (
        error_code IS NULL OR char_length(error_code) BETWEEN 1 AND 120
    ),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finalized_at TIMESTAMPTZ NULL,
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
    ),
    CHECK (
        (state = 'recording' AND finalized_at IS NULL)
        OR (state <> 'recording' AND finalized_at IS NOT NULL)
    ),
    UNIQUE (id, user_id)
);

CREATE INDEX meeting_recordings_user_active_idx
    ON meeting_recordings (user_id, updated_at DESC, id DESC)
    WHERE state IN ('recording', 'queued', 'claimed', 'running', 'failed');

CREATE INDEX meeting_recordings_claimable_idx
    ON meeting_recordings (finalized_at, id)
    WHERE state = 'queued';

CREATE INDEX meeting_recordings_expired_lease_idx
    ON meeting_recordings (claim_expires_at, id)
    WHERE state IN ('claimed', 'running');

CREATE TABLE meeting_recording_chunks (
    recording_id UUID NOT NULL REFERENCES meeting_recordings (id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence BETWEEN 0 AND 9999),
    mime_type TEXT NOT NULL CHECK (char_length(btrim(mime_type)) BETWEEN 1 AND 120),
    audio_data BYTEA NOT NULL CHECK (
        octet_length(audio_data) BETWEEN 1 AND 8388608
    ),
    byte_length INTEGER NOT NULL CHECK (
        byte_length = octet_length(audio_data)
        AND byte_length BETWEEN 1 AND 8388608
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (recording_id, sequence)
);

CREATE TABLE meeting_speakers (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    speaker_key TEXT NOT NULL CHECK (
        char_length(btrim(speaker_key)) BETWEEN 1 AND 80
    ),
    display_name TEXT NULL CHECK (
        display_name IS NULL
        OR char_length(btrim(display_name)) BETWEEN 1 AND 120
    ),
    ordinal SMALLINT NOT NULL CHECK (ordinal BETWEEN 0 AND 99),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (meeting_id, speaker_key),
    UNIQUE (meeting_id, ordinal)
);

CREATE INDEX meeting_speakers_meeting_idx
    ON meeting_speakers (meeting_id, ordinal, id);

CREATE TABLE meeting_transcript_segments (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    speaker_id UUID NOT NULL REFERENCES meeting_speakers (id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 100000),
    starts_at_milliseconds BIGINT NOT NULL CHECK (
        starts_at_milliseconds BETWEEN 0 AND 43200000
    ),
    ends_at_milliseconds BIGINT NOT NULL CHECK (
        ends_at_milliseconds > starts_at_milliseconds
        AND ends_at_milliseconds <= 43200000
    ),
    text TEXT NOT NULL CHECK (char_length(btrim(text)) BETWEEN 1 AND 8000),
    confidence SMALLINT NULL CHECK (
        confidence IS NULL OR confidence BETWEEN 0 AND 100
    ),
    is_final BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (meeting_id, ordinal)
);

CREATE INDEX meeting_transcript_segments_timeline_idx
    ON meeting_transcript_segments (
        meeting_id,
        starts_at_milliseconds,
        ordinal,
        id
    );

CREATE TRIGGER meeting_recordings_set_updated_at
BEFORE UPDATE ON meeting_recordings
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

CREATE TRIGGER meeting_speakers_set_updated_at
BEFORE UPDATE ON meeting_speakers
FOR EACH ROW EXECUTE FUNCTION jimin_set_updated_at();

UPDATE jimin_schema_metadata
SET schema_version = 43
WHERE singleton = TRUE;
