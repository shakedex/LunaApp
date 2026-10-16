//! The contract between the window and whatever actually runs a report.
//!
//! The UI only knows [`RunBackend`]: it hands over a source folder and the
//! settings, and receives [`RunEvent`]s on a worker thread until the run ends
//! with exactly one of `Finished`, `Failed` or `Cancelled`.
//!
//! [`FakeBackend`] simulates a run so every screen can be exercised before the
//! real pipeline in `luna-core` is wired in. It writes nothing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use luna_core::run::RunRequest;
use luna_core::settings::Settings;

/// Starts report runs. Implementations must be cheap to call from the UI
/// thread: all work happens elsewhere and is reported through `events`.
pub trait RunBackend: Send + Sync {
    fn start(
        &self,
        request: RunRequest,
        events: Box<dyn Fn(RunEvent) + Send>,
    ) -> Box<dyn RunControl>;
}

/// A handle on a run in progress.
pub trait RunControl {
    /// Asks the run to stop. The run answers with `RunEvent::Cancelled` and
    /// writes no report (or with `Finished` if it was already done).
    fn cancel(&self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    /// Number of files found in the source folder.
    FilesFound(usize),
    /// Number of clips assembled from those files.
    ClipsTotal(usize),
    ClipDone {
        done: usize,
        total: usize,
        problems: usize,
    },
    Finished(RunOutcome),
    /// The run as a whole failed (source unreadable, output not writable, disk full).
    Failed(String),
    Cancelled,
}

/// What a finished run wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub pdf: PathBuf,
    pub html: PathBuf,
    pub csv: PathBuf,
    /// The run's report folder (contains the three files).
    pub folder: PathBuf,
    /// One line about problems, e.g. "2 clips couldn't be decoded". Empty when
    /// there were none.
    pub summary: String,
}

/// How a [`FakeBackend`] run ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeEnding {
    Finish,
    Fail,
}

/// Simulates a run on a thread: a short scan, a dozen clips, two problems.
#[derive(Debug, Clone)]
pub struct FakeBackend {
    /// Delay between simulated steps.
    pub step: Duration,
    pub clips: usize,
    pub ending: FakeEnding,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self {
            step: Duration::from_millis(350),
            clips: 12,
            ending: FakeEnding::Finish,
        }
    }
}

impl FakeBackend {
    /// `LUNA_FAKE_OUTCOME=fail` makes every run fail, to show the error screen;
    /// `LUNA_FAKE_STEP_MS` sets the delay between simulated steps.
    pub fn from_env() -> Self {
        let ending = match std::env::var("LUNA_FAKE_OUTCOME").as_deref() {
            Ok("fail") => FakeEnding::Fail,
            _ => FakeEnding::Finish,
        };
        let default = Self::default();
        let step = std::env::var("LUNA_FAKE_STEP_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_millis)
            .unwrap_or(default.step);
        Self {
            ending,
            step,
            ..default
        }
    }
}

struct FakeControl(Arc<AtomicBool>);

impl RunControl for FakeControl {
    fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl RunBackend for FakeBackend {
    fn start(
        &self,
        request: RunRequest,
        events: Box<dyn Fn(RunEvent) + Send>,
    ) -> Box<dyn RunControl> {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let this = self.clone();
        thread::spawn(move || {
            let source = request.sources.first().cloned().unwrap_or_default();
            this.simulate(&source, &request.settings, &flag, &*events)
        });
        Box::new(FakeControl(cancel))
    }
}

impl FakeBackend {
    fn simulate(
        &self,
        source: &Path,
        settings: &Settings,
        cancel: &AtomicBool,
        emit: &dyn Fn(RunEvent),
    ) {
        // Sleeps one step; true when the run should stop.
        let wait = || {
            thread::sleep(self.step);
            cancel.load(Ordering::SeqCst)
        };
        if wait() {
            return emit(RunEvent::Cancelled);
        }
        emit(RunEvent::FilesFound(count_files(source, 10_000)));
        if wait() {
            return emit(RunEvent::Cancelled);
        }
        let total = self.clips;
        emit(RunEvent::ClipsTotal(total));
        let mut problems = 0;
        for done in 1..=total {
            if wait() {
                return emit(RunEvent::Cancelled);
            }
            if done == 4 || done == 9 {
                problems += 1;
            }
            emit(RunEvent::ClipDone {
                done,
                total,
                problems,
            });
            if self.ending == FakeEnding::Fail && done == total / 2 {
                return emit(RunEvent::Failed(format!(
                    "Luna can't write to {}. Check that the drive is connected and has free space.",
                    settings.output_dir.display()
                )));
            }
        }
        let folder = settings.output_dir.join(report_folder_name(source));
        emit(RunEvent::Finished(RunOutcome {
            pdf: folder.join("camera-report.pdf"),
            html: folder.join("camera-report.html"),
            csv: folder.join("camera-report.csv"),
            folder,
            summary: match problems {
                0 => String::new(),
                1 => "1 clip couldn't be decoded".into(),
                n => format!("{n} clips couldn't be decoded"),
            },
        }));
    }
}

fn report_folder_name(source: &Path) -> String {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Source".into());
    format!("{name} report")
}

/// Counts regular files under `dir`, stopping at `limit`.
fn count_files(dir: &Path, limit: usize) -> usize {
    let mut stack = vec![dir.to_path_buf()];
    let mut n = 0;
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => n += 1,
                _ => {}
            }
            if n >= limit {
                return n;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn fast(ending: FakeEnding) -> FakeBackend {
        FakeBackend {
            step: Duration::from_millis(1),
            clips: 5,
            ending,
        }
    }

    fn collect(rx: &mpsc::Receiver<RunEvent>) -> Vec<RunEvent> {
        let mut out = vec![];
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(5)) {
            let last = matches!(
                e,
                RunEvent::Finished(_) | RunEvent::Failed(_) | RunEvent::Cancelled
            );
            out.push(e);
            if last {
                break;
            }
        }
        out
    }

