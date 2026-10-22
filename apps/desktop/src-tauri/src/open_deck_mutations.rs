//! Durable companion write receipts. Mutations never surface the Mac window.
use super::{Bridge, authorized, error};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRequest {
    pub mutation_id: String,
    pub operation: String,
    pub item_id: Option<String>,
    pub expected_version: Option<i64>,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub completion_note: Option<String>,
    #[serde(default)]
    pub retry: bool,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub mutation: WriteRequest,
    pub status: String,
    pub error: Option<String>,
    pub entity_id: Option<String>,
    pub already_completed: bool,
    pub updated_at: u64,
}
impl Record {
    fn receipt(&self) -> Value {
        json!({"mutationId":self.mutation.mutation_id,"status":self.status,"error":self.error,"entityId":self.entity_id,"alreadyCompleted":self.already_completed})
    }
}
pub struct Store {
    pub records: Vec<Record>,
    path: PathBuf,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
impl Store {
    pub fn load(path: PathBuf) -> Self {
        let mut records: Vec<Record> = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        records.retain(|r| now().saturating_sub(r.updated_at) < 86_400);
        records.truncate(200);
        for r in &mut records {
            if r.status == "running" {
                r.status = "uncertain".into();
                r.error = Some("UNKNOWN_RESULT".into());
            }
        }
        Self { records, path }
    }
    fn save(&self) -> Result<(), String> {
        use std::io::Write;
        let temporary = self.path.with_extension("tmp");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| "UNAVAILABLE")?;
        file.write_all(&serde_json::to_vec(&self.records).map_err(|_| "UNAVAILABLE")?)
            .map_err(|_| "UNAVAILABLE")?;
        file.sync_all().map_err(|_| "UNAVAILABLE")?;
        fs::rename(temporary, &self.path).map_err(|_| "UNAVAILABLE".into())
    }
}
fn valid(m: &WriteRequest) -> bool {
    if !Uuid::parse_str(&m.mutation_id).is_ok_and(|u| u.get_version_num() == 7) {
        return false;
    }
    let small = |s: &Option<String>, max| s.as_ref().is_none_or(|v| v.chars().count() <= max);
    match m.operation.as_str() {
        "create-schedule" => {
            m.item_id.is_none()
                && m.expected_version.is_none()
                && m.completion_note.is_none()
                && m.title
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= 160)
                && small(&m.notes, 2000)
                && m.starts_at
                    .as_deref()
                    .zip(m.ends_at.as_deref())
                    .is_some_and(|(s, e)| {
                        let dates = OffsetDateTime::parse(s, &Rfc3339)
                            .ok()
                            .zip(OffsetDateTime::parse(e, &Rfc3339).ok());
                        dates.is_some_and(|(s, e)| e > s && (e - s).whole_days() <= 366)
                    })
        }
        "complete-task" => {
            m.title.is_none()
                && m.notes.is_none()
                && m.starts_at.is_none()
                && m.ends_at.is_none()
                && m.item_id
                    .as_deref()
                    .is_some_and(|id| Uuid::parse_str(id).is_ok())
                && m.expected_version.is_some_and(|v| v > 0)
                && small(&m.completion_note, 2000)
        }
        _ => false,
    }
}
pub async fn submit(
    State(state): State<Bridge>,
    headers: HeaderMap,
    Json(mut mutation): Json<WriteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    if !authorized(&state, &headers) {
        return Err(error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED"));
    }
    if !valid(&mutation) {
        return Err(error(StatusCode::BAD_REQUEST, "INVALID"));
    }
    let retry = mutation.retry;
    mutation.retry = false;
    let mut store = state
        .inner
        .mutations
        .lock()
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
    if let Some(record) = store
        .records
        .iter()
        .find(|r| r.mutation.mutation_id == mutation.mutation_id)
    {
        if record.mutation != mutation {
            return Err(error(StatusCode::CONFLICT, "ID_REUSED"));
        }
        if !retry || record.status == "running" || record.status == "succeeded" {
            return Ok((StatusCode::OK, Json(record.receipt())));
        }
    } else if mutation.operation == "complete-task" {
        let snapshot = state
            .inner
            .snapshot
            .lock()
            .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
        let owned = snapshot
            .as_ref()
            .and_then(|s| s.get("tasks"))
            .and_then(Value::as_array)
            .is_some_and(|tasks| {
                tasks.iter().any(|t| {
                    t.get("id").and_then(Value::as_str) == mutation.item_id.as_deref()
                        && t.get("version").and_then(Value::as_i64) == mutation.expected_version
                })
            });
        if !owned {
            return Err(error(StatusCode::CONFLICT, "CONFLICT"));
        }
    }
    if store
        .records
        .iter()
        .filter(|r| r.status == "running")
        .count()
        >= 4
    {
        return Err(error(StatusCode::TOO_MANY_REQUESTS, "BUSY"));
    }
    store
        .records
        .retain(|r| r.mutation.mutation_id != mutation.mutation_id);
    while store.records.len() >= 200 {
        let Some(at) = store.records.iter().position(|r| r.status != "running") else {
            return Err(error(StatusCode::TOO_MANY_REQUESTS, "BUSY"));
        };
        store.records.remove(at);
    }
    let record = Record {
        mutation: mutation.clone(),
        status: "running".into(),
        error: None,
        entity_id: None,
        already_completed: false,
        updated_at: now(),
    };
    let receipt = record.receipt();
    store.records.push(record);
    store
        .save()
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
    drop(store);
    if state
        .inner
        .app
        .emit("open-deck-mutation", mutation)
        .is_err()
    {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE, "UNKNOWN_RESULT"));
    }
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}
pub async fn status(
    State(state): State<Bridge>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !authorized(&state, &headers) {
        return Err(error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED"));
    }
    let mut store = state
        .inner
        .mutations
        .lock()
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
    let record = store
        .records
        .iter_mut()
        .find(|r| r.mutation.mutation_id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "NOT_FOUND"))?;
    if record.status == "running" && now().saturating_sub(record.updated_at) > 120 {
        record.status = "uncertain".into();
        record.error = Some("UNKNOWN_RESULT".into());
    }
    let receipt = record.receipt();
    store
        .save()
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
    Ok(Json(receipt))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Outcome {
    entity_id: Option<String>,
    error: Option<String>,
    #[serde(default)]
    already_completed: bool,
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn finish_open_deck_mutation(
    state: tauri::State<'_, Bridge>,
    mutation_id: String,
    outcome: Outcome,
) -> Result<(), String> {
    let mut store = state.inner.mutations.lock().map_err(|_| "UNAVAILABLE")?;
    let record = store
        .records
        .iter_mut()
        .find(|r| r.mutation.mutation_id == mutation_id)
        .ok_or("NOT_FOUND")?;
    if record.status != "running" && record.status != "uncertain" {
        return Ok(());
    }
    let code = outcome.error.map(|e| {
        if [
            "CONFLICT",
            "INVALID",
            "AUTH_REQUIRED",
            "NOT_FOUND",
            "UNAVAILABLE",
            "UNKNOWN_RESULT",
        ]
        .contains(&e.as_str())
        {
            e
        } else {
            "UNKNOWN_RESULT".into()
        }
    });
    let expected = if record.mutation.operation == "create-schedule" {
        Some(record.mutation.mutation_id.as_str())
    } else {
        record.mutation.item_id.as_deref()
    };
    if code.is_none() && outcome.entity_id.as_deref() != expected {
        return Err("INVALID_RESULT".into());
    }
    record.status = if code.as_deref() == Some("UNKNOWN_RESULT") {
        "uncertain"
    } else if code.is_some() {
        "failed"
    } else {
        "succeeded"
    }
    .into();
    record.error = code;
    record.entity_id = outcome.entity_id;
    record.already_completed = outcome.already_completed;
    record.updated_at = now();
    store.save()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn schedule() -> WriteRequest {
        serde_json::from_value(json!({"mutationId":"019f68cb-9400-7000-8000-000000000001","operation":"create-schedule","title":"Meeting","startsAt":"2026-10-22T01:00:00Z","endsAt":"2026-10-22T02:00:00Z"})).unwrap()
    }
    #[test]
    fn validates_write_shapes() {
        let mut m = schedule();
        assert!(valid(&m));
        m.ends_at = Some("2026-10-21T01:00:00Z".into());
        assert!(!valid(&m));
        m = schedule();
        m.operation = "command".into();
        assert!(!valid(&m));
        m = schedule();
        m.expected_version = Some(1);
        assert!(!valid(&m));
    }
    #[test]
    fn restart_never_replays_uncertain_work() {
        let path = std::env::temp_dir().join(format!("deck-journal-{}.json", Uuid::now_v7()));
        let mut store = Store::load(path.clone());
        store.records.push(Record {
            mutation: schedule(),
            status: "running".into(),
            error: None,
            entity_id: None,
            already_completed: false,
            updated_at: now(),
        });
        store.save().unwrap();
        let resumed = Store::load(path.clone());
        assert_eq!(resumed.records[0].status, "uncertain");
        assert!(resumed.records[0].receipt().get("mutation").is_none());
        let _ = fs::remove_file(path);
    }
}
