use std::path::{Path, PathBuf};

/// Name of a binary with the platform-appropriate extension.
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// A directory counts as an SDK root if it holds the emulator binary.
pub fn is_valid_sdk(root: &Path) -> bool {
    root.join("emulator").join(exe("emulator")).is_file()
}

/// Where the saved SDK override lives.
pub fn settings_file(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("settings.json")
}

fn saved_sdk(app_config_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(settings_file(app_config_dir)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let p = v.get("sdk_path")?.as_str()?;
    Some(PathBuf::from(p))
}

pub fn save_sdk(app_config_dir: &Path, path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(app_config_dir).map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "sdk_path": path.to_string_lossy() });
    std::fs::write(settings_file(app_config_dir), body.to_string()).map_err(|e| e.to_string())
}

/// Default install locations per platform.
fn default_locations() -> Vec<PathBuf> {
    let Some(h) = home() else { return Vec::new() };
    if cfg!(target_os = "macos") {
        vec![h.join("Library/Android/sdk")]
    } else if cfg!(windows) {
        let mut v = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            v.push(PathBuf::from(local).join("Android").join("Sdk"));
        }
        v.push(h.join("AppData").join("Local").join("Android").join("Sdk"));
        v
    } else {
        vec![h.join("Android/Sdk"), h.join("Android/sdk")]
    }
}

/// Resolve the SDK root: saved override first, then env vars, then defaults.
pub fn resolve(app_config_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = saved_sdk(app_config_dir) {
        candidates.push(p);
    }
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(v) = std::env::var_os(key) {
            if !v.is_empty() {
                candidates.push(PathBuf::from(v));
            }
        }
    }
    candidates.extend(default_locations());
    candidates.into_iter().find(|p| is_valid_sdk(p))
}

pub fn emulator_bin(sdk: &Path) -> PathBuf {
    sdk.join("emulator").join(exe("emulator"))
}

pub fn adb_bin(sdk: &Path) -> PathBuf {
    sdk.join("platform-tools").join(exe("adb"))
}
