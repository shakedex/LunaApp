//! One run from source folder to the three report files.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use luna_report::{Camera, Card, Clip, CustomField, Header, Logo, Report, Thumbnail};
use luna_worker::pool::{JobOutcome, PoolConfig, PoolError, run_pool};
use luna_worker::{ClipJob, ClipResult};

use crate::assemble::{OutputFiles, clip_fields, output_folder, thumbnails, write_outputs};
use crate::identity::resolve_identity;
use crate::scan::{Scan, scan_roots};
use crate::settings::Settings;

/// Thumbnails per clip: first, middle, last.
pub const STRIP: u32 = 3;
/// Thumbnail width in pixels.
pub const THUMB_WIDTH: u32 = 640;

/// How to start worker processes.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub workers: usize,
    pub timeout: Duration,
}

impl WorkerSpec {
    /// This executable in `--worker` mode, sized for this machine. The decoders
    /// thread internally, so a few workers keep every core busy.
    pub fn current_exe() -> std::io::Result<WorkerSpec> {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        Ok(WorkerSpec {
            exe: std::env::current_exe()?,
            args: vec!["--worker".into()],
            workers: (cores / 4).clamp(2, 6),
            timeout: Duration::from_secs(300),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    FilesFound(usize),
    ClipsTotal(usize),
    ClipDone {
        done: usize,
        total: usize,
        problems: usize,
    },
}

/// What to report on: the source folders, which cameras, and the details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub sources: Vec<PathBuf>,
    /// Camera selectors; empty means every camera.
    pub cameras: Vec<String>,
    /// Saved settings with this run's overrides already applied.
    pub settings: Settings,
    /// Report folder name; `None` names it from the production and run date.
    pub name: Option<String>,
}

impl RunRequest {
    /// One folder, every camera, the given settings: what the window starts.
    pub fn folder(source: PathBuf, settings: Settings) -> Self {
        Self {
            sources: vec![source],
            cameras: Vec::new(),
            settings,
            name: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub files: OutputFiles,
    pub summary: String,
    pub clips: usize,
    pub clips_with_problems: usize,
    pub not_included: usize,
    pub unreadable: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    SourceUnreadable(String),
    Workers(String),
    Report(String),
    Output(String),
    Cancelled,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::SourceUnreadable(m) => write!(f, "The source folder can't be read: {m}"),
            RunError::Workers(m) => write!(f, "Luna couldn't start reading clips: {m}"),
            RunError::Report(m) => write!(f, "The report couldn't be made: {m}"),
            RunError::Output(m) => write!(f, "The report couldn't be saved: {m}"),
            RunError::Cancelled => f.write_str("Cancelled"),
        }
    }
}

impl std::error::Error for RunError {}

/// Fails with the first source folder that can't be read.
pub(crate) fn check_sources(sources: &[PathBuf]) -> Result<(), RunError> {
    if sources.is_empty() {
        return Err(RunError::SourceUnreadable("no source folder given".into()));
    }
    for source in sources {
        std::fs::read_dir(source)
            .map_err(|e| RunError::SourceUnreadable(format!("{}: {e}", source.display())))?;
    }
    Ok(())
}

/// Scans the request's sources, reads every clip in worker processes, and
/// writes the PDF, HTML and CSV into a new folder under the output location.
pub fn run(
    req: &RunRequest,
    workers: &WorkerSpec,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &AtomicBool,
) -> Result<RunOutcome, RunError> {
    let settings = &req.settings;
    check_sources(&req.sources)?;
    let scanned = scan_roots(&req.sources);
    let files = scanned
        .clips
        .iter()
        .map(|c| 1 + c.segments.len() + c.sidecars.len())
        .sum::<usize>()
        + scanned.not_included.len()
        + scanned.unreadable.len();
    progress(Progress::FilesFound(files));
    let total = scanned.clips.len();
    progress(Progress::ClipsTotal(total));

    let jobs = scanned
        .clips
        .iter()
        .enumerate()
        .map(|(i, files)| ClipJob {
            id: i as u64,
            files: files.clone(),
            strip: STRIP,
            thumb_width: THUMB_WIDTH,
        })
        .collect();
    let counts = Mutex::new((0usize, 0usize));
    let on_done = |_: u64, outcome: &JobOutcome| {
        let mut c = counts.lock().unwrap_or_else(|e| e.into_inner());
        c.0 += 1;
        c.1 += usize::from(outcome_has_problems(outcome));
        progress(Progress::ClipDone {
            done: c.0,
            total,
            problems: c.1,
        });
    };
    let config = PoolConfig {
        exe: workers.exe.clone(),
        args: workers.args.clone(),
        env: vec![],
        workers: workers.workers,
        timeout: workers.timeout,
    };
    let done = match run_pool(&config, jobs, cancel, &on_done) {
        Ok(d) => d,
        Err(PoolError::Cancelled) => return Err(RunError::Cancelled),
        Err(PoolError::Spawn(m)) => return Err(RunError::Workers(m)),
    };
    let mut outcomes = vec![JobOutcome::Crashed; total];
    for (id, outcome) in done {
        if let Some(slot) = outcomes.get_mut(id as usize) {
            *slot = outcome;
        }
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(RunError::Cancelled);
    }

    let now = jiff::Zoned::now();
    let report = build_report(
        &req.sources,
        settings,
        scanned,
        outcomes,
        &now.strftime("%Y-%m-%d %H:%M").to_string(),
    );
    let pdf = luna_report::write_pdf(&report).map_err(|e| RunError::Report(e.to_string()))?;
    let html = luna_report::write_html(&report).map_err(|e| RunError::Report(e.to_string()))?;
    let csv = luna_report::write_csv(&report);
    if cancel.load(Ordering::SeqCst) {
        return Err(RunError::Cancelled);
    }
    let target = output_folder(
        &settings.output_dir,
        &settings.production,
        req.name.as_deref(),
        &now.strftime("%Y-%m-%d %H%M").to_string(),
    );
    let stem = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Luna Report".into());
    let files = write_outputs(&target, &stem, &pdf, &html, &csv)
        .map_err(|e| RunError::Output(e.to_string()))?;
    let totals = report.totals();
    Ok(RunOutcome {
        summary: summary(&report),
        files,
        clips: totals.clips,
        clips_with_problems: totals.clips_with_problems,
        not_included: report.not_included.len(),
        unreadable: report.unreadable.len(),
    })
}

fn outcome_has_problems(outcome: &JobOutcome) -> bool {
    match outcome {
        JobOutcome::Done(r) => {
            r.meta.is_none() || !r.warnings.is_empty() || r.thumbnails.iter().any(|t| t.is_err())
        }
        JobOutcome::Crashed | JobOutcome::TimedOut => true,
    }
}

/// The report model from the scan, the worker outcomes (one per scanned clip,
/// same order) and the settings.
pub fn build_report(
    roots: &[PathBuf],
    settings: &Settings,
    scanned: Scan,
    outcomes: Vec<JobOutcome>,
    generated_at: &str,
) -> Report {
    let results: Vec<ClipResult> = scanned
        .clips
        .iter()
        .zip(
            outcomes
                .into_iter()
                .chain(std::iter::repeat(JobOutcome::Crashed)),
        )
        .enumerate()
        .map(|(i, (_, outcome))| match outcome {
            JobOutcome::Done(r) => *r,
            // The report lists a frame's reason under the clip once, so the
            // tile reason and the warning say the same thing.
            JobOutcome::Crashed => failed(i, CRASHED, CRASHED),
            JobOutcome::TimedOut => failed(i, TIMED_OUT, TIMED_OUT),
        })
        .collect();

    let identity_input: Vec<_> = scanned
        .clips
        .iter()
        .zip(&results)
        .map(|(files, r)| (files.clone(), r.meta.as_ref()))
        .collect();
    let groups = resolve_identity(roots, &identity_input);

    let cameras = groups
        .into_iter()
        .map(|camera| Camera {
            name: camera.name,
            cards: camera
                .cards
                .into_iter()
                .map(|card| Card {
                    reel: card.reel,
                    clips: card
                        .clips
                        .into_iter()
                        .map(|id| {
                            let files = &scanned.clips[id.index];
                            let r = &results[id.index];
                            let name = files
                                .main
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            let mut thumbs = thumbnails(r.thumbnails.clone());
                            thumbs.resize_with(STRIP as usize, || Thumbnail::Missing {
                                reason: "No picture".into(),
                            });
                            Clip {
                                fields: clip_fields(
                                    &name,
                                    r.meta.as_ref(),
                                    r.codec.as_ref(),
                                    r.bytes,
                                ),
                                camera_source: id.camera_source,
                                reel_source: id.reel_source,
                                thumbnails: thumbs,
                                warnings: r.warnings.clone(),
                            }
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    Report {
        header: header(settings),
        theme: settings.report_theme,
        sources: roots
            .iter()
            .map(|r| r.to_string_lossy().into_owned())
            .collect(),
        generated_at: generated_at.to_owned(),
        luna_version: env!("CARGO_PKG_VERSION").to_owned(),
        cameras,
        not_included: scanned.not_included,
        unreadable: scanned.unreadable,
    }
}

const CRASHED: &str = "The decoder crashed on this clip";
const TIMED_OUT: &str = "Reading this clip timed out";

fn failed(index: usize, warning: &str, tile: &str) -> ClipResult {
    ClipResult {
        id: index as u64,
        meta: None,
        codec: None,
        bytes: 0,
        thumbnails: vec![Err(tile.to_owned()); STRIP as usize],
        warnings: vec![warning.to_owned()],
    }
}

fn header(settings: &Settings) -> Header {
    let some = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_owned());
    let logo = settings
        .logo
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| Logo::from_bytes(bytes).ok());
    Header {
        production: some(&settings.production),
        company: some(&settings.company),
        author: some(&settings.author),
        logo,
        custom: settings
            .custom
            .iter()
            .filter(|(l, v)| !l.trim().is_empty() || !v.trim().is_empty())
            .map(|(label, value)| CustomField {
                label: label.clone(),
                value: value.clone(),
            })
            .collect(),
    }
}

/// One line for the Done screen.
pub fn summary(report: &Report) -> String {
    let t = report.totals();
    match (t.clips, t.clips_with_problems) {
        (0, _) => "No camera clips in this folder".into(),
        (1, 0) => "1 clip, no problems".into(),
        (n, 0) => format!("{n} clips, no problems"),
        (1, _) => "The clip needs attention".into(),
        (n, p) => format!("{p} of {n} clips need attention"),
    }
}
