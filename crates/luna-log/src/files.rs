//! The logs folder: file names, opening, retention and rollover.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use jiff::civil::DateTime;

use crate::{APP_LOG_MAX_BYTES, KEEP_RUNS, MAX_AGE};

const APP_LOG: &str = "luna.log";
const APP_LOG_OLD: &str = "luna.old.log";
const MAX_NAME_CHARS: usize = 80;

/// `YYYY-MM-DD_HH-MM-SS <source>.log`, with the source made safe for a file name.
pub fn run_file_name(started: &DateTime, source: &str) -> String {
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02} {}.log",
        started.year(),
        started.month(),
        started.day(),
        started.hour(),
        started.minute(),
        started.second(),
        sanitise(source)
    )
}

/// Replaces characters no file system accepts, trims trailing dots and spaces,
/// and caps the length. An empty result becomes `source`.
pub fn sanitise(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(MAX_NAME_CHARS)
        .collect();
    let trimmed = safe.trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        "source".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Keeps the [`KEEP_RUNS`] newest run logs and deletes any older than
/// [`MAX_AGE`]. `luna.log` and `luna.old.log` stay.
pub fn prune(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut runs: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".log") && name != APP_LOG && name != APP_LOG_OLD
        })
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    // Newest first; the name breaks ties, and names start with the date.
    runs.sort_by(|a, b| b.cmp(a));
    for (i, (modified, path)) in runs.iter().enumerate() {
        let too_old = now.duration_since(*modified).is_ok_and(|age| age > MAX_AGE);
        if too_old || i >= KEEP_RUNS {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Opens `luna.log` for appending, rolling it over first when it is too big.
pub(crate) fn open_app_log(dir: &Path) -> Option<File> {
    let path = dir.join(APP_LOG);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > APP_LOG_MAX_BYTES) {
        let _ = std::fs::rename(&path, dir.join(APP_LOG_OLD));
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// `luna.log` as it is written: rolls over to `luna.old.log` whenever the next
/// write would take it past `limit` bytes.
pub(crate) struct AppLog {
    dir: PathBuf,
    limit: u64,
    written: u64,
    file: Option<BufWriter<File>>,
}

impl AppLog {
    /// Opens `luna.log` (rolling it over first when it is already too big).
    pub(crate) fn open(dir: &Path, limit: u64) -> Option<AppLog> {
        let file = open_app_log(dir)?;
        let written = file.metadata().map_or(0, |m| m.len());
        Some(AppLog {
            dir: dir.to_path_buf(),
            limit,
            written,
            file: Some(BufWriter::new(file)),
        })
    }

    fn roll_over(&mut self) {
        if let Some(mut f) = self.file.take() {
            let _ = f.flush();
        }
        let path = self.dir.join(APP_LOG);
        let _ = std::fs::rename(&path, self.dir.join(APP_LOG_OLD));
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(BufWriter::new);
        // Counted from zero even when the rename failed, so a stuck file is
        // retried once per `limit` bytes rather than on every line.
        self.written = 0;
    }
}

impl Write for AppLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.written > 0 && self.written + buf.len() as u64 > self.limit {
            self.roll_over();
        }
        let Some(f) = self.file.as_mut() else {
            return Err(std::io::Error::other("luna.log is not open"));
        };
        let n = f.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.as_mut().map_or(Ok(()), |f| f.flush())
    }
}

/// Creates a new run file; a name already taken gets ` (2)`, ` (3)`, ….
pub(crate) fn open_run_file(dir: &Path, started: &DateTime, source: &str) -> Option<File> {
    let name = run_file_name(started, source);
    let stem = name.strip_suffix(".log").unwrap_or(&name);
    for n in 1..=1000 {
        let candidate = if n == 1 {
            name.clone()
        } else {
            format!("{stem} ({n}).log")
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(candidate))
        {
            Ok(f) => return Some(f),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;
    use std::time::Duration;

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn names_are_dated_and_sanitised() {
        let started = date(2026, 10, 25).at(14, 3, 12, 0);
        assert_eq!(
            run_file_name(&started, "A001:Mini?LF"),
            "2026-10-25_14-03-12 A001_Mini_LF.log"
        );
        assert_eq!(sanitise(""), "source");
        assert_eq!(sanitise("..."), "source");
        assert_eq!(sanitise("Card A. "), "Card A");
        assert_eq!(sanitise("a\tb<>\"/\\|*c"), "a_b_______c");
        assert_eq!(sanitise(&"x".repeat(200)).chars().count(), 80);
    }

    #[test]
    fn prune_keeps_newest_50_and_drops_old() {
        let dir = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        for i in 0..55 {
            let f = File::create(dir.path().join(format!("2026-10-{i:02} run.log"))).unwrap();
            f.set_modified(now - Duration::from_secs(i * 60)).unwrap();
        }
        let old = dir.path().join("2026-08-01 old.log");
        File::create(&old)
            .unwrap()
            .set_modified(now - Duration::from_secs(61 * 24 * 3600))
            .unwrap();
        File::create(dir.path().join("luna.log"))
            .unwrap()
            .set_modified(now - Duration::from_secs(90 * 24 * 3600))
            .unwrap();

        prune(dir.path(), now);

        let left = names(dir.path());
        assert!(left.contains(&"luna.log".to_string()));
        assert!(!old.exists());
        let runs: Vec<&String> = left.iter().filter(|n| *n != "luna.log").collect();
        assert_eq!(runs.len(), KEEP_RUNS);
        // The five oldest of the recent ones went; the newest stayed.
        assert!(runs.iter().any(|n| *n == "2026-10-00 run.log"));
        assert!(!runs.iter().any(|n| *n == "2026-10-54 run.log"));
    }

    #[test]
    fn app_log_rolls_over_at_5mb() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("luna.log");
        std::fs::write(&log, vec![b'x'; APP_LOG_MAX_BYTES as usize + 1]).unwrap();

        let f = open_app_log(dir.path());

        assert!(f.is_some());
        let old = dir.path().join("luna.old.log");
        assert_eq!(std::fs::metadata(old).unwrap().len(), APP_LOG_MAX_BYTES + 1);
        assert_eq!(std::fs::metadata(&log).unwrap().len(), 0);
    }

    #[test]
    fn app_log_rolls_over_while_writing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("luna.log"), "old\n").unwrap();
        let mut log = AppLog::open(dir.path(), 20).unwrap();
        for line in ["first line\n", "second line\n", "third line\n"] {
            log.write_all(line.as_bytes()).unwrap();
            log.flush().unwrap();
        }
        let read = |name| std::fs::read_to_string(dir.path().join(name)).unwrap();
        assert_eq!(read("luna.old.log"), "second line\n");
        assert_eq!(read("luna.log"), "third line\n");
    }

    #[test]
    fn small_app_log_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("luna.log"), "line\n").unwrap();
        assert!(open_app_log(dir.path()).is_some());
        assert_eq!(names(dir.path()), ["luna.log"]);
    }

    #[test]
    fn taken_run_name_gets_a_number() {
        let dir = tempfile::tempdir().unwrap();
        let started = date(2026, 10, 25).at(14, 3, 12, 0);
        assert!(open_run_file(dir.path(), &started, "Card A").is_some());
        assert!(open_run_file(dir.path(), &started, "Card A").is_some());
        assert!(open_run_file(dir.path(), &started, "Card A").is_some());
        assert_eq!(
            names(dir.path()),
            [
                "2026-10-25_14-03-12 Card A (2).log",
                "2026-10-25_14-03-12 Card A (3).log",
                "2026-10-25_14-03-12 Card A.log",
            ]
        );
    }

    #[test]
    fn unwritable_dir_opens_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let not_a_dir = dir.path().join("file");
        std::fs::write(&not_a_dir, "").unwrap();
        let started = date(2026, 10, 25).at(14, 3, 12, 0);
        assert!(open_run_file(&not_a_dir, &started, "Card A").is_none());
        assert!(open_app_log(&not_a_dir).is_none());
        prune(&not_a_dir, SystemTime::now());
    }
}
