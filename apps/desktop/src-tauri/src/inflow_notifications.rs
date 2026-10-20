//! A native poller keeps checking while the webview is hidden/minimized.
//! Credentials stay in memory and never enter logs or notification payloads.
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Client, Url, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;

#[derive(Clone)]
struct Configuration {
    base_url: String,
    access: SecretString,
}

pub struct InflowNotifications(watch::Sender<Option<Configuration>>, Mutex<Diagnostics>);

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    status: String,
    last_checked_at: Option<u64>,
    last_accepted_at: Option<u64>,
    eligible_count: usize,
    suppressed_count: usize,
}

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    base_url: String,
    cursor: (i64, String),
    seen: Vec<(String, i32, i64)>,
}

async fn save_checkpoint(
    app: &tauri::AppHandle,
    owner: &str,
    base: &str,
    cursor: &(i64, String),
    seen: &HashMap<(String, i32), i64>,
) {
    let Some(path) = checkpoint_path(app, owner) else {
        return;
    };
    let saved = Checkpoint {
        base_url: base.to_owned(),
        cursor: cursor.clone(),
        seen: seen
            .iter()
            .take(2000)
            .map(|((id, revision), at)| (id.clone(), *revision, *at))
            .collect(),
    };
    let Ok(bytes) = serde_json::to_vec(&saved) else {
        return;
    };
    if let Some(parent) = path.parent()
        && tokio::fs::create_dir_all(parent).await.is_err()
    {
        return;
    }
    let temporary = path.with_extension("json.tmp");
    if tokio::fs::write(&temporary, bytes).await.is_ok() {
        let _ = tokio::fs::rename(temporary, path).await;
    }
}

fn checkpoint_cursor(saved: &Checkpoint, base: &str, now: i64) -> Option<(i64, String)> {
    if saved.base_url != base
        || saved.cursor.0 < 0
        || saved.cursor.0 > now
        || uuid::Uuid::parse_str(&saved.cursor.1).is_err()
    {
        return None;
    }
    Some((
        saved.cursor.0.max((now - 86_400_000).max(0)),
        saved.cursor.1.clone(),
    ))
}

