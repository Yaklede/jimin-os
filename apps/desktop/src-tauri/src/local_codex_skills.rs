//! Read only enabled skill metadata from the installed Mac Codex runtime.
use std::{collections::BTreeMap, path::PathBuf, process::Stdio, time::Duration};

use serde::Serialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    time::timeout,
};

#[derive(Serialize)]
pub struct LocalSkill {
    name: String,
    description: String,
}

#[tauri::command]
pub async fn list_local_codex_skills() -> Result<Vec<LocalSkill>, &'static str> {
    timeout(Duration::from_secs(5), read_skills())
        .await
        .ok()
        .flatten()
        .ok_or("Could not read the local Codex skill list.")
}

async fn read_skills() -> Option<Vec<LocalSkill>> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    // The webview cannot choose the executable, its arguments, or the directory.
    let executable = [
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
        PathBuf::from("/Applications/Codex.app/Contents/Resources/codex"),
        PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex"),
        home.join("Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex"),
        home.join(".local/bin/codex"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let mut child = Command::new(executable)
        .args(["app-server", "--listen", "stdio://"])
        .current_dir(&home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let mut stdout = BufReader::new(child.stdout.take()?.take(2 * 1024 * 1024));
    let init = json!({"id": 1, "method": "initialize", "params": {
        "clientInfo": { "name": "jimin-os-skill-catalog", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "experimentalApi": false }
    }});
    stdin.write_all(format!("{init}\n").as_bytes()).await.ok()?;
    read_result(&mut stdout, 1).await?;
    let list = json!({"id": 2, "method": "skills/list", "params": { "cwds": [home], "forceReload": true }});
    stdin
        .write_all(format!("{{\"method\":\"initialized\"}}\n{list}\n").as_bytes())
        .await
        .ok()?;
    let result = read_result(&mut stdout, 2).await?;
    drop(stdin);
    let _ = child.kill().await;
    enabled_skills(&result)
}

async fn read_result<R: AsyncBufRead + Unpin>(reader: &mut R, id: u64) -> Option<Value> {
    let mut line = String::new();
    while reader.read_line(&mut line).await.ok()? != 0 {
        if let Ok(mut value) = serde_json::from_str::<Value>(&line)
            && value["id"].as_u64() == Some(id)
        {
            return value.as_object_mut()?.remove("result");
        }
        line.clear();
    }
    None
}

fn enabled_skills(result: &Value) -> Option<Vec<LocalSkill>> {
    let mut skills = BTreeMap::new();
    for skill in result["data"]
        .as_array()?
        .iter()
        .filter_map(|group| group["skills"].as_array())
        .flatten()
        .filter(|skill| skill["enabled"].as_bool() == Some(true))
    {
        let Some(name) = skill["name"].as_str().filter(|name| {
            (1..=96).contains(&name.len())
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
        }) else {
            continue;
        };
        let description = skill["description"]
            .as_str()
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(260)
            .collect::<String>();
        skills
            .entry(name)
            .and_modify(|previous: &mut String| {
                // Same installed name may appear twice, with different scopes.
                if *previous != description {
                    previous.clear();
                }
            })
            .or_insert(description);
    }
    Some(
        skills
            .into_iter()
            .take(256)
            .map(|(name, description)| LocalSkill {
                name: name.to_owned(),
                description,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_well_formed_enabled_names() {
        let result = json!({"data":[{"skills":[
            {"name":"z-skill","enabled":true,"path":"private path"},
            {"name":"a:skill","enabled":true},
            {"name":"z-skill","enabled":true},
            {"name":"disabled","enabled":false},
            {"name":"unsafe\nname","enabled":true}
        ]}]});
        assert_eq!(
            enabled_skills(&result)
                .unwrap()
                .into_iter()
                .map(|skill| skill.name)
                .collect::<Vec<_>>(),
            ["a:skill", "z-skill"]
        );
    }
}
