//! The main window's one flow, as a plain state machine:
//! Idle → SourcePicked → Running → Done / Failed / Cancelled → Idle.
//!
//! No UI types here; `view` turns a [`Flow`] into what the window shows.

use std::path::{Path, PathBuf};

use crate::backend::{RunEvent, RunOutcome};

/// Counts shown while a run is in progress. `None` until the run reports them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub files_found: Option<usize>,
    pub clips_total: Option<usize>,
    pub clips_done: usize,
    pub problems: usize,
    /// Cancel was pressed; waiting for the run to stop.
    pub cancelling: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowState {
    /// Waiting for a source. `error` explains why the last pick was rejected.
    Idle {
        error: Option<String>,
    },
    SourcePicked {
        source: PathBuf,
    },
    Running {
        source: PathBuf,
        run: u64,
        progress: Progress,
    },
    Done {
        outcome: RunOutcome,
        progress: Progress,
    },
    Failed {
        source: PathBuf,
        message: String,
    },
    Cancelled {
        source: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    state: FlowState,
    /// Id of the most recent run; events from older runs are ignored.
    last_run: u64,
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

impl Flow {
    pub fn new() -> Self {
        Self {
            state: FlowState::Idle { error: None },
            last_run: 0,
        }
    }

    pub fn state(&self) -> &FlowState {
        &self.state
    }

    /// Whether a new source may be picked (or dropped) right now.
    pub fn accepts_source(&self) -> bool {
        matches!(
            self.state,
            FlowState::Idle { .. } | FlowState::SourcePicked { .. }
        )
    }

    /// Applies the result of validating a picked or dropped folder.
    pub fn pick_source(&mut self, picked: Result<PathBuf, String>) {
        if !self.accepts_source() {
            return;
        }
        self.state = match picked {
            Ok(source) => FlowState::SourcePicked { source },
            Err(error) => FlowState::Idle { error: Some(error) },
        };
    }

    /// Moves to Running. Returns the run id and source to hand to the backend,
    /// or `None` when there is nothing to start.
    pub fn start(&mut self) -> Option<(u64, PathBuf)> {
        let FlowState::SourcePicked { source } = &self.state else {
            return None;
        };
        let source = source.clone();
        self.last_run += 1;
        self.state = FlowState::Running {
            source: source.clone(),
            run: self.last_run,
            progress: Progress::default(),
        };
        Some((self.last_run, source))
    }

    /// A run handed over at launch (`Luna.exe report …`): straight from Idle
    /// to Running, showing `source`. Returns the run id, or `None` when a run
    /// is already under way or finished.
    pub fn start_preset(&mut self, source: PathBuf) -> Option<u64> {
        if !self.accepts_source() {
            return None;
        }
        self.state = FlowState::SourcePicked { source };
        self.start().map(|(run, _)| run)
    }

    /// Marks the run as stopping. Returns true when the backend should be told.
    pub fn request_cancel(&mut self) -> bool {
        match &mut self.state {
            FlowState::Running { progress, .. } if !progress.cancelling => {
                progress.cancelling = true;
                true
            }
            _ => false,
        }
    }

    /// Applies an event from run `run`. Events from any other run are ignored.
    pub fn apply(&mut self, run: u64, event: RunEvent) {
        let FlowState::Running {
            source,
            run: current,
            progress,
        } = &mut self.state
        else {
            return;
        };
        if *current != run {
            return;
        }
        match event {
            RunEvent::FilesFound(n) => progress.files_found = Some(n),
            RunEvent::ClipsTotal(n) => progress.clips_total = Some(n),
            RunEvent::ClipDone {
                done,
                total,
                problems,
            } => {
                progress.clips_done = done;
                progress.clips_total = Some(total);
                progress.problems = problems;
            }
            RunEvent::Finished(outcome) => {
                let progress = std::mem::take(progress);
                self.state = FlowState::Done { outcome, progress };
            }
            RunEvent::Failed(message) => {
                let source = std::mem::take(source);
                self.state = FlowState::Failed { source, message };
            }
            RunEvent::Cancelled => {
                let source = std::mem::take(source);
                self.state = FlowState::Cancelled { source };
            }
        }
    }

    /// From Failed or Cancelled, back to the same source so it can be started again.
    pub fn back(&mut self) {
        if let FlowState::Failed { source, .. } | FlowState::Cancelled { source } = &mut self.state
        {
            let source = std::mem::take(source);
            self.state = FlowState::SourcePicked { source };
        }
    }

    /// From any finished state, back to an empty Idle.
    pub fn new_report(&mut self) {
        if matches!(
            self.state,
            FlowState::Done { .. } | FlowState::Failed { .. } | FlowState::Cancelled { .. }
        ) {
            self.state = FlowState::Idle { error: None };
        }
    }
}

/// A source must be an existing, readable directory.
pub fn validate_source(path: &Path) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(path)
        .map_err(|_| format!("{} doesn't exist or can't be reached.", path.display()))?;
    if !meta.is_dir() {
        return Err(format!(
            "{} is a file. Pick the folder that holds the camera originals.",
            name_of(path)
        ));
    }
    std::fs::read_dir(path).map_err(|e| format!("Luna can't read {}: {e}", path.display()))?;
    Ok(path.to_path_buf())
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome() -> RunOutcome {
        RunOutcome {
            pdf: "r/a.pdf".into(),
            html: "r/a.html".into(),
            csv: "r/a.csv".into(),
            folder: "r".into(),
            summary: String::new(),
        }
    }

    fn running() -> (Flow, u64) {
        let mut f = Flow::new();
        f.pick_source(Ok("A001".into()));
        let (run, _) = f.start().unwrap();
        (f, run)
    }

    #[test]
    fn starts_idle_without_error() {
        assert_eq!(Flow::new().state(), &FlowState::Idle { error: None });
    }

    #[test]
    fn picking_a_valid_source_moves_to_source_picked() {
        let mut f = Flow::new();
        f.pick_source(Ok("A001".into()));
        assert_eq!(
            f.state(),
            &FlowState::SourcePicked {
                source: "A001".into()
            }
        );
    }

    #[test]
    fn a_rejected_source_shows_its_error_in_idle() {
        let mut f = Flow::new();
        f.pick_source(Ok("A001".into()));
        f.pick_source(Err("nope".into()));
        assert_eq!(
            f.state(),
            &FlowState::Idle {
                error: Some("nope".into())
            }
        );
    }

    #[test]
    fn start_needs_a_source() {
        assert_eq!(Flow::new().start(), None);
    }

    #[test]
    fn start_hands_out_a_fresh_run_id_each_time() {
        let (mut f, first) = running();
        f.apply(first, RunEvent::Cancelled);
        f.back();
        let (second, source) = f.start().unwrap();
        assert_ne!(first, second);
        assert_eq!(source, PathBuf::from("A001"));
    }

    #[test]
    fn sources_cannot_change_while_running() {
        let (mut f, _) = running();
        f.pick_source(Ok("B002".into()));
        assert!(
            matches!(f.state(), FlowState::Running { source, .. } if source == Path::new("A001"))
        );
        assert!(!f.accepts_source());
    }

    #[test]
    fn progress_events_update_the_counts() {
        let (mut f, run) = running();
        f.apply(run, RunEvent::FilesFound(40));
        f.apply(run, RunEvent::ClipsTotal(12));
        f.apply(
            run,
            RunEvent::ClipDone {
                done: 3,
                total: 12,
                problems: 1,
            },
        );
        let FlowState::Running { progress, .. } = f.state() else {
            panic!()
        };
        assert_eq!(
            progress,
            &Progress {
                files_found: Some(40),
                clips_total: Some(12),
                clips_done: 3,
                problems: 1,
                cancelling: false
            }
        );
    }

    #[test]
    fn events_from_an_older_run_are_ignored() {
        let (mut f, run) = running();
        f.apply(run + 7, RunEvent::FilesFound(40));
        f.apply(run + 7, RunEvent::Cancelled);
        assert!(
            matches!(f.state(), FlowState::Running { progress, .. } if progress.files_found.is_none())
        );
    }

    #[test]
    fn finished_moves_to_done_and_keeps_the_counts() {
        let (mut f, run) = running();
        f.apply(
            run,
            RunEvent::ClipDone {
                done: 2,
                total: 2,
                problems: 1,
            },
        );
        f.apply(run, RunEvent::Finished(outcome()));
        let FlowState::Done {
            outcome: o,
            progress,
        } = f.state()
        else {
            panic!()
        };
        assert_eq!(o, &outcome());
        assert_eq!(progress.problems, 1);
    }

    #[test]
    fn cancel_is_requested_once_then_cancelled_event_ends_the_run() {
        let (mut f, run) = running();
        assert!(f.request_cancel());
        assert!(!f.request_cancel());
        f.apply(run, RunEvent::Cancelled);
        assert_eq!(
            f.state(),
            &FlowState::Cancelled {
                source: "A001".into()
            }
        );
    }

    #[test]
    fn cancel_outside_a_run_does_nothing() {
        let mut f = Flow::new();
        assert!(!f.request_cancel());
    }

    #[test]
    fn failure_keeps_the_message_and_back_returns_to_the_source() {
        let (mut f, run) = running();
        f.apply(run, RunEvent::Failed("disk full".into()));
        assert_eq!(
            f.state(),
            &FlowState::Failed {
                source: "A001".into(),
                message: "disk full".into()
            }
        );
        f.back();
        assert_eq!(
            f.state(),
            &FlowState::SourcePicked {
                source: "A001".into()
            }
        );
    }

    #[test]
    fn new_report_returns_to_idle_from_done() {
        let (mut f, run) = running();
        f.apply(run, RunEvent::Finished(outcome()));
        f.new_report();
        assert_eq!(f.state(), &FlowState::Idle { error: None });
    }

    #[test]
    fn new_report_does_not_interrupt_a_run() {
        let (mut f, _) = running();
        f.new_report();
        assert!(matches!(f.state(), FlowState::Running { .. }));
    }

    #[test]
    fn a_directory_is_a_valid_source() {
        let dir = std::env::temp_dir();
        assert_eq!(validate_source(&dir), Ok(dir.clone()));
    }

    #[test]
    fn a_missing_path_is_rejected() {
        let err = validate_source(Path::new("Z:/definitely/not/here/luna")).unwrap_err();
        assert!(err.contains("doesn't exist"), "{err}");
    }

    #[test]
    fn a_file_is_rejected() {
        let file = std::env::temp_dir().join(format!("luna-app-src-{}.mov", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let err = validate_source(&file).unwrap_err();
        std::fs::remove_file(&file).unwrap();
        assert!(err.contains("is a file"), "{err}");
    }

    #[test]
    fn start_preset_goes_straight_to_running() {
        let mut f = Flow::new();
        assert_eq!(f.start_preset("E:/Footage".into()), Some(1));
        assert!(matches!(
            f.state(),
            FlowState::Running { source, run: 1, .. } if source == Path::new("E:/Footage")
        ));
    }

    #[test]
    fn start_preset_ignored_while_running() {
        let mut f = Flow::new();
        f.start_preset("E:/Footage".into());
        assert_eq!(f.start_preset("F:/Other".into()), None);
    }
}
