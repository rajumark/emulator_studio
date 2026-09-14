//! Reads AVD metadata straight off disk, so a stopped emulator can still
//! report which Android version it runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct AvdInfo {
    /// Friendly device name, e.g. "Pixel 10".
    pub display: Option<String>,
    /// Marketing version, e.g. "Android 17".
    pub android: Option<String>,
    /// API level as written by the SDK, e.g. "37.0".
    pub api: Option<String>,
    /// The `<name>.avd` directory on disk.
    pub path: Option<String>,
    /// When that folder was created, in ms since the epoch.
    pub created: Option<u64>,
}

/// Parse a `key=value` properties file, ignoring comments.
fn props(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

/// Folder creation time, falling back to mtime on filesystems that don't
/// record a birth time (common on Linux).
fn created_ms(dir: &Path) -> Option<u64> {
    let meta = std::fs::metadata(dir).ok()?;
    let time = meta.created().or_else(|_| meta.modified()).ok()?;
    let since = time.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(since.as_millis() as u64)
}

fn avd_home() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("ANDROID_AVD_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    if let Some(v) = std::env::var_os("ANDROID_SDK_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v).join(".android").join("avd"));
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".android").join("avd"))
}

/// Marketing version for an API level when the matching platform isn't installed.
fn fallback_version(major: u32) -> Option<&'static str> {
    Some(match major {
        37 => "17",
        36 => "16",
        35 => "15",
        34 => "14",
        33 => "13",
        31 | 32 => "12",
        30 => "11",
        29 => "10",
        28 => "9",
        27 => "8.1",
        26 => "8.0",
        25 => "7.1",
        24 => "7.0",
        23 => "6.0",
        21 | 22 => "5.1",
        19 | 20 => "4.4",
        _ => return None,
    })
}

/// Turn an API id like "37.0" into "Android 17", preferring the SDK's own
/// `platforms/<id>/source.properties` over the built-in table.
fn version_for(sdk: &Path, api_id: &str) -> Option<String> {
    let installed = props(
        &sdk.join("platforms")
            .join(format!("android-{api_id}"))
            .join("source.properties"),
    );
    if let Some(v) = installed.get("Platform.Version") {
        if !v.is_empty() {
            return Some(format!("Android {v}"));
        }
    }
    let major: u32 = api_id.split('.').next()?.parse().ok()?;
    fallback_version(major).map(|v| format!("Android {v}"))
}

/// Metadata for one AVD, best-effort — any missing field is simply `None`.
pub fn lookup(sdk: &Path, name: &str) -> AvdInfo {
    let mut info =
        AvdInfo { display: None, android: None, api: None, path: None, created: None };
    let Some(home) = avd_home() else { return info };

    // <name>.ini points at the AVD folder and names its target platform.
    let ini = props(&home.join(format!("{name}.ini")));
    let dir = ini
        .get("path")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(format!("{name}.avd")));
    let config = props(&dir.join("config.ini"));
    if dir.is_dir() {
        info.path = Some(dir.to_string_lossy().into_owned());
        info.created = created_ms(&dir);
    }

    info.display = config
        .get("avd.ini.displayname")
        .filter(|s| !s.is_empty())
        .cloned();

    // Prefer the target in the .ini; fall back to the system-image path.
    let api_id = ini
        .get("target")
        .and_then(|t| t.strip_prefix("android-"))
        .map(str::to_string)
        .or_else(|| {
            let sysdir = config.get("image.sysdir.1")?;
            sysdir
                .split(['/', '\\'])
                .find_map(|seg| seg.strip_prefix("android-"))
                .map(str::to_string)
        });

    if let Some(id) = api_id {
        info.android = version_for(sdk, &id);
        // The SDK writes "37.0" but only a real minor ("36.1") is worth showing.
        info.api = Some(id.strip_suffix(".0").unwrap_or(&id).to_string());
    }
    info
}
