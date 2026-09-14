use crate::avd_info;
use crate::sdk;
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

#[derive(Serialize)]
pub struct Avd {
    pub name: String,
    pub running: bool,
    pub serial: Option<String>,
    /// True once the device reports `sys.boot_completed`.
    pub booted: bool,
    /// Friendly device name, e.g. "Pixel 10".
    pub display: Option<String>,
    /// Marketing Android version, e.g. "Android 17".
    pub android: Option<String>,
    /// API level, e.g. "37.0".
    pub api: Option<String>,
    /// The `<name>.avd` directory, for "Open folder".
    pub path: Option<String>,
    /// Folder creation time (ms since epoch), newest-first ordering.
    pub created: Option<u64>,
}

/// A short-lived command that never flashes a console window on Windows.
fn quiet(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run(program: &Path, args: &[&str]) -> Result<String, String> {
    let out = quiet(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {}: {e}", program.display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("{} exited with {}", program.display(), out.status)
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Every AVD defined on this machine.
fn defined_avds(sdk: &Path) -> Result<Vec<String>, String> {
    let out = run(&sdk::emulator_bin(sdk), &["-list-avds"])?;
    Ok(out
        .lines()
        .map(str::trim)
        // The emulator occasionally prefixes warnings on stdout; AVD names have no spaces.
        .filter(|l| !l.is_empty() && !l.contains(' '))
        .map(|s| s.to_string())
        .collect())
}

/// Serial numbers of currently attached emulator instances.
fn running_serials(adb: &Path) -> Vec<String> {
    let Ok(out) = run(adb, &["devices"]) else {
        return Vec::new();
    };
    out.lines()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?;
            let state = parts.next()?;
            if serial.starts_with("emulator-") && state != "offline" {
                Some(serial.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Ask a running emulator which AVD it is.
fn avd_name_of(adb: &Path, serial: &str) -> Option<String> {
    let out = run(adb, &["-s", serial, "emu", "avd", "name"]).ok()?;
    out.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && *l != "OK")
        .map(|s| s.to_string())
}

/// A booted device knows its own release; trust that over the on-disk config.
fn release_of(adb: &Path, serial: &str) -> Option<String> {
    let out = run(adb, &["-s", serial, "shell", "getprop", "ro.build.version.release"]).ok()?;
    let v = out.trim();
    if v.is_empty() {
        None
    } else {
        Some(format!("Android {v}"))
    }
}

fn is_booted(adb: &Path, serial: &str) -> bool {
    run(adb, &["-s", serial, "shell", "getprop", "sys.boot_completed"])
        .map(|o| o.trim() == "1")
        .unwrap_or(false)
}

pub fn list(sdk: &Path) -> Result<Vec<Avd>, String> {
    let adb = sdk::adb_bin(sdk);
    let live: Vec<(String, Option<String>)> = running_serials(&adb)
        .into_iter()
        .map(|s| {
            let name = avd_name_of(&adb, &s);
            (s, name)
        })
        .collect();

    let mut avds: Vec<Avd> = defined_avds(sdk)?
        .into_iter()
        .map(|name| {
            let serial = live
                .iter()
                .find(|(_, n)| n.as_deref() == Some(name.as_str()))
                .map(|(s, _)| s.clone());
            let booted = serial.as_deref().map(|s| is_booted(&adb, s)).unwrap_or(false);
            let info = avd_info::lookup(sdk, &name);
            let android = if booted {
                serial
                    .as_deref()
                    .and_then(|s| release_of(&adb, s))
                    .or(info.android)
            } else {
                info.android
            };
            Avd {
                running: serial.is_some(),
                serial,
                booted,
                display: info.display,
                android,
                api: info.api,
                path: info.path,
                created: info.created,
                name,
            }
        })
        .collect();

    // Emulators running from an AVD we could not name still deserve a row.
    for (serial, name) in live {
        let known = avds.iter().any(|a| a.serial.as_deref() == Some(serial.as_str()));
        if !known {
            let booted = is_booted(&adb, &serial);
            let android = if booted { release_of(&adb, &serial) } else { None };
            let info = name
                .as_deref()
                .map(|n| avd_info::lookup(sdk, n));
            avds.push(Avd {
                name: name.unwrap_or_else(|| serial.clone()),
                running: true,
                serial: Some(serial),
                booted,
                display: info.as_ref().and_then(|i| i.display.clone()),
                android: android.or_else(|| info.as_ref().and_then(|i| i.android.clone())),
                api: info.as_ref().and_then(|i| i.api.clone()),
                path: info.as_ref().and_then(|i| i.path.clone()),
                created: info.as_ref().and_then(|i| i.created),
            });
        }
    }

    // Newest AVD first; ties (and undated rows) fall back to name order.
    avds.sort_by(|a, b| {
        b.created
            .cmp(&a.created)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(avds)
}

/// Launch an emulator fully detached, so it outlives this app.
pub fn start(sdk: &Path, name: &str) -> Result<(), String> {
    let bin = sdk::emulator_bin(sdk);
    let mut cmd = Command::new(&bin);
    cmd.arg("-avd")
        .arg(name)
        .env("ANDROID_SDK_ROOT", sdk)
        .env("ANDROID_HOME", sdk)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // New session: the emulator is no longer in our process group, so it
        // survives this app quitting and never receives our SIGHUP.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("could not launch {}: {e}", bin.display()))
}

pub fn stop(sdk: &Path, serial: &str) -> Result<(), String> {
    run(&sdk::adb_bin(sdk), &["-s", serial, "emu", "kill"]).map(|_| ())
}

/// Regex-escape the characters an AVD name may legally contain.
#[cfg(windows)]
fn escape(name: &str) -> String {
    name.chars()
        .flat_map(|c| {
            let esc = matches!(c, '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\');
            esc.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect()
}

/// SIGKILL the qemu process backing an AVD, for emulators that ignore
/// `adb emu kill` (typically ones wedged mid-boot).
#[cfg(unix)]
fn kill_process(name: &str) -> Result<u32, String> {
    let out = Command::new("ps")
        .args(["ax", "-o", "pid=,args="])
        .output()
        .map_err(|e| format!("could not list processes: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);

    // The emulator always re-execs into qemu with `-avd <name>` in its argv.
    let needle = format!("-avd {name}");
    let mut killed = 0;
    for line in text.lines() {
        let line = line.trim_start();
        let Some((pid, args)) = line.split_once(char::is_whitespace) else { continue };
        let Ok(pid) = pid.parse::<i32>() else { continue };
        let matches_avd = match args.find(&needle) {
            // Guard against "Pixel_1" matching "Pixel_10".
            Some(i) => args[i + needle.len()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace),
            None => false,
        };
        if matches_avd && args.contains("qemu-system") {
            unsafe { libc::kill(pid, libc::SIGKILL) };
            killed += 1;
        }
    }
    Ok(killed)
}

#[cfg(windows)]
fn kill_process(name: &str) -> Result<u32, String> {
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name LIKE 'qemu-system%'\" | \
         Where-Object {{ $_.CommandLine -match '-avd\\s+{}(\\s|$)' }} | \
         ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}",
        escape(name)
    );
    let mut cmd = quiet(Path::new("powershell"));
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    let out = cmd.output().map_err(|e| format!("could not run powershell: {e}"))?;
    if out.status.success() {
        Ok(1)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Ask nicely first, then kill the process outright.
pub fn force_kill(sdk: &Path, name: &str, serial: Option<&str>) -> Result<(), String> {
    if let Some(serial) = serial {
        let _ = stop(sdk, serial);
    }
    match kill_process(name)? {
        0 => Err(format!("No running emulator process found for {name}.")),
        _ => Ok(()),
    }
}

/// Reveal a folder in Finder / Explorer / the Linux file manager.
pub fn open_folder(path: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("{path} no longer exists."));
    }
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut cmd = quiet(Path::new(program));
    cmd.arg(dir);
    // explorer.exe reports a non-zero exit even on success, so only a spawn
    // failure counts as an error here.
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open {path}: {e}"))
}
