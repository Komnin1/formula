//! Keeping work safe if the program stops unexpectedly: the open drawing is copied to a
//! recovery file every few seconds while it has unsaved changes, and an error inside the
//! program writes a crash log before it closes. The next start offers the drawing back.

use std::path::PathBuf;
use std::time::SystemTime;

/// The folder for recent files, recovery copies and crash logs.
pub fn data_dir() -> Option<PathBuf> {
    // Tests (or a portable setup) can point everything at another folder.
    if let Some(d) = std::env::var_os("FORMULA_DATA_DIR") {
        return Some(PathBuf::from(d));
    }
    let name = crate::app::APP_NAME;
    let dir = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?).join(name)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support").join(name)
    } else {
        // XDG_CONFIG_HOME when it is set (a Flatpak sets it to the app's own folder, as the
        // sandbox can't write to ~/.config); otherwise ~/.config as before.
        let config = match std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()) {
            Some(c) => c,
            None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
        };
        config.join("formula")
    };
    Some(dir)
}

/// Each window keeps its own recovery copy, named by the program's process and the window.
fn recovery_file(win: u32) -> Option<PathBuf> {
    Some(data_dir()?.join(format!("Recovered drawing {}-{}.cdxml", std::process::id(), win)))
}

/// A running window touches this file every few seconds, so other windows know its recovery
/// copy is still in use and don't offer it back.
fn alive_file(pid: u32) -> Option<PathBuf> {
    Some(data_dir()?.join(format!("Window {}.alive", pid)))
}

/// "I'm still running" (called every few seconds by each window).
pub fn heartbeat() {
    if let Some(f) = alive_file(std::process::id()) {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, b"");
    }
}

/// The program is closing normally (its windows have cleared their own copies).
pub fn goodbye() {
    if let Some(f) = alive_file(std::process::id()) {
        let _ = std::fs::remove_file(f);
    }
}

/// Remove a recovery copy that has been restored or discarded.
pub fn remove(f: &std::path::Path) {
    let _ = std::fs::remove_file(f);
}

pub fn crash_dir() -> Option<PathBuf> {
    Some(data_dir()?.join("Crash logs"))
}

/// Copy the drawing to the recovery file (written next to it first, so a stop half-way
/// through never leaves a broken copy).
pub fn save(doc: &crate::doc::Doc, win: u32) -> Result<(), String> {
    let f = recovery_file(win).ok_or("no folder for the recovery copy")?;
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = f.with_extension("tmp.cdxml");
    crate::app::write_doc_file(doc, &tmp)?;
    std::fs::rename(&tmp, &f).map_err(|e| e.to_string())
}

/// The work is saved or the program closed normally: the copy isn't needed.
pub fn clear(win: u32) {
    if let Some(f) = recovery_file(win) {
        let _ = std::fs::remove_file(f);
    }
}

/// A recovery copy left by a window that didn't close normally (not one of the windows still
/// open), and when it was written. The newest one if there are several.
pub fn pending() -> Option<(PathBuf, SystemTime)> {
    let dir = data_dir()?;
    let me = std::process::id();
    let now = SystemTime::now();
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let rest = name.strip_prefix("Recovered drawing")?.strip_suffix(".cdxml")?;
            if rest.ends_with(".tmp") {
                return None;
            }
            // The older single copy has no number; "pid" or "pid-window" belongs to that program.
            if let Ok(pid) = rest.trim().split('-').next().unwrap_or("").parse::<u32>() {
                if pid == me {
                    return None;
                }
                let alive = alive_file(pid).and_then(|f| std::fs::metadata(f).ok()).and_then(|m| m.modified().ok());
                if alive.is_some_and(|t| now.duration_since(t).map(|d| d.as_secs() < 30).unwrap_or(true)) {
                    return None;
                }
            }
            Some((e.path(), e.metadata().ok()?.modified().ok()?))
        })
        .max_by_key(|x| x.1)
}

/// The newest crash log written after `since`.
pub fn crash_log_since(since: SystemTime) -> Option<PathBuf> {
    let dir = crash_dir()?;
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| *t >= since)
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// "3 minutes ago", "just now"…
pub fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    match s {
        0..=59 => "less than a minute ago".into(),
        60..=119 => "a minute ago".into(),
        120..=3599 => format!("{} minutes ago", s / 60),
        3600..=7199 => "an hour ago".into(),
        7200..=86399 => format!("{} hours ago", s / 3600),
        _ => format!("{} days ago", s / 86400),
    }
}

/// On an error inside the program, write what happened and where to a crash log, then let
/// the usual handling go on.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = crash_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown error".into());
            let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let text = format!(
                "{} {} stopped because of an internal error.\n\nError: {}\nWhere: {}\nSystem: {} {}\n\nDetails:\n{}\n",
                crate::app::APP_NAME,
                crate::app::APP_VERSION,
                msg,
                place,
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::backtrace::Backtrace::force_capture()
            );
            let _ = std::fs::write(dir.join(format!("crash-{}.txt", secs)), text);
        }
        default(info);
    }));
}
