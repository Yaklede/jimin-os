//! Owner-scoped meeting transcripts, AI analysis, and approval-gated actions.

use std::time::Duration;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

const MAX_TITLE_CHARS: usize = 200;
const MAX_PURPOSE_CHARS: usize = 2_000;
const MAX_PARTICIPANTS: usize = 100;
const MAX_PARTICIPANT_CHARS: usize = 120;
const MAX_TRANSCRIPT_CHARS: usize = 120_000;
const MAX_SUMMARY_CHARS: usize = 20_000;
const MAX_DETAIL_CHARS: usize = 4_000;
const MAX_EXCERPT_CHARS: usize = 2_000;
const MAX_ANALYSIS_ITEMS: usize = 32;
const MAX_TIME_ZONE_CHARS: usize = 100;
const MAX_RECORDING_NOTES_CHARS: usize = 40_000;
const MAX_RECORDING_CHUNK_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECORDING_BYTES: i64 = 512 * 1024 * 1024;
const MAX_RECORDING_CHUNKS: i32 = 10_000;
const MAX_TRANSCRIPT_SEGMENTS: usize = 100_000;
const MAX_SEGMENT_CHARS: usize = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingStatus {
    Recording,
    Transcribing,
    Queued,
    Analyzing,
    ReviewReady,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingRecordingState {
    Recording,
    Queued,
    Claimed,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingActionKind {
    Task,
    Schedule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingActionStatus {
    Suggested,
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
    pub id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub project_title: Option<String>,
    pub title: String,
    pub purpose: Option<String>,
    pub participants: Vec<String>,
    pub transcript: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration_seconds: Option<i32>,
    pub status: MeetingStatus,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub risks: Vec<String>,
    pub follow_up: Option<String>,
    pub analyzed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingDecision {
    pub id: Uuid,
    pub content: String,
    pub rationale: Option<String>,
    pub source_excerpt: String,
    pub source_timestamp_seconds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingActionItem {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub kind: MeetingActionKind,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub assignee_name: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub starts_at: Option<OffsetDateTime>,
    pub ends_at: Option<OffsetDateTime>,
    pub time_zone: Option<String>,
    pub source_excerpt: String,
    pub confidence: i16,
    pub status: MeetingActionStatus,
    pub target_entity_id: Uuid,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingDetail {
    pub meeting: Meeting,
    pub recording: Option<MeetingRecording>,
    pub speakers: Vec<MeetingSpeaker>,
    pub transcript_segments: Vec<MeetingTranscriptSegment>,
    pub decisions: Vec<MeetingDecision>,
    pub action_items: Vec<MeetingActionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingRecording {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub state: MeetingRecordingState,
    pub mime_type: Option<String>,
    pub notes: String,
    pub duration_milliseconds: Option<i64>,
    pub chunk_count: i32,
    pub byte_length: i64,
    pub error_code: Option<String>,
    pub started_at: OffsetDateTime,
    pub finalized_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingSpeaker {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub speaker_key: String,
    pub display_name: Option<String>,
    pub ordinal: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingTranscriptSegment {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub speaker_id: Uuid,
    pub speaker_key: String,
    pub speaker_name: Option<String>,
    pub ordinal: i32,
    pub starts_at_milliseconds: i64,
    pub ends_at_milliseconds: i64,
    pub text: String,
    pub confidence: Option<i16>,
    pub is_final: bool,
}

pub struct NewRecordedMeeting {
    pub meeting_id: Uuid,
    pub recording_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub purpose: Option<String>,
    pub participants: Vec<String>,
    pub started_at: OffsetDateTime,
}

pub struct RecordingChunk {
    pub recording_id: Uuid,
    pub user_id: Uuid,
    pub sequence: i32,
    pub mime_type: String,
    pub audio_data: Vec<u8>,
}

pub struct RecordingNoteUpdate {
    pub recording_id: Uuid,
    pub user_id: Uuid,
    pub notes: String,
}

pub struct RecordingFinalize {
    pub recording_id: Uuid,
    pub user_id: Uuid,
    pub mime_type: String,
    pub duration_milliseconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedMeetingTranscription {
    pub recording_id: Uuid,
    pub meeting_id: Uuid,
    pub user_id: Uuid,
    pub mime_type: String,
    pub audio_data: Vec<u8>,
    pub participants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NewMeetingSpeaker {
    pub id: Uuid,
    pub speaker_key: String,
    pub display_name: Option<String>,
    pub ordinal: i16,
}

#[derive(Debug, Clone)]
pub struct NewMeetingTranscriptSegment {
    pub id: Uuid,
    pub speaker_key: String,
    pub ordinal: i32,
    pub starts_at_milliseconds: i64,
    pub ends_at_milliseconds: i64,
    pub text: String,
    pub confidence: Option<i16>,
}

pub struct MeetingTranscriptionResult {
    pub transcript: String,
    pub speakers: Vec<NewMeetingSpeaker>,
    pub segments: Vec<NewMeetingTranscriptSegment>,
}

pub struct NewMeeting {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub purpose: Option<String>,
    pub participants: Vec<String>,
    pub transcript: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration_seconds: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewMeetingDecision {
    pub id: Uuid,
    pub content: String,
    pub rationale: Option<String>,
    pub source_excerpt: String,
    pub source_timestamp_seconds: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewMeetingActionItem {
    pub id: Uuid,
    pub target_entity_id: Uuid,
    pub kind: MeetingActionKind,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub assignee_name: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub starts_at: Option<OffsetDateTime>,
    pub ends_at: Option<OffsetDateTime>,
    pub time_zone: Option<String>,
    pub source_excerpt: String,
    pub confidence: i16,
}

#[derive(Debug, Clone)]
pub struct MeetingActionItemUpdate {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub user_id: Uuid,
    pub expected_version: i64,
    pub kind: MeetingActionKind,
    pub title: String,
    pub notes: Option<String>,
    pub assignee_name: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub starts_at: Option<OffsetDateTime>,
    pub ends_at: Option<OffsetDateTime>,
    pub time_zone: Option<String>,
}

pub struct MeetingAnalysisResult {
    pub summary: String,
    pub topics: Vec<String>,
    pub risks: Vec<String>,
    pub follow_up: Option<String>,
    pub decisions: Vec<NewMeetingDecision>,
    pub action_items: Vec<NewMeetingActionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedMeetingAnalysis {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub purpose: Option<String>,
    pub participants: Vec<String>,
    pub transcript: String,
    pub project_id: Option<Uuid>,
    pub project_title: Option<String>,
    pub started_at: Option<OffsetDateTime>,
    pub processing_model_id: Option<String>,
    pub processing_reasoning_effort: Option<String>,
}

#[derive(sqlx::FromRow)]
struct MeetingRow {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_title: Option<String>,
    title: String,
    purpose: Option<String>,
    participants: Vec<String>,
    transcript: String,
    started_at: Option<OffsetDateTime>,
    duration_seconds: Option<i32>,
    status: String,
    summary: Option<String>,
    topics: Vec<String>,
    risks: Vec<String>,
    follow_up: Option<String>,
    analyzed_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct MeetingDecisionRow {
    id: Uuid,
    content: String,
    rationale: Option<String>,
    source_excerpt: String,
    source_timestamp_seconds: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct MeetingActionItemRow {
    id: Uuid,
    meeting_id: Uuid,
    kind: String,
    project_id: Option<Uuid>,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    priority: i16,
    due_at: Option<OffsetDateTime>,
    starts_at: Option<OffsetDateTime>,
    ends_at: Option<OffsetDateTime>,
    time_zone: Option<String>,
    source_excerpt: String,
    confidence: i16,
    status: String,
    target_entity_id: Uuid,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct MeetingRecordingRow {
    id: Uuid,
    meeting_id: Uuid,
    state: String,
    mime_type: Option<String>,
    notes: String,
    duration_milliseconds: Option<i64>,
    chunk_count: i32,
    byte_length: i64,
    error_code: Option<String>,
    started_at: OffsetDateTime,
    finalized_at: Option<OffsetDateTime>,
    finished_at: Option<OffsetDateTime>,
    updated_at: OffsetDateTime,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct MeetingSpeakerRow {
    id: Uuid,
    meeting_id: Uuid,
    speaker_key: String,
    display_name: Option<String>,
    ordinal: i16,
}

#[derive(sqlx::FromRow)]
struct MeetingTranscriptSegmentRow {
    id: Uuid,
    meeting_id: Uuid,
    speaker_id: Uuid,
    speaker_key: String,
    speaker_name: Option<String>,
    ordinal: i32,
    starts_at_milliseconds: i64,
    ends_at_milliseconds: i64,
    text: String,
    confidence: Option<i16>,
    is_final: bool,
}

#[derive(sqlx::FromRow)]
struct ClaimedMeetingTranscriptionRow {
    recording_id: Uuid,
    meeting_id: Uuid,
    user_id: Uuid,
    mime_type: String,
    audio_data: Vec<u8>,
    participants: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct ClaimedMeetingAnalysisRow {
    id: Uuid,
    meeting_id: Uuid,
    user_id: Uuid,
    title: String,
    purpose: Option<String>,
    participants: Vec<String>,
    transcript: String,
    project_id: Option<Uuid>,
    project_title: Option<String>,
    started_at: Option<OffsetDateTime>,
    processing_model_id: Option<String>,
    processing_reasoning_effort: Option<String>,
}

impl NewMeeting {
    /// Validates bounded transcript metadata before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        let duration_valid = self
            .duration_seconds
            .is_none_or(|seconds| (1..=43_200).contains(&seconds));
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_optional_id(self.workspace_id)
            || !valid_optional_id(self.project_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS)
            || !valid_optional_body_text(self.purpose.as_deref(), MAX_PURPOSE_CHARS)
            || self.participants.len() > MAX_PARTICIPANTS
            || !self
                .participants
                .iter()
                .all(|participant| valid_text(participant, MAX_PARTICIPANT_CHARS))
            || !valid_body_text(&self.transcript, MAX_TRANSCRIPT_CHARS)
            || !duration_valid
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl NewRecordedMeeting {
    /// Validates metadata before a resumable meeting recording is opened.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.meeting_id)
            || !is_v7(self.recording_id)
            || !is_v7(self.user_id)
            || !valid_optional_id(self.workspace_id)
            || !valid_optional_id(self.project_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS)
            || !valid_optional_body_text(self.purpose.as_deref(), MAX_PURPOSE_CHARS)
            || self.participants.len() > MAX_PARTICIPANTS
            || !self
                .participants
                .iter()
                .all(|participant| valid_text(participant, MAX_PARTICIPANT_CHARS))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl RecordingChunk {
    /// Validates one bounded audio chunk before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.recording_id)
            || !is_v7(self.user_id)
            || !(0..MAX_RECORDING_CHUNKS).contains(&self.sequence)
            || !valid_text(&self.mime_type, 120)
            || self.audio_data.is_empty()
            || self.audio_data.len() > MAX_RECORDING_CHUNK_BYTES
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl RecordingNoteUpdate {
    /// Validates an autosaved note update.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.recording_id)
            || !is_v7(self.user_id)
            || self.notes.chars().count() > MAX_RECORDING_NOTES_CHARS
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl RecordingFinalize {
    /// Validates the final recording metadata before transcription is queued.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.recording_id)
            || !is_v7(self.user_id)
            || !valid_text(&self.mime_type, 120)
            || !(1..=43_200_000).contains(&self.duration_milliseconds)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl MeetingTranscriptionResult {
    /// Validates speaker-attributed transcription output before it replaces a draft.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for incomplete or
    /// internally inconsistent output.
    pub fn validate(&self) -> Result<(), StorageError> {
        let unique_speakers = self
            .speakers
            .iter()
            .map(|speaker| speaker.speaker_key.as_str())
            .collect::<std::collections::HashSet<_>>();
        let unique_ordinals = self
            .segments
            .iter()
            .map(|segment| segment.ordinal)
            .collect::<std::collections::HashSet<_>>();
        let valid_speakers = !self.speakers.is_empty()
            && self.speakers.len() <= 100
            && unique_speakers.len() == self.speakers.len()
            && self.speakers.iter().all(|speaker| {
                is_v7(speaker.id)
                    && valid_text(&speaker.speaker_key, 80)
                    && valid_optional_body_text(
                        speaker.display_name.as_deref(),
                        MAX_PARTICIPANT_CHARS,
                    )
                    && (0..=99).contains(&speaker.ordinal)
            });
        let valid_segments = !self.segments.is_empty()
            && self.segments.len() <= MAX_TRANSCRIPT_SEGMENTS
            && unique_ordinals.len() == self.segments.len()
            && self.segments.iter().all(|segment| {
                is_v7(segment.id)
                    && unique_speakers.contains(segment.speaker_key.as_str())
                    && (0..=100_000).contains(&segment.ordinal)
                    && (0..43_200_000).contains(&segment.starts_at_milliseconds)
                    && segment.ends_at_milliseconds > segment.starts_at_milliseconds
                    && segment.ends_at_milliseconds <= 43_200_000
                    && valid_body_text(&segment.text, MAX_SEGMENT_CHARS)
                    && segment
                        .confidence
                        .is_none_or(|value| (0..=100).contains(&value))
            });
        if valid_body_text(&self.transcript, MAX_TRANSCRIPT_CHARS)
            && valid_speakers
            && valid_segments
        {
            Ok(())
        } else {
            Err(StorageError::InvalidConfiguration)
        }
    }
}

impl MeetingAnalysisResult {
    /// Validates all model-derived content before it is persisted.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when model output is
    /// unbounded, incomplete, or internally inconsistent.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !valid_body_text(&self.summary, MAX_SUMMARY_CHARS)
            || self.topics.len() > MAX_ANALYSIS_ITEMS
            || self.risks.len() > MAX_ANALYSIS_ITEMS
            || self.decisions.len() > MAX_ANALYSIS_ITEMS
            || self.action_items.len() > MAX_ANALYSIS_ITEMS
            || !self
                .topics
                .iter()
                .chain(&self.risks)
                .all(|value| valid_body_text(value, MAX_DETAIL_CHARS))
            || !valid_optional_body_text(self.follow_up.as_deref(), MAX_DETAIL_CHARS)
            || !self
                .decisions
                .iter()
                .all(|decision| decision.validate().is_ok())
            || !self.action_items.iter().all(|item| item.validate().is_ok())
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl NewMeetingDecision {
    /// Validates one model-derived decision before it is included in a result.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the decision is
    /// incomplete or exceeds the persisted bounds.
    pub fn validate(&self) -> Result<(), StorageError> {
        (is_v7(self.id)
            && valid_body_text(&self.content, MAX_EXCERPT_CHARS)
            && valid_optional_body_text(self.rationale.as_deref(), MAX_EXCERPT_CHARS)
            && valid_body_text(&self.source_excerpt, MAX_EXCERPT_CHARS)
            && self.source_timestamp_seconds.is_none_or(|value| value >= 0))
        .then_some(())
        .ok_or(StorageError::InvalidConfiguration)
    }
}

impl NewMeetingActionItem {
    /// Validates one model-derived action candidate before it is included in a result.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the candidate is
    /// incomplete, out of bounds, or has an inconsistent schedule window.
    pub fn validate(&self) -> Result<(), StorageError> {
        let schedule_fields_valid = match self.kind {
            MeetingActionKind::Task => {
                self.starts_at.is_none() && self.ends_at.is_none() && self.time_zone.is_none()
            }
            MeetingActionKind::Schedule => {
                self.starts_at
                    .zip(self.ends_at)
                    .is_some_and(|(start, end)| end > start)
                    && self
                        .time_zone
                        .as_deref()
                        .is_some_and(|value| valid_text(value, 100))
            }
        };
        (is_v7(self.id)
            && is_v7(self.target_entity_id)
            && valid_optional_id(self.project_id)
            && valid_text(&self.title, MAX_TITLE_CHARS)
            && valid_optional_body_text(self.notes.as_deref(), MAX_DETAIL_CHARS)
            && valid_optional_body_text(self.assignee_name.as_deref(), MAX_PARTICIPANT_CHARS)
            && (0..=3).contains(&self.priority)
            && valid_body_text(&self.source_excerpt, MAX_EXCERPT_CHARS)
            && (0..=100).contains(&self.confidence)
            && schedule_fields_valid)
            .then_some(())
            .ok_or(StorageError::InvalidConfiguration)
    }
}

impl MeetingActionItemUpdate {
    /// Validates an owner-reviewed action before replacing the AI suggestion.
    ///
    /// # Errors
    ///
    /// Returns an invalid configuration error for incomplete or inconsistent data.
    pub fn validate(&self) -> Result<(), StorageError> {
        let common_valid = is_v7(self.id)
            && is_v7(self.meeting_id)
            && is_v7(self.user_id)
            && self.expected_version > 0
            && valid_text(&self.title, MAX_TITLE_CHARS)
            && valid_optional_body_text(self.notes.as_deref(), MAX_DETAIL_CHARS)
            && valid_optional_body_text(self.assignee_name.as_deref(), MAX_PARTICIPANT_CHARS)
            && (0..=3).contains(&self.priority);
        let schedule_valid = match self.kind {
            MeetingActionKind::Task => {
                self.starts_at.is_none() && self.ends_at.is_none() && self.time_zone.is_none()
            }
            MeetingActionKind::Schedule => {
                matches!((self.starts_at, self.ends_at), (Some(start), Some(end)) if end > start)
                    && self.due_at.is_none()
                    && self
                        .time_zone
                        .as_deref()
                        .is_some_and(|value| valid_text(value, MAX_TIME_ZONE_CHARS))
            }
        };
        if common_valid && schedule_valid {
            Ok(())
        } else {
            Err(StorageError::InvalidConfiguration)
        }
    }
}

impl TryFrom<MeetingRow> for Meeting {
    type Error = StorageError;

    fn try_from(row: MeetingRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            project_id: row.project_id,
            project_title: row.project_title,
            title: row.title,
            purpose: row.purpose,
            participants: row.participants,
            transcript: row.transcript,
            started_at: row.started_at,
            duration_seconds: row.duration_seconds,
            status: parse_meeting_status(&row.status)?,
            summary: row.summary,
            topics: row.topics,
            risks: row.risks,
            follow_up: row.follow_up,
            analyzed_at: row.analyzed_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}

impl TryFrom<MeetingDecisionRow> for MeetingDecision {
    type Error = StorageError;

    fn try_from(row: MeetingDecisionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            content: row.content,
            rationale: row.rationale,
            source_excerpt: row.source_excerpt,
            source_timestamp_seconds: row.source_timestamp_seconds,
        })
    }
}

impl TryFrom<MeetingActionItemRow> for MeetingActionItem {
    type Error = StorageError;

    fn try_from(row: MeetingActionItemRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            meeting_id: row.meeting_id,
            kind: parse_action_kind(&row.kind)?,
            project_id: row.project_id,
            title: row.title,
            notes: row.notes,
            assignee_name: row.assignee_name,
            priority: row.priority,
            due_at: row.due_at,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            time_zone: row.time_zone,
            source_excerpt: row.source_excerpt,
            confidence: row.confidence,
            status: parse_action_status(&row.status)?,
            target_entity_id: row.target_entity_id,
            version: row.version,
        })
    }
}

impl TryFrom<MeetingRecordingRow> for MeetingRecording {
    type Error = StorageError;

    fn try_from(row: MeetingRecordingRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            meeting_id: row.meeting_id,
            state: parse_recording_state(&row.state)?,
            mime_type: row.mime_type,
            notes: row.notes,
            duration_milliseconds: row.duration_milliseconds,
            chunk_count: row.chunk_count,
            byte_length: row.byte_length,
            error_code: row.error_code,
            started_at: row.started_at,
            finalized_at: row.finalized_at,
            finished_at: row.finished_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}

impl From<MeetingSpeakerRow> for MeetingSpeaker {
    fn from(row: MeetingSpeakerRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            speaker_key: row.speaker_key,
            display_name: row.display_name,
            ordinal: row.ordinal,
        }
    }
}

impl From<MeetingTranscriptSegmentRow> for MeetingTranscriptSegment {
    fn from(row: MeetingTranscriptSegmentRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            speaker_id: row.speaker_id,
            speaker_key: row.speaker_key,
            speaker_name: row.speaker_name,
            ordinal: row.ordinal,
            starts_at_milliseconds: row.starts_at_milliseconds,
            ends_at_milliseconds: row.ends_at_milliseconds,
            text: row.text,
            confidence: row.confidence,
            is_final: row.is_final,
        }
    }
}

impl From<ClaimedMeetingTranscriptionRow> for ClaimedMeetingTranscription {
    fn from(row: ClaimedMeetingTranscriptionRow) -> Self {
        Self {
            recording_id: row.recording_id,
            meeting_id: row.meeting_id,
            user_id: row.user_id,
            mime_type: row.mime_type,
            audio_data: row.audio_data,
            participants: row.participants,
        }
    }
}

impl From<ClaimedMeetingAnalysisRow> for ClaimedMeetingAnalysis {
    fn from(row: ClaimedMeetingAnalysisRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            user_id: row.user_id,
            title: row.title,
            purpose: row.purpose,
            participants: row.participants,
            transcript: row.transcript,
            project_id: row.project_id,
            project_title: row.project_title,
            started_at: row.started_at,
            processing_model_id: row.processing_model_id,
            processing_reasoning_effort: row.processing_reasoning_effort,
        }
    }
}

impl Database {
    /// Creates an owner-scoped meeting and atomically queues its AI analysis.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, or persistence error.
    pub async fn create_meeting(&self, meeting: &NewMeeting) -> Result<Meeting, StorageError> {
        meeting.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if !meeting_scope_is_owned(
            &mut transaction,
            meeting.user_id,
            meeting.workspace_id,
            meeting.project_id,
        )
        .await?
        {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        }
        let row = sqlx::query_as::<_, MeetingRow>(
            "INSERT INTO meetings (
                id, user_id, workspace_id, project_id, title, purpose,
                participants, transcript, started_at, duration_seconds
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id, workspace_id, project_id,
                NULL::text AS project_title, title, purpose, participants,
                transcript, started_at, duration_seconds, status, summary,
                topics, risks, follow_up, analyzed_at, created_at, updated_at,
                version",
        )
        .bind(meeting.id)
        .bind(meeting.user_id)
        .bind(meeting.workspace_id)
        .bind(meeting.project_id)
        .bind(meeting.title.trim())
        .bind(trimmed_optional(meeting.purpose.as_deref()))
        .bind(trimmed_strings(&meeting.participants))
        .bind(meeting.transcript.trim())
        .bind(meeting.started_at)
        .bind(meeting.duration_seconds)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let job_id = Uuid::now_v7();
        let job_version = sqlx::query_scalar::<_, i64>(
            "INSERT INTO meeting_analysis_jobs (id, meeting_id, user_id)
             VALUES ($1, $2, $3)
             RETURNING version",
        )
        .bind(job_id)
        .bind(meeting.id)
        .bind(meeting.user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            meeting.user_id,
            "meeting",
            meeting.id,
            row.version,
        )
        .await?;
        append_change(
            &mut transaction,
            meeting.user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Meeting::try_from(row)
    }

    /// Opens an owner-scoped recording draft without queuing analysis yet.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, or persistence error.
    pub async fn create_recorded_meeting(
        &self,
        input: &NewRecordedMeeting,
    ) -> Result<(Meeting, MeetingRecording), StorageError> {
        input.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if !meeting_scope_is_owned(
            &mut transaction,
            input.user_id,
            input.workspace_id,
            input.project_id,
        )
        .await?
        {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        }
        let meeting_row = sqlx::query_as::<_, MeetingRow>(
            "INSERT INTO meetings (
                id, user_id, workspace_id, project_id, title, purpose,
                participants, transcript, started_at, status
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, '', $8, 'recording')
             RETURNING id, workspace_id, project_id,
                NULL::text AS project_title, title, purpose, participants,
                transcript, started_at, duration_seconds, status, summary,
                topics, risks, follow_up, analyzed_at, created_at, updated_at,
                version",
        )
        .bind(input.meeting_id)
        .bind(input.user_id)
        .bind(input.workspace_id)
        .bind(input.project_id)
        .bind(input.title.trim())
        .bind(trimmed_optional(input.purpose.as_deref()))
        .bind(trimmed_strings(&input.participants))
        .bind(input.started_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let recording_row = sqlx::query_as::<_, MeetingRecordingRow>(
            "INSERT INTO meeting_recordings (
                id, meeting_id, user_id, started_at
             ) VALUES ($1, $2, $3, $4)
             RETURNING id, meeting_id, state, mime_type, notes,
                duration_milliseconds, chunk_count, byte_length, error_code,
                started_at, finalized_at, finished_at, updated_at, version",
        )
        .bind(input.recording_id)
        .bind(input.meeting_id)
        .bind(input.user_id)
        .bind(input.started_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            input.user_id,
            "meeting",
            input.meeting_id,
            meeting_row.version,
        )
        .await?;
        append_change(
            &mut transaction,
            input.user_id,
            "meeting_recording",
            input.recording_id,
            recording_row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok((
            Meeting::try_from(meeting_row)?,
            MeetingRecording::try_from(recording_row)?,
        ))
    }

    /// Appends one idempotent audio chunk while a recording is active.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, state, or persistence error.
    pub async fn append_meeting_recording_chunk(
        &self,
        chunk: &RecordingChunk,
    ) -> Result<MeetingRecording, StorageError> {
        chunk.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let recording = sqlx::query_as::<_, MeetingRecordingRow>(
            "SELECT id, meeting_id, state, mime_type, notes,
                duration_milliseconds, chunk_count, byte_length, error_code,
                started_at, finalized_at, finished_at, updated_at, version
             FROM meeting_recordings
             WHERE id = $1 AND user_id = $2
             FOR UPDATE",
        )
        .bind(chunk.recording_id)
        .bind(chunk.user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::IdentityConflict)?;
        if parse_recording_state(&recording.state)? != MeetingRecordingState::Recording {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        }
        let existing = sqlx::query_as::<_, (String, Vec<u8>)>(
            "SELECT mime_type, audio_data
             FROM meeting_recording_chunks
             WHERE recording_id = $1 AND sequence = $2",
        )
        .bind(chunk.recording_id)
        .bind(chunk.sequence)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some((mime_type, audio_data)) = existing {
            if mime_type != chunk.mime_type || audio_data != chunk.audio_data {
                transaction.rollback().await.map_err(classify)?;
                return Err(StorageError::IdentityConflict);
            }
            transaction.commit().await.map_err(classify)?;
            return MeetingRecording::try_from(recording);
        }
        let next_byte_length = recording
            .byte_length
            .checked_add(
                i64::try_from(chunk.audio_data.len())
                    .map_err(|_| StorageError::InvalidConfiguration)?,
            )
            .filter(|value| *value <= MAX_RECORDING_BYTES)
            .ok_or(StorageError::InvalidConfiguration)?;
        if recording.chunk_count >= MAX_RECORDING_CHUNKS {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "INSERT INTO meeting_recording_chunks (
                recording_id, sequence, mime_type, audio_data, byte_length
             ) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(chunk.recording_id)
        .bind(chunk.sequence)
        .bind(chunk.mime_type.trim())
        .bind(&chunk.audio_data)
        .bind(
            i32::try_from(chunk.audio_data.len())
                .map_err(|_| StorageError::InvalidConfiguration)?,
        )
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        let updated = sqlx::query_as::<_, MeetingRecordingRow>(
            "UPDATE meeting_recordings
             SET chunk_count = chunk_count + 1, byte_length = $3,
                 mime_type = COALESCE(mime_type, $4)
             WHERE id = $1 AND user_id = $2 AND state = 'recording'
             RETURNING id, meeting_id, state, mime_type, notes,
                duration_milliseconds, chunk_count, byte_length, error_code,
                started_at, finalized_at, finished_at, updated_at, version",
        )
        .bind(chunk.recording_id)
        .bind(chunk.user_id)
        .bind(next_byte_length)
        .bind(chunk.mime_type.trim())
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            chunk.user_id,
            "meeting_recording",
            chunk.recording_id,
            updated.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        MeetingRecording::try_from(updated)
    }

    /// Autosaves notes without stopping an active recording.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, state, or persistence error.
    pub async fn update_meeting_recording_notes(
        &self,
        input: &RecordingNoteUpdate,
    ) -> Result<MeetingRecording, StorageError> {
        input.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, MeetingRecordingRow>(
            "UPDATE meeting_recordings
             SET notes = $3
             WHERE id = $1 AND user_id = $2 AND state = 'recording'
             RETURNING id, meeting_id, state, mime_type, notes,
                duration_milliseconds, chunk_count, byte_length, error_code,
                started_at, finalized_at, finished_at, updated_at, version",
        )
        .bind(input.recording_id)
        .bind(input.user_id)
        .bind(input.notes.trim_end())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::IdentityConflict)?;
        append_change(
            &mut transaction,
            input.user_id,
            "meeting_recording",
            input.recording_id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        MeetingRecording::try_from(row)
    }

    /// Finalizes a non-empty recording and queues speaker transcription.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, state, or persistence error.
    pub async fn finalize_meeting_recording(
        &self,
        input: &RecordingFinalize,
    ) -> Result<MeetingRecording, StorageError> {
        input.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, MeetingRecordingRow>(
            "UPDATE meeting_recordings
             SET state = 'queued', mime_type = $3,
                 duration_milliseconds = $4, finalized_at = NOW(),
                 error_code = NULL
             WHERE id = $1 AND user_id = $2 AND state = 'recording'
               AND chunk_count > 0 AND byte_length > 0
             RETURNING id, meeting_id, state, mime_type, notes,
                duration_milliseconds, chunk_count, byte_length, error_code,
                started_at, finalized_at, finished_at, updated_at, version",
        )
        .bind(input.recording_id)
        .bind(input.user_id)
        .bind(input.mime_type.trim())
        .bind(input.duration_milliseconds)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::IdentityConflict)?;
        let duration_seconds = i32::try_from((input.duration_milliseconds + 999) / 1_000)
            .map_err(|_| StorageError::InvalidConfiguration)?;
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings
             SET status = 'transcribing', duration_seconds = $3
             WHERE id = $1 AND user_id = $2 AND status = 'recording'
             RETURNING version",
        )
        .bind(row.meeting_id)
        .bind(input.user_id)
        .bind(duration_seconds)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            input.user_id,
            "meeting_recording",
            input.recording_id,
            row.version,
        )
        .await?;
        append_change(
            &mut transaction,
            input.user_id,
            "meeting",
            row.meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        MeetingRecording::try_from(row)
    }

    /// Cancels an owner-scoped recording draft and keeps no audio or notes.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, state, or persistence error.
    pub async fn cancel_meeting_recording(
        &self,
        user_id: Uuid,
        recording_id: Uuid,
    ) -> Result<(), StorageError> {
        if !is_v7(user_id) || !is_v7(recording_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE meeting_recordings
             SET state = 'cancelled', claim_owner = NULL,
                 claim_expires_at = NULL, finalized_at = NOW(),
                 finished_at = NOW(), notes = ''
             WHERE id = $1 AND user_id = $2 AND state = 'recording'
             RETURNING meeting_id, version",
        )
        .bind(recording_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::IdentityConflict)?;
        sqlx::query("DELETE FROM meeting_recording_chunks WHERE recording_id = $1")
            .bind(recording_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        sqlx::query("DELETE FROM meetings WHERE id = $1 AND user_id = $2 AND status = 'recording'")
            .bind(row.0)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        transaction.commit().await.map_err(classify)
    }

    /// Claims the oldest finalized recording for speaker-aware transcription.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn claim_next_meeting_transcription(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedMeetingTranscription>, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let row = sqlx::query_as::<_, ClaimedMeetingTranscriptionRow>(
            "WITH recovered AS (
                UPDATE meeting_recordings
                SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL
                WHERE state IN ('claimed', 'running') AND claim_expires_at < NOW()
             ), candidate AS (
                SELECT id FROM meeting_recordings
                WHERE state = 'queued'
                ORDER BY finalized_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             ), claimed AS (
                UPDATE meeting_recordings AS recording
                SET state = 'claimed', claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE recording.id = candidate.id
                RETURNING recording.id, recording.meeting_id,
                    recording.user_id, recording.mime_type
             )
             SELECT claimed.id AS recording_id, claimed.meeting_id,
                claimed.user_id, claimed.mime_type,
                COALESCE(chunks.audio_data, ''::bytea) AS audio_data,
                meeting.participants
             FROM claimed
             INNER JOIN meetings AS meeting ON meeting.id = claimed.meeting_id
             LEFT JOIN LATERAL (
                SELECT decode(
                    string_agg(encode(chunk.audio_data, 'hex'), '' ORDER BY chunk.sequence),
                    'hex'
                ) AS audio_data
                FROM meeting_recording_chunks AS chunk
                WHERE chunk.recording_id = claimed.id
             ) AS chunks ON TRUE",
        )
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        Ok(row.map(ClaimedMeetingTranscription::from))
    }

    /// Marks a claimed recording as actively transcribing.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn start_meeting_transcription(
        &self,
        recording_id: Uuid,
        runner_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        if !is_v7(recording_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "UPDATE meeting_recordings
             SET state = 'running',
                 claim_expires_at = NOW() + ($3 * INTERVAL '1 millisecond')
             WHERE id = $1 AND claim_owner = $2 AND state = 'claimed'",
        )
        .bind(recording_id)
        .bind(runner_id)
        .bind(lease_millis)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(result.rows_affected() == 1)
    }

    /// Stores a final speaker-attributed transcript and queues meeting analysis.
    ///
    /// # Errors
    ///
    /// Returns a validation, lease, ownership, or persistence error.
    pub async fn complete_meeting_transcription(
        &self,
        job: &ClaimedMeetingTranscription,
        runner_id: &str,
        result: &MeetingTranscriptionResult,
    ) -> Result<bool, StorageError> {
        result.validate()?;
        if !valid_runner_id(runner_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let owned = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM meeting_recordings
                WHERE id = $1 AND meeting_id = $2 AND user_id = $3
                  AND claim_owner = $4 AND state = 'running'
            )",
        )
        .bind(job.recording_id)
        .bind(job.meeting_id)
        .bind(job.user_id)
        .bind(runner_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        if !owned {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        }
        for speaker in &result.speakers {
            sqlx::query(
                "INSERT INTO meeting_speakers (
                    id, meeting_id, speaker_key, display_name, ordinal
                 ) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(speaker.id)
            .bind(job.meeting_id)
            .bind(speaker.speaker_key.trim())
            .bind(trimmed_optional(speaker.display_name.as_deref()))
            .bind(speaker.ordinal)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        for segment in &result.segments {
            let speaker_id = result
                .speakers
                .iter()
                .find(|speaker| speaker.speaker_key == segment.speaker_key)
                .map(|speaker| speaker.id)
                .ok_or(StorageError::InvalidConfiguration)?;
            sqlx::query(
                "INSERT INTO meeting_transcript_segments (
                    id, meeting_id, speaker_id, ordinal,
                    starts_at_milliseconds, ends_at_milliseconds,
                    text, confidence
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(segment.id)
            .bind(job.meeting_id)
            .bind(speaker_id)
            .bind(segment.ordinal)
            .bind(segment.starts_at_milliseconds)
            .bind(segment.ends_at_milliseconds)
            .bind(segment.text.trim())
            .bind(segment.confidence)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings SET transcript = $3, status = 'queued'
             WHERE id = $1 AND user_id = $2 AND status = 'transcribing'
             RETURNING version",
        )
        .bind(job.meeting_id)
        .bind(job.user_id)
        .bind(result.transcript.trim())
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let recording_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meeting_recordings
             SET state = 'completed', claim_owner = NULL,
                 claim_expires_at = NULL, finished_at = NOW(),
                 error_code = NULL
             WHERE id = $1 AND claim_owner = $2 AND state = 'running'
             RETURNING version",
        )
        .bind(job.recording_id)
        .bind(runner_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let analysis_job_id = Uuid::now_v7();
        let analysis_job_version = sqlx::query_scalar::<_, i64>(
            "INSERT INTO meeting_analysis_jobs (id, meeting_id, user_id)
             VALUES ($1, $2, $3)
             RETURNING version",
        )
        .bind(analysis_job_id)
        .bind(job.meeting_id)
        .bind(job.user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            job.user_id,
            "meeting",
            job.meeting_id,
            meeting_version,
        )
        .await?;
        append_change(
            &mut transaction,
            job.user_id,
            "meeting_recording",
            job.recording_id,
            recording_version,
        )
        .await?;
        append_change(
            &mut transaction,
            job.user_id,
            "meeting_analysis_job",
            analysis_job_id,
            analysis_job_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Fails a lease-owned transcription without storing provider detail.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn fail_meeting_transcription(
        &self,
        recording_id: Uuid,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(recording_id) || !valid_runner_id(runner_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "UPDATE meeting_recordings
             SET state = 'failed', claim_owner = NULL,
                 claim_expires_at = NULL, error_code = $3, finished_at = NOW()
             WHERE id = $1 AND claim_owner = $2
               AND state IN ('claimed', 'running')
             RETURNING user_id, meeting_id, version",
        )
        .bind(recording_id)
        .bind(runner_id)
        .bind(error_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((user_id, meeting_id, recording_version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings SET status = 'failed'
             WHERE id = $1 AND user_id = $2
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_recording",
            recording_id,
            recording_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Lists recent meeting summaries without transferring source transcripts.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn meetings_for_user(&self, user_id: Uuid) -> Result<Vec<Meeting>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, MeetingRow>(
            "SELECT meeting.id, meeting.workspace_id, meeting.project_id,
                project.title AS project_title, meeting.title, meeting.purpose,
                meeting.participants, ''::text AS transcript, meeting.started_at,
                meeting.duration_seconds, meeting.status, meeting.summary,
                meeting.topics, meeting.risks, meeting.follow_up,
                meeting.analyzed_at, meeting.created_at, meeting.updated_at,
                meeting.version
             FROM meetings AS meeting
             LEFT JOIN projects AS project ON project.id = meeting.project_id
             WHERE meeting.user_id = $1
             ORDER BY meeting.created_at DESC, meeting.id DESC
             LIMIT 100",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?
        .into_iter()
        .map(Meeting::try_from)
        .collect()
    }

    /// Returns one meeting with every review item for its owner.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn meeting_detail_for_user(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
    ) -> Result<Option<MeetingDetail>, StorageError> {
        if !is_v7(user_id) || !is_v7(meeting_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, MeetingRow>(
            "SELECT meeting.id, meeting.workspace_id, meeting.project_id,
                project.title AS project_title, meeting.title, meeting.purpose,
                meeting.participants, meeting.transcript, meeting.started_at,
                meeting.duration_seconds, meeting.status, meeting.summary,
                meeting.topics, meeting.risks, meeting.follow_up,
                meeting.analyzed_at, meeting.created_at, meeting.updated_at,
                meeting.version
             FROM meetings AS meeting
             LEFT JOIN projects AS project ON project.id = meeting.project_id
             WHERE meeting.user_id = $1 AND meeting.id = $2",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let decisions = sqlx::query_as::<_, MeetingDecisionRow>(
            "SELECT id, content, rationale, source_excerpt, source_timestamp_seconds
             FROM meeting_decisions
             WHERE meeting_id = $1
             ORDER BY created_at, id",
        )
        .bind(meeting_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?
        .into_iter()
        .map(MeetingDecision::try_from)
        .collect::<Result<Vec<_>, _>>()?;
        let action_items = meeting_action_rows(self, user_id, meeting_id)
            .await?
            .into_iter()
            .map(MeetingActionItem::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let recording = sqlx::query_as::<_, MeetingRecordingRow>(
            "SELECT recording.id, recording.meeting_id, recording.state,
                recording.mime_type, recording.notes,
                recording.duration_milliseconds, recording.chunk_count,
                recording.byte_length, recording.error_code,
                recording.started_at, recording.finalized_at,
                recording.finished_at, recording.updated_at, recording.version
             FROM meeting_recordings AS recording
             WHERE recording.user_id = $1 AND recording.meeting_id = $2",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?
        .map(MeetingRecording::try_from)
        .transpose()?;
        let speakers = sqlx::query_as::<_, MeetingSpeakerRow>(
            "SELECT speaker.id, speaker.meeting_id, speaker.speaker_key,
                speaker.display_name, speaker.ordinal
             FROM meeting_speakers AS speaker
             INNER JOIN meetings AS meeting ON meeting.id = speaker.meeting_id
             WHERE meeting.user_id = $1 AND speaker.meeting_id = $2
             ORDER BY speaker.ordinal, speaker.id",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?
        .into_iter()
        .map(MeetingSpeaker::from)
        .collect();
        let transcript_segments = sqlx::query_as::<_, MeetingTranscriptSegmentRow>(
            "SELECT segment.id, segment.meeting_id, segment.speaker_id,
                speaker.speaker_key, speaker.display_name AS speaker_name,
                segment.ordinal, segment.starts_at_milliseconds,
                segment.ends_at_milliseconds, segment.text,
                segment.confidence, segment.is_final
             FROM meeting_transcript_segments AS segment
             INNER JOIN meeting_speakers AS speaker ON speaker.id = segment.speaker_id
             INNER JOIN meetings AS meeting ON meeting.id = segment.meeting_id
             WHERE meeting.user_id = $1 AND segment.meeting_id = $2
             ORDER BY segment.ordinal, segment.id",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?
        .into_iter()
        .map(MeetingTranscriptSegment::from)
        .collect();
        Ok(Some(MeetingDetail {
            meeting: Meeting::try_from(row)?,
            recording,
            speakers,
            transcript_segments,
            decisions,
            action_items,
        }))
    }

    /// Claims the oldest queued meeting analysis for this worker.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn claim_next_meeting_analysis(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedMeetingAnalysis>, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let row = sqlx::query_as::<_, ClaimedMeetingAnalysisRow>(
            "WITH recovered AS (
                UPDATE meeting_analysis_jobs
                SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL
                WHERE state = 'claimed' AND claim_expires_at < NOW()
             ), candidate AS (
                SELECT id FROM meeting_analysis_jobs
                WHERE state = 'queued'
                ORDER BY created_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             ), claimed AS (
                UPDATE meeting_analysis_jobs AS job
                SET state = 'claimed', claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE job.id = candidate.id
                RETURNING job.id, job.meeting_id, job.user_id
             )
             SELECT claimed.id, claimed.meeting_id, claimed.user_id,
                meeting.title, meeting.purpose, meeting.participants,
                meeting.transcript, meeting.project_id, project.title AS project_title,
                meeting.started_at,
                selected_model.id AS processing_model_id,
                selected_effort.effort AS processing_reasoning_effort
             FROM claimed
             INNER JOIN meetings AS meeting ON meeting.id = claimed.meeting_id
             LEFT JOIN projects AS project ON project.id = meeting.project_id
             LEFT JOIN agent_preferences AS preference
                ON preference.user_id = claimed.user_id
             LEFT JOIN agent_models AS selected_model
                ON selected_model.id = preference.model_id
               AND selected_model.available = TRUE
             LEFT JOIN agent_models AS default_model
                ON default_model.is_default = TRUE AND default_model.available = TRUE
             LEFT JOIN agent_model_reasoning_efforts AS selected_effort
                ON selected_effort.model_id = COALESCE(selected_model.id, default_model.id)
               AND selected_effort.effort = preference.reasoning_effort",
        )
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        Ok(row.map(ClaimedMeetingAnalysis::from))
    }

    /// Marks a claimed meeting analysis as running before contacting Codex.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn start_meeting_analysis(
        &self,
        job_id: Uuid,
        runner_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        if !is_v7(job_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, Uuid, i64)>(
            "UPDATE meeting_analysis_jobs
             SET state = 'running', started_at = COALESCE(started_at, NOW()),
                 claim_expires_at = NOW() + ($3 * INTERVAL '1 millisecond')
             WHERE id = $1 AND claim_owner = $2 AND state = 'claimed'
             RETURNING user_id, meeting_id, id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((user_id, meeting_id, job_id, job_version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings SET status = 'analyzing'
             WHERE id = $1 AND user_id = $2 AND status = 'queued'
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Atomically stores validated AI analysis and releases it for review.
    ///
    /// # Errors
    ///
    /// Returns a validation, lease, ownership, or persistence error.
    #[allow(clippy::too_many_lines)] // One transaction keeps analysis rows and queue state atomic.
    pub async fn complete_meeting_analysis(
        &self,
        job: &ClaimedMeetingAnalysis,
        runner_id: &str,
        result: &MeetingAnalysisResult,
    ) -> Result<bool, StorageError> {
        result.validate()?;
        if !valid_runner_id(runner_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        if result
            .action_items
            .iter()
            .any(|item| item.project_id.is_some() && item.project_id != job.project_id)
        {
            return Err(StorageError::IdentityConflict);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let owned = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM meeting_analysis_jobs
                WHERE id = $1 AND meeting_id = $2 AND user_id = $3
                  AND claim_owner = $4 AND state = 'running'
            )",
        )
        .bind(job.id)
        .bind(job.meeting_id)
        .bind(job.user_id)
        .bind(runner_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        if !owned {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        }
        for decision in &result.decisions {
            sqlx::query(
                "INSERT INTO meeting_decisions (
                    id, meeting_id, content, rationale, source_excerpt,
                    source_timestamp_seconds
                 ) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(decision.id)
            .bind(job.meeting_id)
            .bind(decision.content.trim())
            .bind(trimmed_optional(decision.rationale.as_deref()))
            .bind(decision.source_excerpt.trim())
            .bind(decision.source_timestamp_seconds)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        for item in &result.action_items {
            sqlx::query(
                "INSERT INTO meeting_action_items (
                    id, meeting_id, kind, project_id, title, notes,
                    assignee_name, priority, due_at, starts_at, ends_at,
                    time_zone, source_excerpt, confidence, target_entity_id
                 ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
                 )",
            )
            .bind(item.id)
            .bind(job.meeting_id)
            .bind(action_kind_value(item.kind))
            .bind(item.project_id)
            .bind(item.title.trim())
            .bind(trimmed_optional(item.notes.as_deref()))
            .bind(trimmed_optional(item.assignee_name.as_deref()))
            .bind(item.priority)
            .bind(item.due_at)
            .bind(item.starts_at)
            .bind(item.ends_at)
            .bind(trimmed_optional(item.time_zone.as_deref()))
            .bind(item.source_excerpt.trim())
            .bind(item.confidence)
            .bind(item.target_entity_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings
             SET status = 'review_ready', summary = $3, topics = $4, risks = $5,
                 follow_up = $6, analyzed_at = NOW()
             WHERE id = $1 AND user_id = $2 AND status = 'analyzing'
             RETURNING version",
        )
        .bind(job.meeting_id)
        .bind(job.user_id)
        .bind(result.summary.trim())
        .bind(trimmed_strings(&result.topics))
        .bind(trimmed_strings(&result.risks))
        .bind(trimmed_optional(result.follow_up.as_deref()))
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let job_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meeting_analysis_jobs
             SET state = 'completed', claim_owner = NULL, claim_expires_at = NULL,
                 finished_at = NOW()
             WHERE id = $1 AND claim_owner = $2 AND state = 'running'
             RETURNING version",
        )
        .bind(job.id)
        .bind(runner_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            job.user_id,
            "meeting",
            job.meeting_id,
            meeting_version,
        )
        .await?;
        append_change(
            &mut transaction,
            job.user_id,
            "meeting_analysis_job",
            job.id,
            job_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Fails a lease-owned analysis without exposing provider error details.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn fail_meeting_analysis(
        &self,
        job_id: Uuid,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id) || !valid_runner_id(runner_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "UPDATE meeting_analysis_jobs
             SET state = 'failed', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = $3, finished_at = NOW()
             WHERE id = $1 AND claim_owner = $2 AND state IN ('claimed', 'running')
             RETURNING user_id, meeting_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(error_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((user_id, meeting_id, job_version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings SET status = 'failed'
             WHERE id = $1 AND user_id = $2
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Queues a failed meeting analysis again for an explicit owner retry.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, state, or persistence error.
    pub async fn retry_meeting_analysis(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
    ) -> Result<Meeting, StorageError> {
        if !is_v7(user_id) || !is_v7(meeting_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let job_row = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE meeting_analysis_jobs AS job
             SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = NULL, started_at = NULL, finished_at = NULL
             FROM meetings AS meeting
             WHERE job.meeting_id = $2 AND job.user_id = $1
               AND meeting.id = job.meeting_id AND meeting.user_id = $1
               AND job.state = 'failed' AND meeting.status = 'failed'
               AND job.attempt_count < 8
             RETURNING job.id, job.version",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((job_id, job_version)) = job_row else {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        };
        let row = sqlx::query_as::<_, MeetingRow>(
            "UPDATE meetings AS meeting
             SET status = 'queued', summary = NULL, topics = '{}', risks = '{}',
                 follow_up = NULL, analyzed_at = NULL
             WHERE meeting.id = $2 AND meeting.user_id = $1
             RETURNING meeting.id, meeting.workspace_id, meeting.project_id,
                NULL::text AS project_title, meeting.title, meeting.purpose,
                meeting.participants, meeting.transcript, meeting.started_at,
                meeting.duration_seconds, meeting.status, meeting.summary,
                meeting.topics, meeting.risks, meeting.follow_up,
                meeting.analyzed_at, meeting.created_at, meeting.updated_at,
                meeting.version",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Meeting::try_from(row)
    }

    /// Fails provider-started analyses whose lease expired after a restart.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn fail_expired_running_meeting_analyses(
        &self,
        error_code: &str,
    ) -> Result<usize, StorageError> {
        if !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, i64)>(
            "UPDATE meeting_analysis_jobs
             SET state = 'failed', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = $1, finished_at = NOW()
             WHERE state = 'running' AND claim_expires_at < NOW()
             RETURNING id, user_id, meeting_id, version",
        )
        .bind(error_code)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        for (job_id, user_id, meeting_id, job_version) in &rows {
            let meeting_version = sqlx::query_scalar::<_, i64>(
                "UPDATE meetings SET status = 'failed'
                 WHERE id = $1 AND user_id = $2
                 RETURNING version",
            )
            .bind(meeting_id)
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(classify)?;
            append_change(
                &mut transaction,
                *user_id,
                "meeting_analysis_job",
                *job_id,
                *job_version,
            )
            .await?;
            append_change(
                &mut transaction,
                *user_id,
                "meeting",
                *meeting_id,
                meeting_version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(rows.len())
    }

    /// Returns one review item only when both meeting and item belong to owner.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn meeting_action_item_for_user(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
        item_id: Uuid,
    ) -> Result<Option<MeetingActionItem>, StorageError> {
        if !is_v7(user_id) || !is_v7(meeting_id) || !is_v7(item_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, MeetingActionItemRow>(
            "SELECT item.id, item.meeting_id, item.kind, item.project_id,
                item.title, item.notes, item.assignee_name, item.priority, item.due_at,
                item.starts_at, item.ends_at, item.time_zone,
                item.source_excerpt, item.confidence, item.status,
                item.target_entity_id, item.version
             FROM meeting_action_items AS item
             INNER JOIN meetings AS meeting ON meeting.id = item.meeting_id
             WHERE meeting.user_id = $1 AND meeting.id = $2 AND item.id = $3",
        )
        .bind(user_id)
        .bind(meeting_id)
        .bind(item_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?
        .map(MeetingActionItem::try_from)
        .transpose()
    }

    /// Replaces a suggested action with the owner's reviewed values.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, version-conflict, or persistence error.
    pub async fn update_meeting_action_item(
        &self,
        update: &MeetingActionItemUpdate,
    ) -> Result<Option<MeetingActionItem>, StorageError> {
        update.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, MeetingActionItemRow>(
            "UPDATE meeting_action_items AS item
             SET title = $5, notes = $6, assignee_name = $7, priority = $8,
                 due_at = $9, starts_at = $10, ends_at = $11, time_zone = $12
             FROM meetings AS meeting
             WHERE item.id = $3 AND item.meeting_id = $2
               AND meeting.id = item.meeting_id AND meeting.user_id = $1
               AND item.status = 'suggested' AND item.version = $4
               AND item.kind = $13
             RETURNING item.id, item.meeting_id, item.kind, item.project_id,
                item.title, item.notes, item.assignee_name, item.priority,
                item.due_at, item.starts_at, item.ends_at, item.time_zone,
                item.source_excerpt, item.confidence, item.status,
                item.target_entity_id, item.version",
        )
        .bind(update.user_id)
        .bind(update.meeting_id)
        .bind(update.id)
        .bind(update.expected_version)
        .bind(update.title.trim())
        .bind(trimmed_optional(update.notes.as_deref()))
        .bind(trimmed_optional(update.assignee_name.as_deref()))
        .bind(update.priority)
        .bind(update.due_at)
        .bind(update.starts_at)
        .bind(update.ends_at)
        .bind(trimmed_optional(update.time_zone.as_deref()))
        .bind(action_kind_value(update.kind))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        append_change(
            &mut transaction,
            update.user_id,
            "meeting_action_item",
            update.id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        MeetingActionItem::try_from(row).map(Some)
    }

    /// Records the owner's final decision after a target action succeeds.
    ///
    /// # Errors
    ///
    /// Returns a validation, conflict, or persistence error.
    pub async fn decide_meeting_action_item(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
        item_id: Uuid,
        decision: MeetingActionStatus,
    ) -> Result<MeetingActionItem, StorageError> {
        if !is_v7(user_id)
            || !is_v7(meeting_id)
            || !is_v7(item_id)
            || decision == MeetingActionStatus::Suggested
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, MeetingActionItemRow>(
            "UPDATE meeting_action_items AS item
             SET status = $4,
                 applied_at = CASE WHEN $4 = 'applied' THEN NOW() ELSE NULL END,
                 rejected_at = CASE WHEN $4 = 'rejected' THEN NOW() ELSE NULL END
             FROM meetings AS meeting
             WHERE item.id = $3 AND item.meeting_id = $2
               AND meeting.id = item.meeting_id AND meeting.user_id = $1
               AND item.status IN ('suggested', $4)
             RETURNING item.id, item.meeting_id, item.kind, item.project_id,
                item.title, item.notes, item.assignee_name, item.priority, item.due_at,
                item.starts_at, item.ends_at, item.time_zone,
                item.source_excerpt, item.confidence, item.status,
                item.target_entity_id, item.version",
        )
        .bind(user_id)
        .bind(meeting_id)
        .bind(item_id)
        .bind(action_status_value(decision))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        };
        append_change(
            &mut transaction,
            user_id,
            "meeting_action_item",
            item_id,
            row.version,
        )
        .await?;
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings
             SET status = CASE WHEN EXISTS(
                    SELECT 1 FROM meeting_action_items
                    WHERE meeting_id = $1 AND status = 'suggested'
                 ) THEN 'review_ready' ELSE 'applied' END
             WHERE id = $1 AND user_id = $2 AND status IN ('review_ready', 'applied')
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        MeetingActionItem::try_from(row)
    }
}

async fn meeting_action_rows(
    database: &Database,
    user_id: Uuid,
    meeting_id: Uuid,
) -> Result<Vec<MeetingActionItemRow>, StorageError> {
    sqlx::query_as::<_, MeetingActionItemRow>(
        "SELECT item.id, item.meeting_id, item.kind, item.project_id,
            item.title, item.notes, item.assignee_name, item.priority, item.due_at,
            item.starts_at, item.ends_at, item.time_zone,
            item.source_excerpt, item.confidence, item.status,
            item.target_entity_id, item.version
         FROM meeting_action_items AS item
         INNER JOIN meetings AS meeting ON meeting.id = item.meeting_id
         WHERE meeting.user_id = $1 AND meeting.id = $2
         ORDER BY item.created_at, item.id",
    )
    .bind(user_id)
    .bind(meeting_id)
    .fetch_all(database.pool())
    .await
    .map_err(classify)
}

async fn meeting_scope_is_owned(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> Result<bool, StorageError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT
            ($2::uuid IS NULL OR EXISTS(
                SELECT 1 FROM workspaces WHERE id = $2 AND user_id = $1
            ))
            AND ($3::uuid IS NULL OR EXISTS(
                SELECT 1 FROM projects
                WHERE id = $3 AND user_id = $1
                  AND ($2::uuid IS NULL OR workspace_id = $2)
            ))",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(project_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

const fn action_kind_value(kind: MeetingActionKind) -> &'static str {
    match kind {
        MeetingActionKind::Task => "task",
        MeetingActionKind::Schedule => "schedule",
    }
}

const fn action_status_value(status: MeetingActionStatus) -> &'static str {
    match status {
        MeetingActionStatus::Suggested => "suggested",
        MeetingActionStatus::Applied => "applied",
        MeetingActionStatus::Rejected => "rejected",
    }
}

fn parse_meeting_status(value: &str) -> Result<MeetingStatus, StorageError> {
    match value {
        "recording" => Ok(MeetingStatus::Recording),
        "transcribing" => Ok(MeetingStatus::Transcribing),
        "queued" => Ok(MeetingStatus::Queued),
        "analyzing" => Ok(MeetingStatus::Analyzing),
        "review_ready" => Ok(MeetingStatus::ReviewReady),
        "applied" => Ok(MeetingStatus::Applied),
        "failed" => Ok(MeetingStatus::Failed),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_recording_state(value: &str) -> Result<MeetingRecordingState, StorageError> {
    match value {
        "recording" => Ok(MeetingRecordingState::Recording),
        "queued" => Ok(MeetingRecordingState::Queued),
        "claimed" => Ok(MeetingRecordingState::Claimed),
        "running" => Ok(MeetingRecordingState::Running),
        "completed" => Ok(MeetingRecordingState::Completed),
        "failed" => Ok(MeetingRecordingState::Failed),
        "cancelled" => Ok(MeetingRecordingState::Cancelled),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_action_kind(value: &str) -> Result<MeetingActionKind, StorageError> {
    match value {
        "task" => Ok(MeetingActionKind::Task),
        "schedule" => Ok(MeetingActionKind::Schedule),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_action_status(value: &str) -> Result<MeetingActionStatus, StorageError> {
    match value {
        "suggested" => Ok(MeetingActionStatus::Suggested),
        "applied" => Ok(MeetingActionStatus::Applied),
        "rejected" => Ok(MeetingActionStatus::Rejected),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn claim_lease_millis(runner_id: &str, lease: Duration) -> Result<i64, StorageError> {
    if !valid_runner_id(runner_id) || lease.is_zero() {
        return Err(StorageError::InvalidConfiguration);
    }
    i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)
}

fn valid_runner_id(value: &str) -> bool {
    valid_text(value, 200)
}

fn valid_error_code(value: &str) -> bool {
    valid_text(value, 120)
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
}

fn valid_optional_id(value: Option<Uuid>) -> bool {
    value.is_none_or(is_v7)
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_optional_body_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| valid_body_text(value, maximum))
}

fn valid_text(value: &str, maximum: usize) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

fn valid_body_text(value: &str, maximum: usize) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().count() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn trimmed_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trimmed_strings(values: &[String]) -> Vec<String> {
    values.iter().map(|value| value.trim().to_owned()).collect()
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{
        MeetingActionKind, MeetingAnalysisResult, MeetingTranscriptionResult, NewMeeting,
        NewMeetingActionItem, NewMeetingDecision, NewMeetingSpeaker, NewMeetingTranscriptSegment,
        RecordingChunk, RecordingFinalize, RecordingNoteUpdate,
    };
    use uuid::Uuid;

    #[test]
    fn meeting_input_requires_bounded_source_text() {
        let input = NewMeeting {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            workspace_id: None,
            project_id: None,
            title: "제품 회의".to_owned(),
            purpose: Some("출시 전 검토 범위를 확정한다.".to_owned()),
            participants: vec!["조지민".to_owned()],
            transcript: "지민: 출시 전 흐름을 다시 검토해요.\n담당자: 내일까지 확인할게요."
                .to_owned(),
            started_at: None,
            duration_seconds: Some(600),
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn schedule_suggestion_requires_a_complete_time_window() {
        let result = MeetingAnalysisResult {
            summary: "출시 전 검토 일정을 잡기로 했다.".to_owned(),
            topics: vec!["출시 준비".to_owned()],
            risks: Vec::new(),
            follow_up: None,
            decisions: vec![NewMeetingDecision {
                id: Uuid::now_v7(),
                content: "계약 등록 흐름을 재검토한다.".to_owned(),
                rationale: None,
                source_excerpt: "계약 등록 흐름을 다시 보죠.".to_owned(),
                source_timestamp_seconds: None,
            }],
            action_items: vec![NewMeetingActionItem {
                id: Uuid::now_v7(),
                target_entity_id: Uuid::now_v7(),
                kind: MeetingActionKind::Schedule,
                project_id: None,
                title: "계약 등록 검토".to_owned(),
                notes: None,
                assignee_name: None,
                priority: 1,
                due_at: None,
                starts_at: None,
                ends_at: None,
                time_zone: None,
                source_excerpt: "내일 다시 검토하죠.".to_owned(),
                confidence: 90,
            }],
        };
        assert!(result.validate().is_err());
    }

    #[test]
    fn recording_inputs_keep_chunks_notes_and_duration_bounded() {
        let recording_id = Uuid::now_v7();
        assert!(
            RecordingChunk {
                recording_id,
                user_id: Uuid::now_v7(),
                sequence: 0,
                mime_type: "audio/webm;codecs=opus".to_owned(),
                audio_data: vec![1, 2, 3],
            }
            .validate()
            .is_ok()
        );
        assert!(
            RecordingNoteUpdate {
                recording_id,
                user_id: Uuid::now_v7(),
                notes: "담당자와 마감일을 다시 확인".to_owned(),
            }
            .validate()
            .is_ok()
        );
        assert!(
            RecordingFinalize {
                recording_id,
                user_id: Uuid::now_v7(),
                mime_type: "audio/webm;codecs=opus".to_owned(),
                duration_milliseconds: 65_000,
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn transcript_segments_must_reference_a_known_speaker() {
        let result = MeetingTranscriptionResult {
            transcript: "화자 1: 출시 범위를 정해요.".to_owned(),
            speakers: vec![NewMeetingSpeaker {
                id: Uuid::now_v7(),
                speaker_key: "SPEAKER_00".to_owned(),
                display_name: None,
                ordinal: 0,
            }],
            segments: vec![NewMeetingTranscriptSegment {
                id: Uuid::now_v7(),
                speaker_key: "SPEAKER_01".to_owned(),
                ordinal: 0,
                starts_at_milliseconds: 0,
                ends_at_milliseconds: 1_500,
                text: "출시 범위를 정해요.".to_owned(),
                confidence: Some(92),
            }],
        };
        assert!(result.validate().is_err());
    }
}
