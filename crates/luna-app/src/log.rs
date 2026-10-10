//! A small log file for the release app, which has no console:
//! `%LOCALAPPDATA%\Luna\luna.log` (`~/Library/Application Support/Luna/luna.log`).
//! Startup failures and panics land here so they can be diagnosed.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Where the log lives; `None` when the platform has no local data folder.
pub fn log_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Luna").join("luna.log"))
}

/// Appends one line, creating the folder. Logging never fails the app.
pub fn append(path: &Path, message: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(f, "[{secs}] {message}");
    }
}

/// Writes `message` to the log and to stderr (visible in debug builds).
pub fn error(message: &str) {
    eprintln!("{message}");
    if let Some(p) = log_path() {
        append(&p, message);
    }
}

/// Sends panics to the log as well as stderr.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(p) = log_path() {
            append(&p, &format!("panic: {info}"));
        }
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines_and_creates_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Luna").join("luna.log");
        append(&path, "first");
        append(&path, "second");
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("] first") && lines[1].ends_with("] second"));
    }
}
