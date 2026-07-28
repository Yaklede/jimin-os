use std::time::Duration;

use jimin_storage::{
    Database,
    meetings::{MeetingTranscriptionResult, NewMeetingSpeaker, NewMeetingTranscriptSegment},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::worker_loop::WorkerError;

const REQUEST_TIMEOUT: Duration = Duration::from_mins(30);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptionResponse {
    transcript: String,
    speakers: Vec<TranscriptionSpeaker>,
    segments: Vec<TranscriptionSegment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptionSpeaker {
    key: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptionSegment {
    speaker_key: String,
    starts_at_milliseconds: i64,
    ends_at_milliseconds: i64,
    text: String,
    confidence: Option<i16>,
}

pub(crate) async fn process_next(
    database: &Database,
    runner_id: &str,
    lease: Duration,
    endpoint: Option<&str>,
) -> Result<bool, WorkerError> {
    let Some(endpoint) = endpoint else {
        return Ok(false);
    };
    let Some(job) = database
        .claim_next_meeting_transcription(runner_id, lease)
        .await?
    else {
        return Ok(false);
    };
    if !database
        .start_meeting_transcription(job.recording_id, runner_id, REQUEST_TIMEOUT)
        .await?
    {
        return Err(WorkerError::LostLease);
    }

    let client = reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build();
    let client = match client {
        Ok(client) => client,
        Err(_) => {
            fail(
                database,
                job.recording_id,
                runner_id,
                "meeting.transcriber_client",
            )
            .await?;
            return Ok(true);
        }
    };
    let response = client
        .post(format!("{}/v1/transcribe", endpoint.trim_end_matches('/')))
        .header("Content-Type", &job.mime_type)
        .header(
            "X-Meeting-Participants",
            serde_json::to_string(&job.participants).unwrap_or_else(|_| "[]".to_owned()),
        )
        .body(job.audio_data.clone())
        .send()
        .await;
    let response = match response {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            let code = if response.status().as_u16() == 503 {
                "meeting.transcriber_not_ready"
            } else {
                "meeting.transcriber_rejected"
            };
            fail(database, job.recording_id, runner_id, code).await?;
            return Ok(true);
        }
        Err(_) => {
            fail(
                database,
                job.recording_id,
                runner_id,
                "meeting.transcriber_unavailable",
            )
            .await?;
            return Ok(true);
        }
    };
    let parsed = response.json::<TranscriptionResponse>().await;
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(_) => {
            fail(
                database,
                job.recording_id,
                runner_id,
                "meeting.transcriber_invalid_response",
            )
            .await?;
            return Ok(true);
        }
    };
    let result = MeetingTranscriptionResult {
        transcript: parsed.transcript,
        speakers: parsed
            .speakers
            .into_iter()
            .enumerate()
            .map(|(ordinal, speaker)| NewMeetingSpeaker {
                id: Uuid::now_v7(),
                speaker_key: speaker.key,
                display_name: speaker.display_name,
                ordinal: i16::try_from(ordinal).unwrap_or(i16::MAX),
            })
            .collect(),
        segments: parsed
            .segments
            .into_iter()
            .enumerate()
            .map(|(ordinal, segment)| NewMeetingTranscriptSegment {
                id: Uuid::now_v7(),
                speaker_key: segment.speaker_key,
                ordinal: i32::try_from(ordinal).unwrap_or(i32::MAX),
                starts_at_milliseconds: segment.starts_at_milliseconds,
                ends_at_milliseconds: segment.ends_at_milliseconds,
                text: segment.text,
                confidence: segment.confidence,
            })
            .collect(),
    };
    match database
        .complete_meeting_transcription(&job, runner_id, &result)
        .await
    {
        Ok(true) => Ok(true),
        Ok(false) => Err(WorkerError::LostLease),
        Err(jimin_storage::StorageError::InvalidConfiguration) => {
            fail(
                database,
                job.recording_id,
                runner_id,
                "meeting.transcriber_invalid_response",
            )
            .await?;
            Ok(true)
        }
        Err(error) => Err(error.into()),
    }
}

async fn fail(
    database: &Database,
    recording_id: Uuid,
    runner_id: &str,
    code: &str,
) -> Result<(), WorkerError> {
    if database
        .fail_meeting_transcription(recording_id, runner_id, code)
        .await?
    {
        Ok(())
    } else {
        Err(WorkerError::LostLease)
    }
}
