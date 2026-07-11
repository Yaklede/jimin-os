use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "android")]
use tauri::Manager;
use uuid::Uuid;

const SESSION_ACCOUNT: &str = "device-session";
const INSTALLATION_ACCOUNT: &str = "device-installation";
const SESSION_SERVICE: &str = "io.jimin.os";
const MAX_SESSION_BYTES: usize = 8 * 1024;

#[cfg(target_os = "android")]
struct AndroidQrScanner<R: tauri::Runtime>(tauri::plugin::PluginHandle<R>);

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QrScanResponse {
    content: Option<String>,
}

#[tauri::command]
fn read_device_session() -> Result<Option<String>, String> {
    let entry = session_entry()?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err("The device secure store could not be read.".to_owned()),
    }
}

#[tauri::command]
fn save_device_session(value: &str) -> Result<(), String> {
    if value.len() > MAX_SESSION_BYTES {
        return Err("The device session is too large to store safely.".to_owned());
    }

    session_entry()?
        .set_password(value)
        .map_err(|_| "The device secure store could not be updated.".to_owned())
}

#[tauri::command]
fn clear_device_session() -> Result<(), String> {
    let entry = session_entry()?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(_) => Err("The device secure store could not be cleared.".to_owned()),
    }
}

#[tauri::command]
fn read_or_create_installation_id() -> Result<String, String> {
    let entry = installation_entry()?;
    match entry.get_password() {
        Ok(value) if valid_installation_id(&value) => Ok(value),
        Err(KeyringError::NoEntry) => {
            let installation_id = Uuid::now_v7().to_string();
            entry
                .set_password(&installation_id)
                .map_err(|_| "The device identity could not be saved safely.".to_owned())?;
            Ok(installation_id)
        }
        Ok(_) | Err(_) => Err("The device identity could not be read safely.".to_owned()),
    }
}

#[tauri::command]
async fn scan_qr_code(app: tauri::AppHandle) -> Result<QrScanResponse, String> {
    #[cfg(target_os = "android")]
    {
        return app
            .state::<AndroidQrScanner<tauri::Wry>>()
            .inner()
            .0
            .run_mobile_plugin_async::<QrScanResponse>("scan", ())
            .await
            .map_err(|_| "The QR scanner could not be opened.".to_owned());
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("QR scanning is only available on Android in this build.".to_owned())
    }
}

fn session_entry() -> Result<Entry, String> {
    #[cfg(target_os = "android")]
    configure_platform_store()?;
    Entry::new(SESSION_SERVICE, SESSION_ACCOUNT)
        .map_err(|_| "The device secure store is unavailable.".to_owned())
}

fn installation_entry() -> Result<Entry, String> {
    #[cfg(target_os = "android")]
    configure_platform_store()?;
    Entry::new(SESSION_SERVICE, INSTALLATION_ACCOUNT)
        .map_err(|_| "The device secure store is unavailable.".to_owned())
}

#[cfg(target_os = "android")]
fn configure_platform_store() -> Result<(), String> {
    if keyring_core::get_default_store().is_none() {
        let store = android_native_keyring_store::Store::new()
            .map_err(|_| "The device secure store is unavailable.".to_owned())?;
        keyring_core::set_default_store(store);
    }
    Ok(())
}

fn valid_installation_id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| id.get_version_num() == 7)
}

fn init_qr_scanner<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("jimin-qr-scanner")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let scanner =
                    _api.register_android_plugin("io.jimin.os", "JiminQrScannerPlugin")?;
                _app.manage(AndroidQrScanner(scanner));
            }
            Ok(())
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(init_qr_scanner())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            read_device_session,
            save_device_session,
            clear_device_session,
            read_or_create_installation_id,
            scan_qr_code
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("Jimin OS could not start: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::valid_installation_id;

    #[test]
    fn accepts_only_version_seven_installation_ids() {
        assert!(valid_installation_id(
            "019f68cb-9400-7000-8000-000000000000"
        ));
        assert!(!valid_installation_id(
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(!valid_installation_id("not-an-installation-id"));
    }
}
