//! A native poller keeps checking while the webview is hidden/minimized.
//! Credentials stay in memory and never enter logs or notification payloads.
use std::{collections::HashMap, time::Duration};

use reqwest::{Client, Url, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;

#[derive(Clone)]
struct Configuration {
    base_url: String,
    access: SecretString,
}

pub struct InflowNotifications(watch::Sender<Option<Configuration>>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feed {
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
    app.manage(InflowNotifications(sender));
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

async fn run(
    app: tauri::AppHandle,
    client: Client,
    mut receiver: watch::Receiver<Option<Configuration>>,
) {
    let mut cursor: Option<(i64, String)> = None;
    let mut floor = 0;
    let mut seen = HashMap::new();
    let mut previous_base: Option<String> = None;
    loop {
        let config = receiver.borrow_and_update().clone();
        if let Some(config) = config {
            if previous_base.as_deref() != Some(config.base_url.as_str()) {
                cursor = None;
                seen.clear();
                previous_base = Some(config.base_url.clone());
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
                    if cursor.is_none() {
                        floor = feed.next_epoch_millis;
                    }
                    let success = notify_unseen(feed.items, &mut seen, |title, body| {
                        app.notification()
                            .builder()
                            .title(title)
                            .body(body)
                            .show()
                            .is_ok()
                    });
                    if success {
                        if feed.has_more {
                            cursor = Some((feed.next_epoch_millis, feed.next_id));
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
                        let _ = app.emit("inflow-notification-status", "connected");
                    } else {
                        let _ = app.emit("inflow-notification-status", "offline");
                    }
                }
                Err(reason) => {
                    let _ = app.emit("inflow-notification-status", reason);
                    if reason == "unauthorized" {
                        let _ = app.emit("inflow-notification-auth-required", ());
                    }
                }
            }
        } else {
            cursor = None;
            seen.clear();
            previous_base = None;
        }
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(30)) => {},
            changed = receiver.changed() => { if changed.is_err() { return; } },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, notify_unseen, valid_base_url};
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
        assert!(!valid_base_url("https://user:password@example.test"));
        assert!(!valid_base_url("https://example.test?access=anything"));
        assert!(!valid_base_url("file:///tmp/notifications"));
    }
}
