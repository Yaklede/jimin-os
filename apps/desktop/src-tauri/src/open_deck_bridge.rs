//! Authenticated, loopback-only companion surface. Server sessions never leave Jimin OS.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    os::unix::fs::OpenOptionsExt,
    sync::{Arc, Mutex},
    time::Duration,
};
use subtle::ConstantTimeEq;
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;
use uuid::Uuid;

type Reply = Result<Json<Value>, (StatusCode, Json<Value>)>;
type Completion = oneshot::Sender<Result<(), String>>;
#[derive(Clone)]
pub struct Bridge {
    inner: Arc<Inner>,
}
struct Inner {
    app: tauri::AppHandle,
    credential: String,
    host: String,
    snapshot: Mutex<Option<Value>>,
    pending: Mutex<HashMap<String, Completion>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    pub operation: String,
    pub view: Option<String>,
    pub item_id: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionEvent {
    request_id: String,
    action: Action,
}
fn error(status: StatusCode, code: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error":code})))
}
fn authorized(state: &Bridge, headers: &HeaderMap) -> bool {
    if headers.contains_key("origin")
        || headers.get("host").and_then(|h| h.to_str().ok()) != Some(state.inner.host.as_str())
    {
        return false;
    }
    let expected = format!("Bearer {}", state.inner.credential);
    headers
        .get("authorization")
        .is_some_and(|v| bool::from(v.as_bytes().ct_eq(expected.as_bytes())))
}
fn valid_action(action: &Action) -> bool {
    match action.operation.as_str() {
        "refresh" => action.view.is_none() && action.item_id.is_none(),
        "navigate" => {
            action.item_id.is_none()
                && action.view.as_deref().is_some_and(|v| {
                    matches!(
                        v,
                        "home"
                            | "calendar"
                            | "tasks"
                            | "requests"
                            | "scheduled"
                            | "meetings"
                            | "memory"
                            | "assistant"
                            | "notifications"
                            | "new-schedule"
                            | "new-task"
                    )
                })
        }
        "open-schedule" | "open-task" => {
            action.view.is_none()
                && action
                    .item_id
                    .as_deref()
                    .is_some_and(|id| Uuid::parse_str(id).is_ok())
        }
        _ => false,
    }
}
async fn snapshot(State(state): State<Bridge>, headers: HeaderMap) -> Reply {
    if !authorized(&state, &headers) {
        return Err(error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED"));
    }
    state
        .inner
        .snapshot
        .lock()
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?
        .clone()
        .map(Json)
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "APP_STARTING"))
}
async fn action(
    State(state): State<Bridge>,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Reply {
    if !authorized(&state, &headers) {
        return Err(error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED"));
    }
    if !valid_action(&action) {
        return Err(error(StatusCode::BAD_REQUEST, "UNSUPPORTED_ACTION"));
    }
    let id = Uuid::now_v7().to_string();
    let (tx, rx) = oneshot::channel();
    {
        let mut pending = state
            .inner
            .pending
            .lock()
            .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"))?;
        if pending.len() >= 4 {
            return Err(error(StatusCode::TOO_MANY_REQUESTS, "BUSY"));
        }
        pending.insert(id.clone(), tx);
    }
    let emitted = state.inner.app.emit(
        "open-deck-action",
        ActionEvent {
            request_id: id.clone(),
            action: action.clone(),
        },
    );
    let result = if emitted.is_ok() {
        tokio::time::timeout(Duration::from_secs(8), rx)
            .await
            .ok()
            .and_then(Result::ok)
    } else {
        None
    };
    if let Ok(mut pending) = state.inner.pending.lock() {
        pending.remove(&id);
    }
    match result {
        Some(Ok(())) => {
            if action.operation != "refresh"
                && let Some(window) = state.inner.app.get_webview_window("main")
            {
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(Json(json!({"ok":true})))
        }
        Some(Err(code)) => Err(error(StatusCode::CONFLICT, &code)),
        None => Err(error(StatusCode::GATEWAY_TIMEOUT, "APP_NOT_RESPONDING")),
    }
}
fn valid_snapshot(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let allowed = [
        "schemaVersion",
        "updatedAt",
        "connected",
        "timeZone",
        "range",
        "counts",
        "events",
        "tasks",
        "inbox",
        "routines",
    ];
    object.keys().all(|k| allowed.contains(&k.as_str()))
        && value.get("schemaVersion") == Some(&json!(1))
        && value.to_string().len() <= 131_072
        && value.get("updatedAt").is_some_and(Value::is_number)
        && value.get("connected").is_some_and(Value::is_boolean)
        && value.get("timeZone").is_some_and(Value::is_string)
        && value
            .get("range")
            .is_some_and(|v| fields(v, &["from", "to"], Value::is_string))
        && value.get("counts").is_some_and(|v| {
            fields(
                v,
                &["requests", "updates", "uncertain", "today", "overdue"],
                Value::is_number,
            )
        })
        && rows(
            value,
            "events",
            120,
            &["id", "title", "startsAt", "endsAt", "source"],
        )
        && rows(
            value,
            "tasks",
            40,
            &["id", "title", "dueAt", "assignee", "state"],
        )
        && rows(
            value,
            "inbox",
            30,
            &[
                "id",
                "title",
                "projectName",
                "receivedAt",
                "kind",
                "revision",
            ],
        )
        && rows(
            value,
            "routines",
            20,
            &["id", "title", "nextRunAt", "enabled"],
        )
}
fn fields(value: &Value, allowed: &[&str], valid: fn(&Value) -> bool) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .iter()
            .all(|(key, v)| allowed.contains(&key.as_str()) && valid(v))
    })
}
fn rows(value: &Value, key: &str, limit: usize, allowed: &[&str]) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.len() <= limit
                && items.iter().all(|item| {
                    fields(item, allowed, |v| {
                        v.is_string() || v.is_null() || v.is_boolean() || v.is_number()
                    })
                })
        })
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn publish_open_deck_snapshot(
    state: tauri::State<'_, Bridge>,
    snapshot: Value,
) -> Result<(), String> {
    if !valid_snapshot(&snapshot) {
        return Err("INVALID_SNAPSHOT".into());
    }
    *state.inner.snapshot.lock().map_err(|_| "UNAVAILABLE")? = Some(snapshot);
    Ok(())
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn finish_open_deck_action(
    state: tauri::State<'_, Bridge>,
    request_id: String,
    error_code: Option<String>,
) -> Result<(), String> {
    if let Some(tx) = state
        .inner
        .pending
        .lock()
        .map_err(|_| "UNAVAILABLE")?
        .remove(&request_id)
    {
        let code = error_code.map(|c| {
            if matches!(
                c.as_str(),
                "BUSY_EDITING" | "NOT_FOUND" | "NOT_CONNECTED" | "ACTION_TIMEOUT" | "BUSY"
            ) {
                c
            } else {
                "ACTION_FAILED".into()
            }
        });
        let _ = tx.send(code.map_or(Ok(()), Err));
    }
    Ok(())
}
pub fn initialize(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app = app.clone();
    let directory = app.path().app_data_dir()?;
    fs::create_dir_all(&directory)?;
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:0").await else {
            return;
        };
        let Ok(address) = listener.local_addr() else {
            return;
        };
        let mut credential = String::new();
        for byte in rand::random::<[u8; 32]>() {
            let _ = write!(credential, "{byte:02x}");
        }
        let descriptor =
            json!({"protocol":1,"port":address.port(),"token":credential,"pid":std::process::id()});
        let state = Bridge {
            inner: Arc::new(Inner {
                app: app.clone(),
                credential,
                host: address.to_string(),
                snapshot: Mutex::new(None),
                pending: Mutex::new(HashMap::new()),
            }),
        };
        app.manage(state.clone());
        let file = directory.join("open-deck-bridge.json");
        let temporary = directory.join(format!(".open-deck-{}.tmp", std::process::id()));
        let written = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)?;
            f.write_all(descriptor.to_string().as_bytes())?;
            fs::rename(&temporary, &file)
        })();
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
            return;
        }
        let router = Router::new()
            .route("/v1/snapshot", get(snapshot))
            .route("/v1/action", post(action))
            .layer(DefaultBodyLimit::max(8192))
            .with_state(state);
        let _ = axum::serve(listener, router).await;
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actions_are_allowlisted() {
        assert!(valid_action(&Action {
            operation: "navigate".into(),
            view: Some("calendar".into()),
            item_id: None
        }));
        assert!(!valid_action(&Action {
            operation: "command".into(),
            view: None,
            item_id: None
        }));
        assert!(!valid_action(&Action {
            operation: "navigate".into(),
            view: Some("https://untrusted".into()),
            item_id: None
        }));
    }
    #[test]
    fn snapshot_rejects_credentials_at_top_level() {
        assert!(!valid_snapshot(
            &json!({"schemaVersion":1,"accessToken":"must-not-leave"})
        ));
        let snapshot = json!({"schemaVersion":1,"updatedAt":1,"connected":true,"timeZone":"Asia/Seoul","range":{"from":"","to":""},"counts":{},"events":[],"tasks":[],"inbox":[],"routines":[]});
        assert!(valid_snapshot(&snapshot));
        let mut leaked = snapshot;
        leaked["events"] = json!([{"id":"id","title":"Meeting","accessToken":"must-not-leave"}]);
        assert!(!valid_snapshot(&leaked));
    }
}
