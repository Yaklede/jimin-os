use keyring::{Entry, Error as KeyringError};

const SESSION_ACCOUNT: &str = "device-session";
const SESSION_SERVICE: &str = "io.jimin.os";
const MAX_SESSION_BYTES: usize = 8 * 1024;

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

fn session_entry() -> Result<Entry, String> {
    Entry::new(SESSION_SERVICE, SESSION_ACCOUNT)
        .map_err(|_| "The device secure store is unavailable.".to_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            read_device_session,
            save_device_session,
            clear_device_session
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("Jimin OS could not start: {error}");
        std::process::exit(1);
    }
}