    fn start(b: &FakeBackend) -> (Box<dyn RunControl>, mpsc::Receiver<RunEvent>) {
        let (tx, rx) = mpsc::channel();
        let settings = Settings {
            output_dir: PathBuf::from("out"),
            ..Settings::default()
        };
        let source = std::env::temp_dir().join(format!("luna-app-fake-{}", std::process::id()));
        std::fs::create_dir_all(&source).unwrap();
        let ctl = b.start(
            RunRequest::folder(source, settings),
            Box::new(move |e| {
                let _ = tx.send(e);
            }),
        );
        (ctl, rx)
    }

    #[test]
    fn a_fake_run_reports_progress_then_finishes_with_a_problem_line() {
        let (_ctl, rx) = start(&fast(FakeEnding::Finish));
        let events = collect(&rx);
        assert!(matches!(events[0], RunEvent::FilesFound(_)));
        assert_eq!(events[1], RunEvent::ClipsTotal(5));
        assert_eq!(
            events[5],
            RunEvent::ClipDone {
                done: 4,
                total: 5,
                problems: 1
            }
        );
        let RunEvent::Finished(outcome) = events.last().unwrap() else {
            panic!("expected Finished, got {events:?}");
        };
        assert_eq!(outcome.summary, "1 clip couldn't be decoded");
        assert!(outcome.pdf.starts_with(&outcome.folder));
        assert!(outcome.folder.starts_with("out"));
    }

    #[test]
    fn a_fake_run_can_fail() {
        let (_ctl, rx) = start(&fast(FakeEnding::Fail));
        assert!(matches!(collect(&rx).last(), Some(RunEvent::Failed(_))));
    }

    #[test]
    fn cancelling_a_fake_run_ends_with_cancelled() {
        let mut b = fast(FakeEnding::Finish);
        b.step = Duration::from_millis(30);
        let (ctl, rx) = start(&b);
        ctl.cancel();
        assert_eq!(collect(&rx).last(), Some(&RunEvent::Cancelled));
    }

    #[test]
    fn counts_files_recursively_up_to_a_limit() {
        let dir = std::env::temp_dir().join(format!("luna-app-count-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for p in ["a", "b", "sub/c"] {
            std::fs::write(dir.join(p), b"x").unwrap();
        }
        assert_eq!(count_files(&dir, 100), 3);
        assert_eq!(count_files(&dir, 2), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
