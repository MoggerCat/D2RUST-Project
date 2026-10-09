//! Starting the client from a download: where the game files are, and
//! the crash log (`docs/PLAYTEST.md`).
//!
//! [`game_dir`] picks the install folder: `--game-dir`, then
//! `$D2_GAME_DIR`, then the folder of `d2-client.exe` or the current
//! folder when it holds `d2data.mpq`, so `d2-client.exe` copied into the
//! Diablo II folder starts on a double-click. [`install_crash_log`] makes
//! a panic (and a fatal error, [`write_error`]) write `d2rs-crash.log`
//! next to the exe, with the backtrace and the last [`note`]s.
// d2rs-own, unverified (a launcher convenience, not 1.14d behavior)

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The archive whose presence marks a Diablo II install folder.
pub const MARKER: &str = "d2data.mpq";

/// The crash log's file name, next to the exe.
pub const CRASH_LOG: &str = "d2rs-crash.log";

/// The notes kept for the crash log (oldest dropped first).
const NOTES: usize = 16;

static LAST: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The install folder, in order: `arg` (`--game-dir`), `env`
/// (`$D2_GAME_DIR`, when not empty), then the first of `candidates` that
/// holds [`MARKER`]. `None`: none found.
pub fn game_dir(
    arg: Option<&Path>,
    env: Option<PathBuf>,
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(a) = arg {
        return Some(a.to_path_buf());
    }
    if let Some(e) = env.filter(|e| !e.as_os_str().is_empty()) {
        return Some(e);
    }
    candidates.iter().find(|c| holds_marker(c)).cloned()
}

/// The folders searched when nothing names the install: the exe's
/// folder, then the current folder.
pub fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = exe_dir() {
        v.push(d);
    }
    if let Ok(d) = std::env::current_dir() {
        if !v.contains(&d) {
            v.push(d);
        }
    }
    v
}

/// Whether `dir` holds [`MARKER`] (any letter case: installs differ).
fn holds_marker(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(MARKER))
    })
}

/// The folder of the running exe.
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Where the crash log goes: next to the exe, else the current folder.
pub fn crash_log_path() -> PathBuf {
    exe_dir().unwrap_or_default().join(CRASH_LOG)
}

/// Remembers one line of "what the client was doing" for the crash log.
pub fn note(line: impl Into<String>) {
    if let Ok(mut l) = LAST.lock() {
        if l.len() == NOTES {
            l.remove(0);
        }
        l.push(line.into());
    }
}

/// The crash log's text: the headline, the notes, then `detail`.
pub fn report(headline: &str, notes: &[String], detail: &str) -> String {
    let mut s = format!(
        "d2rs {} ({} {})\n{headline}\n\nargs: {:?}\n\nlast state (oldest first):\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::args().collect::<Vec<_>>(),
    );
    if notes.is_empty() {
        s.push_str("  (none)\n");
    }
    for n in notes {
        s.push_str("  ");
        s.push_str(n);
        s.push('\n');
    }
    s.push('\n');
    s.push_str(detail);
    s.push('\n');
    s
}

fn notes() -> Vec<String> {
    LAST.lock().map(|l| l.clone()).unwrap_or_default()
}

fn write_log(text: &str) -> Option<PathBuf> {
    let path = crash_log_path();
    std::fs::write(&path, text).ok().map(|_| path)
}

/// Installs the panic hook: the default message on stderr as before, then
/// `d2rs-crash.log` with the panic, the notes and a backtrace.
pub fn install_crash_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let thread = std::thread::current();
        let headline = format!(
            "panic in thread {}: {info}",
            thread.name().unwrap_or("<unnamed>")
        );
        let backtrace = std::backtrace::Backtrace::force_capture();
        let text = report(&headline, &notes(), &format!("backtrace:\n{backtrace}"));
        if let Some(p) = write_log(&text) {
            eprintln!("crash log written to {}", p.display());
        }
    }));
}

/// Writes a fatal error (the `Err` `main` returns) to the crash log.
pub fn write_error(error: &anyhow::Error) {
    let text = report(&format!("error: {error:#}"), &notes(), "");
    if let Some(p) = write_log(&text) {
        eprintln!("error log written to {}", p.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("d2rs-launch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn game_dir_order() {
        let empty = scratch("empty");
        let install = scratch("install");
        std::fs::write(install.join("D2DATA.MPQ"), b"").unwrap();
        let c = [empty.clone(), install.clone()];
        let arg = Path::new("arg");
        assert_eq!(
            game_dir(Some(arg), Some("env".into()), &c),
            Some(PathBuf::from("arg"))
        );
        assert_eq!(game_dir(None, Some("env".into()), &c), Some("env".into()));
        // An empty variable is unset; the marker is matched in any case.
        assert_eq!(game_dir(None, Some("".into()), &c), Some(install.clone()));
        assert_eq!(game_dir(None, None, &c[..1]), None);
        let _ = std::fs::remove_dir_all(empty);
        let _ = std::fs::remove_dir_all(install);
    }

    #[test]
    fn report_holds_notes_and_detail() {
        let r = report("panic: x", &["play: new sorceress".into()], "backtrace:\nf");
        assert!(r.contains("panic: x"));
        assert!(r.contains("  play: new sorceress\n"));
        assert!(r.ends_with("backtrace:\nf\n"));
        assert!(report("e", &[], "").contains("(none)"));
    }

    #[test]
    fn notes_keep_the_last() {
        for i in 0..NOTES + 3 {
            note(format!("n{i}"));
        }
        let n = notes();
        assert_eq!(n.len(), NOTES);
        assert_eq!(
            n.last().map(String::as_str),
            Some(&*format!("n{}", NOTES + 2))
        );
    }
}
