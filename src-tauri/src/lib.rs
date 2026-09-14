mod avd_info;
mod emulator;
mod sdk;

use std::path::PathBuf;
use tauri::Manager;

/// Resolve the SDK root or return a message the UI can show verbatim.
fn require_sdk(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    sdk::resolve(&dir).ok_or_else(|| {
        "No Android SDK found. Set ANDROID_HOME, or enter the SDK path in Settings.".to_string()
    })
}

#[tauri::command]
fn sdk_path(app: tauri::AppHandle) -> Option<String> {
    require_sdk(&app).ok().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn set_sdk_path(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let trimmed = path.trim();
    let mut candidate = PathBuf::from(trimmed);
    if trimmed.is_empty() {
        return Err("Enter a path to the Android SDK folder.".into());
    }
    // Accept the SDK root, or a path pointing straight at the emulator folder.
    if !sdk::is_valid_sdk(&candidate) {
        if let Some(parent) = candidate.parent() {
            if sdk::is_valid_sdk(parent) {
                candidate = parent.to_path_buf();
            }
        }
    }
    if !sdk::is_valid_sdk(&candidate) {
        return Err(format!(
            "No emulator binary under {}. Pick the SDK folder that contains 'emulator'.",
            candidate.display()
        ));
    }
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    sdk::save_sdk(&dir, &candidate)?;
    Ok(candidate.to_string_lossy().into_owned())
}

/// Shown in the About dialog.
const CREATOR: &str = "rajumark";

#[tauri::command]
fn about(app: tauri::AppHandle) -> serde_json::Value {
    let info = app.package_info();
    serde_json::json!({
        "name": info.name,
        "version": info.version.to_string(),
        "creator": CREATOR,
    })
}

#[tauri::command]
fn list_avds(app: tauri::AppHandle) -> Result<Vec<emulator::Avd>, String> {
    emulator::list(&require_sdk(&app)?)
}

#[tauri::command]
fn start_avd(app: tauri::AppHandle, name: String) -> Result<(), String> {
    emulator::start(&require_sdk(&app)?, &name)
}

#[tauri::command]
fn stop_avd(app: tauri::AppHandle, serial: String) -> Result<(), String> {
    emulator::stop(&require_sdk(&app)?, &serial)
}

#[tauri::command]
fn force_kill_avd(app: tauri::AppHandle, name: String, serial: Option<String>) -> Result<(), String> {
    emulator::force_kill(&require_sdk(&app)?, &name, serial.as_deref())
}

#[tauri::command]
fn open_avd_folder(path: String) -> Result<(), String> {
    emulator::open_folder(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            about,
            sdk_path,
            set_sdk_path,
            list_avds,
            start_avd,
            stop_avd,
            force_kill_avd,
            open_avd_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running Emulator Studio");
}
