//! What the main window shows for a given [`Flow`], as plain strings and
//! numbers. Kept free of Slint types so it can be unit tested.

use std::path::Path;

use crate::flow::{Flow, FlowState, Progress};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewPhase {
    #[default]
    Idle,
    Picked,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MainView {
    pub phase: ViewPhase,
    /// The picked source folder, in full.
    pub source: String,
    /// Just the source folder's own name, e.g. `A001_R2KL`.
    pub source_name: String,
    /// Why the last pick was rejected (Idle only).
    pub source_error: String,
    pub files_found: String,
    pub clips_done: String,
    pub clips_total: String,
    pub problems: String,
    pub has_problems: bool,
    /// 0..=1. Meaningless while `indeterminate`.
    pub progress: f32,
    /// True until the run knows how many clips there are.
    pub indeterminate: bool,
    pub cancelling: bool,
    /// One line about problems after a finished run.
    pub summary: String,
    pub pdf_name: String,
    pub html_name: String,
    pub csv_name: String,
    pub folder: String,
    /// A source-drive copy that couldn't be saved (Done only).
    pub copy_error: String,
    /// The run-level error (Failed only).
    pub failure: String,
}

pub fn main_view(flow: &Flow) -> MainView {
    let mut v = MainView::default();
    match flow.state() {
        FlowState::Idle { error } => {
            v.phase = ViewPhase::Idle;
            v.source_error = error.clone().unwrap_or_default();
        }
        FlowState::SourcePicked { source } => {
            v.phase = ViewPhase::Picked;
            v.source = source.display().to_string();
            v.source_name = file_name(source);
        }
        FlowState::Running {
            source, progress, ..
        } => {
            v.phase = ViewPhase::Running;
            v.source = source.display().to_string();
            v.source_name = file_name(source);
            fill_counts(&mut v, progress);
            v.cancelling = progress.cancelling;
            match progress.clips_total {
                Some(total) if total > 0 => {
                    v.indeterminate = false;
                    v.progress = (progress.clips_done as f32 / total as f32).clamp(0.0, 1.0);
                }
                Some(_) => v.progress = 1.0,
                None => v.indeterminate = true,
            }
        }
        FlowState::Done { outcome, progress } => {
            v.phase = ViewPhase::Done;
            fill_counts(&mut v, progress);
            v.progress = 1.0;
            v.summary = done_summary(&outcome.summary, progress);
            v.pdf_name = file_name(&outcome.pdf);
            v.html_name = file_name(&outcome.html);
            v.csv_name = file_name(&outcome.csv);
            v.folder = outcome.folder.display().to_string();
            v.copy_error = outcome.copy_error.clone();
        }
        FlowState::Failed { source, message } => {
            v.phase = ViewPhase::Failed;
            v.source = source.display().to_string();
            v.failure = message.clone();
        }
        FlowState::Cancelled { source } => {
            v.phase = ViewPhase::Cancelled;
            v.source = source.display().to_string();
        }
    }
    v
}

fn fill_counts(v: &mut MainView, p: &Progress) {
    v.files_found = p.files_found.map(count).unwrap_or_else(|| "–".into());
    v.clips_done = count(p.clips_done);
    v.clips_total = p.clips_total.map(count).unwrap_or_else(|| "–".into());
    v.problems = count(p.problems);
    v.has_problems = p.problems > 0;
}

fn done_summary(summary: &str, p: &Progress) -> String {
    if !summary.trim().is_empty() {
        return summary.trim().to_owned();
    }
    match p.clips_total {
        Some(1) => "1 clip, no problems".into(),
        Some(n) => format!("{} clips, no problems", count(n)),
        None => "No problems".into(),
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// Formats a count with thousands separators: 12345 → "12,345".
pub fn count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{RunEvent, RunOutcome};
    use std::path::PathBuf;

    fn outcome(summary: &str) -> RunOutcome {
        RunOutcome {
            pdf: "out/A001 report/camera-report.pdf".into(),
            html: "out/A001 report/camera-report.html".into(),
            csv: "out/A001 report/camera-report.csv".into(),
            folder: "out/A001 report".into(),
            copy_error: String::new(),
            summary: summary.into(),
        }
    }

    fn running() -> (Flow, u64) {
        let mut f = Flow::new();
        f.pick_source(Ok("A001".into()));
        let (run, _) = f.start().unwrap();
        (f, run)
    }

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1000), "1,000");
        assert_eq!(count(1234567), "1,234,567");
    }

    #[test]
    fn idle_shows_the_rejection() {
        let mut f = Flow::new();
        f.pick_source(Err("Not a folder".into()));
        let v = main_view(&f);
        assert_eq!(v.phase, ViewPhase::Idle);
        assert_eq!(v.source_error, "Not a folder");
    }

    #[test]
    fn picked_shows_the_folder_name_and_full_path() {
        let mut f = Flow::new();
        f.pick_source(Ok(PathBuf::from("cards").join("A001_R2KL")));
        let v = main_view(&f);
        assert_eq!(v.phase, ViewPhase::Picked);
        assert_eq!(v.source_name, "A001_R2KL");
        assert!(v.source.ends_with("A001_R2KL") && v.source.starts_with("cards"));
    }

    #[test]
    fn running_is_indeterminate_until_the_clip_total_is_known() {
        let (mut f, run) = running();
        let v = main_view(&f);
        assert!(v.indeterminate);
        assert_eq!(v.files_found, "–");
        assert_eq!(v.clips_total, "–");
        f.apply(run, RunEvent::FilesFound(1204));
        f.apply(
            run,
            RunEvent::ClipDone {
                done: 3,
                total: 12,
                problems: 0,
            },
        );
        let v = main_view(&f);
        assert!(!v.indeterminate);
        assert_eq!(v.files_found, "1,204");
        assert_eq!(v.clips_done, "3");
        assert_eq!(v.clips_total, "12");
        assert!((v.progress - 0.25).abs() < 1e-6);
        assert!(!v.has_problems);
    }

    #[test]
    fn running_flags_problems_and_cancelling() {
        let (mut f, run) = running();
        f.apply(
            run,
            RunEvent::ClipDone {
                done: 5,
                total: 12,
                problems: 2,
            },
        );
        f.request_cancel();
        let v = main_view(&f);
        assert!(v.has_problems);
        assert_eq!(v.problems, "2");
        assert!(v.cancelling);
    }

    #[test]
    fn done_lists_the_files_and_the_problem_line() {
        let (mut f, run) = running();
        f.apply(
            run,
            RunEvent::Finished(outcome("2 clips couldn't be decoded")),
        );
        let v = main_view(&f);
        assert_eq!(v.phase, ViewPhase::Done);
        assert_eq!(v.pdf_name, "camera-report.pdf");
        assert_eq!(v.html_name, "camera-report.html");
        assert_eq!(v.csv_name, "camera-report.csv");
        assert_eq!(v.summary, "2 clips couldn't be decoded");
    }

    #[test]
    fn done_without_problems_says_so_with_the_count() {
        let (mut f, run) = running();
        f.apply(
            run,
            RunEvent::ClipDone {
                done: 12,
                total: 12,
                problems: 0,
            },
        );
        f.apply(run, RunEvent::Finished(outcome("")));
        assert_eq!(main_view(&f).summary, "12 clips, no problems");
    }

    #[test]
    fn failed_carries_the_message() {
        let (mut f, run) = running();
        f.apply(run, RunEvent::Failed("Disk full".into()));
        let v = main_view(&f);
        assert_eq!(v.phase, ViewPhase::Failed);
        assert_eq!(v.failure, "Disk full");
    }
}