fn now_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn diagnostic_status(
    app: &tauri::AppHandle,
    status: &str,
    checked: bool,
    eligible: usize,
    suppressed: usize,
    accepted: bool,
) {
    if let Ok(mut value) = app.state::<InflowNotifications>().1.lock() {
        status.clone_into(&mut value.status);
        if checked {
            value.last_checked_at = Some(now_millis());
        }
        value.eligible_count = eligible;
        value.suppressed_count = suppressed;
        if accepted {
            value.last_accepted_at = Some(now_millis());
        }
        let _ = app.emit("inflow-notification-diagnostics", value.clone());
    }
    let _ = app.emit("inflow-notification-status", status);
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects owned state arguments.
pub fn inflow_notification_diagnostics(
    state: tauri::State<'_, InflowNotifications>,
) -> Result<Diagnostics, String> {
    state
        .1
        .lock()
        .map(|value| value.clone())
        .map_err(|_| "알림 상태를 다시 확인해 주세요.".to_owned())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects an owned AppHandle.
pub fn test_inflow_notification(app: tauri::AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("Jimin OS 알림 확인")
        .body("이 알림이 보이면 Mac 알림 표시가 허용되어 있어요.")
        .show()
        .map_err(|_| "테스트 알림을 보내지 못했어요. Mac 알림 설정을 확인해 주세요.".to_owned())
}

fn checkpoint_path(app: &tauri::AppHandle, user_id: &str) -> Option<std::path::PathBuf> {
    let id = uuid::Uuid::parse_str(user_id).ok()?;
    Some(
        app.path()
            .app_local_data_dir()
            .ok()?
            .join(format!("inflow-notifications-{id}.json")),
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feed {
    #[serde(default)]
    user_id: String,
    items: Vec<Item>,
    next_epoch_millis: i64,
    next_id: String,
    has_more: bool,
}

#[derive(Deserialize)]
struct Item {
    id: String,
    revision: i32,
    title: String,
    body: String,
    #[serde(rename = "occurredAtEpochMillis")]
    occurred_at: i64,
}

pub fn initialize(app: &tauri::AppHandle) -> Result<(), reqwest::Error> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(Policy::none())
        .build()?;
    let (sender, receiver) = watch::channel(None);
    app.manage(InflowNotifications(
        sender,
        Mutex::new(Diagnostics {
            status: "checking".to_owned(),
            ..Diagnostics::default()
        }),
    ));
    tauri::async_runtime::spawn(run(app.clone(), client, receiver));
    Ok(())
}

#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments."
)]
pub fn configure_inflow_notifications(
    state: tauri::State<'_, InflowNotifications>,
    base_url: String,
    access: Option<String>,
) -> Result<(), String> {
    let config = match access {
        None => None,
        Some(value) => {
            if !valid_base_url(&base_url)
                || value.is_empty()
                || value.len() > 8192
                || value.chars().any(char::is_whitespace)
            {
                return Err("알림 연결을 확인하고 다시 시도해 주세요.".to_owned());
            }
            Some(Configuration {
                base_url: base_url.trim_end_matches('/').to_owned(),
                access: SecretString::from(value),
            })
        }
    };
    state.0.send_replace(config);
    Ok(())
}

fn valid_base_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && (url.scheme() == "https"
            || (cfg!(debug_assertions)
                && url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
}

async fn fetch(
    client: &Client,
    config: &Configuration,
    cursor: Option<&(i64, String)>,
) -> Result<Feed, &'static str> {
    let mut url =
        Url::parse(&format!("{}/v1/push/inflow", config.base_url)).map_err(|_| "invalid")?;
    if let Some((after, id)) = cursor {
        url.query_pairs_mut()
            .append_pair("afterEpochMillis", &after.to_string())
            .append_pair("afterId", id);
    }
    let response = client
        .get(url)
        .bearer_auth(config.access.expose_secret())
        .send()
        .await
        .map_err(|_| "offline")?;
    if response.status().as_u16() == 401 {
        return Err("unauthorized");
    }
    let mut response = response.error_for_status().map_err(|_| "offline")?;
    if response
        .content_length()
        .is_some_and(|size| size > 512 * 1024)
    {
        return Err("invalid");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "offline")? {
        if bytes.len() + chunk.len() > 512 * 1024 {
            return Err("invalid");
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json_decode(&bytes)
}

fn serde_json_decode(bytes: &[u8]) -> Result<Feed, &'static str> {
    // reqwest's JSON decoder is not used so the response size is bounded first.
    serde_json::from_slice(bytes).map_err(|_| "invalid")
}

fn notify_unseen(
    items: Vec<Item>,
    seen: &mut HashMap<(String, i32), i64>,
    mut show: impl FnMut(String, String) -> bool,
) -> bool {
    let fresh: Vec<_> = items
        .into_iter()
        .filter(|item| {
            !seen.contains_key(&(item.id.clone(), item.revision))
                && !item.title.is_empty()
                && item.title.chars().count() <= 120
                && item.body.chars().count() <= 240
        })
        .collect();
    if fresh.len() > 3 {
        let title = format!("확인할 업무 대화 {}개가 있어요", fresh.len());
        let body = fresh
            .iter()
            .take(3)
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(240)
            .collect();
        if !show(title, body) {
            return false;
        }
        for item in fresh {
            seen.insert((item.id, item.revision), item.occurred_at);
        }
        return true;
    }
    let mut success = true;
    for item in fresh {
        if show(item.title, item.body) {
            seen.insert((item.id, item.revision), item.occurred_at);
        } else {
            success = false;
        }
    }
    success
}

#[allow(clippy::too_many_lines)] // One cancellation-safe polling loop maintains account, cursor and receipt checkpoints together.
async fn run(
    app: tauri::AppHandle,
    client: Client,
    mut receiver: watch::Receiver<Option<Configuration>>,
) {
    let mut cursor: Option<(i64, String)> = None;
    let mut floor = 0;
    let mut seen = HashMap::new();
    let mut previous_base: Option<String> = None;
    let mut owner = String::new();
    loop {
        let config = receiver.borrow_and_update().clone();
        if let Some(config) = config {
            if previous_base.as_deref() != Some(config.base_url.as_str()) {
                cursor = None;
                seen.clear();
                previous_base = Some(config.base_url.clone());
                owner.clear();
            }
            // Configuration changes cancel an in-flight request, preventing
            // notifications from a logged-out or replaced session.
            let fetched = tokio::select! {
                result = fetch(&client, &config, cursor.as_ref()) => Some(result),
                changed = receiver.changed() => { if changed.is_err() { return; } None },
            };
            let Some(fetched) = fetched else {
                continue;
            };
            match fetched {
                Ok(feed) => {
                    if owner != feed.user_id && !feed.user_id.is_empty() {
                        owner.clone_from(&feed.user_id);
                        seen.clear();
                        cursor = None;
                        if let Some(path) = checkpoint_path(&app, &owner)
                            && let Ok(bytes) = tokio::fs::read(path).await
                            && bytes.len() <= 512 * 1024
                            && let Ok(saved) = serde_json::from_slice::<Checkpoint>(&bytes)
                            && let Some(restored) =
                                checkpoint_cursor(&saved, &config.base_url, feed.next_epoch_millis)
                        {
                            floor = (feed.next_epoch_millis - 86_400_000).max(0);
                            cursor = Some(restored);
                            seen.extend(
                                saved
                                    .seen
                                    .into_iter()
                                    .take(2000)
                                    .map(|(id, revision, at)| ((id, revision), at)),
                            );
                            continue;
                        }
                    }
                    if cursor.is_none() {
                        floor = feed.next_epoch_millis;
                    }
                    let eligible = feed.items.len();
                    let suppressed = feed
                        .items
                        .iter()
                        .filter(|item| seen.contains_key(&(item.id.clone(), item.revision)))
                        .count();
                    let previous_seen = seen.len();
                    let success = notify_unseen(feed.items, &mut seen, |title, body| {
                        app.notification()
                            .builder()
                            .title(title)
                            .body(body)
                            .show()
                            .is_ok()
                    });
                    let accepted = seen.len() > previous_seen;
                    if success {
                        if feed.has_more {
                            cursor = Some((feed.next_epoch_millis, feed.next_id));
                            save_checkpoint(
                                &app,
                                &owner,
                                &config.base_url,
                                cursor.as_ref().expect("paged cursor exists"),
                                &seen,
                            )
                            .await;
                            diagnostic_status(
                                &app,
                                "connected",
                                true,
                                eligible,
                                suppressed,
                                accepted,
                            );
                            continue;
                        }
                        // Transactions can commit after their timestamp. A short
                        // overlap plus revision deduplication prevents missed rows.
                        cursor = Some((
                            (feed.next_epoch_millis - 120_000).max(floor),
                            "00000000-0000-0000-0000-000000000000".to_owned(),
                        ));
                        seen.retain(|_, timestamp| {
                            *timestamp >= feed.next_epoch_millis - 86_400_000
                        });
                        save_checkpoint(
                            &app,
                            &owner,
                            &config.base_url,
                            cursor.as_ref().expect("successful cursor exists"),
                            &seen,
                        )
                        .await;
                        diagnostic_status(&app, "connected", true, eligible, suppressed, accepted);
                    } else {
                        diagnostic_status(
                            &app,
                            "delivery_failed",
                            true,
                            eligible,
                            suppressed,
                            false,
                        );
                    }
                }
                Err(reason) => {
                    diagnostic_status(&app, reason, true, 0, 0, false);
                    if reason == "unauthorized" {
                        let _ = app.emit("inflow-notification-auth-required", ());
                    }
                }
            }
        } else {
            cursor = None;
            seen.clear();
            previous_base = None;
            owner.clear();
            diagnostic_status(&app, "disabled", false, 0, 0, false);
        }
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(30)) => {},
            changed = receiver.changed() => { if changed.is_err() { return; } },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Checkpoint, Item, checkpoint_cursor, notify_unseen, valid_base_url};
    use std::collections::HashMap;

    fn item(id: &str, revision: i32) -> Item {
        Item {
            id: id.to_owned(),
            revision,
            title: "새 업무 요청 · 오류 확인".to_owned(),
            body: "확인해 주세요.".to_owned(),
            occurred_at: 1000,
        }
    }

    #[test]
    fn checkpoint_is_server_scoped_bounded_and_never_resumes_a_future_cursor() {
        let mut value = Checkpoint {
            base_url: "https://server.example".into(),
            cursor: (1, uuid::Uuid::nil().to_string()),
            seen: vec![],
        };
        assert_eq!(
            checkpoint_cursor(&value, "https://server.example", 100_000_000)
                .unwrap()
                .0,
            13_600_000
        );
        assert!(checkpoint_cursor(&value, "https://other.example", 100_000_000).is_none());
        value.cursor.0 = 101_000_000;
        assert!(checkpoint_cursor(&value, "https://server.example", 100_000_000).is_none());
        value.cursor = (100, "invalid".into());
        assert!(checkpoint_cursor(&value, "https://server.example", 100_000_000).is_none());
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("access"));
        assert!(!encoded.contains("body"));
    }

    #[test]
    fn notifications_are_revision_deduplicated_and_failed_dispatch_retries() {
        let mut seen = HashMap::new();
        assert!(!notify_unseen(vec![item("same", 1)], &mut seen, |_, _| {
            false
        }));
        assert!(seen.is_empty());
        assert!(notify_unseen(vec![item("same", 1)], &mut seen, |_, _| true));
        assert!(notify_unseen(
            vec![item("same", 1)],
            &mut seen,
            |_, _| panic!("duplicate notification")
        ));
        let mut dispatched = 0;
        assert!(notify_unseen(vec![item("same", 2)], &mut seen, |_, _| {
            dispatched += 1;
            true
        }));
        assert_eq!(dispatched, 1);
    }

    #[test]
    fn a_burst_is_one_bounded_notification_not_a_notification_storm() {
        let mut seen = HashMap::new();
        let mut dispatched = 0;
        assert!(notify_unseen(
            (0..5).map(|id| item(&id.to_string(), 1)).collect(),
            &mut seen,
            |title, body| {
                dispatched += 1;
                assert!(title.contains("5개"));
                assert!(body.chars().count() <= 240);
                true
            }
        ));
        assert_eq!(dispatched, 1);
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn rejects_untrusted_redirectable_or_plaintext_hosts() {
        assert!(valid_base_url("https://os.example.test"));
        assert!(!valid_base_url("http://example.test"));
        assert!(!valid_base_url(
            "https://user:compose-validation-fixture@example.test"
        ));
        assert!(!valid_base_url("https://example.test?access=anything"));
        assert!(!valid_base_url("file:///tmp/notifications"));
    }
}
