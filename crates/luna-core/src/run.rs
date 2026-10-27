//! One run from source folder to the three report files.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use luna_report::{Camera, Card, Clip, CustomField, Header, Logo, Report, Thumbnail};
use luna_worker::pool::{JobOutcome, PoolConfig, PoolError, run_pool};
use luna_worker::{ClipJob, ClipResult, ColourJob};

use crate::assemble::{
    OutputFiles, clip_fields, copy_drives, output_folder, thumbnails, write_outputs,
};
use crate::identity::resolve_identity;
use crate::scan::{Scan, outermost, scan_roots};
use crate::select::CameraFilter;
use crate::settings::{Settings, ThumbnailColour};

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
    /// Metadata read for a camera filter, before any clip is decoded.
    Probed {
        done: usize,
        total: usize,
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
    /// The copies on the source drives (`copy_to_source`).
    pub copies: Vec<PathBuf>,
    /// Copies that couldn't be written; the report itself was.
    pub copy_errors: Vec<String>,
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
    /// The chosen LUT is missing or can't be used; a full sentence.
    Lut(String),
    /// The camera filter matched none of the cameras found.
    NoSuchCamera {
        asked: Vec<String>,
        found: Vec<String>,
    },
    Cancelled,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::SourceUnreadable(m) => write!(f, "The source folder can't be read: {m}"),
            RunError::Workers(m) => write!(f, "Luna couldn't start reading clips: {m}"),
            RunError::Report(m) => write!(f, "The report couldn't be made: {m}"),
            RunError::Output(m) => write!(f, "The report couldn't be saved: {m}"),
            RunError::Lut(m) => f.write_str(m),
            RunError::NoSuchCamera { asked, found } => write!(
                f,
                "No camera matches {}. Cameras found: {}",
                asked.join(", "),
                if found.is_empty() {
                    "none".to_owned()
                } else {
                    found.join(", ")
                }
            ),
            RunError::Cancelled => f.write_str("Cancelled"),
        }
    }
}

impl std::error::Error for RunError {}

/// The source folders as one set of roots: each made absolute and canonical
/// (so `E:\footage`, `E:\Footage\` and `E:\Footage\A001\..` are one folder),
/// then any root inside another dropped. Fails with the first source that
/// can't be read.
pub fn source_roots(sources: &[PathBuf]) -> Result<Vec<PathBuf>, RunError> {
    if sources.is_empty() {
        return Err(RunError::SourceUnreadable("no source folder given".into()));
    }
    let unreadable = |s: &PathBuf, e: std::io::Error| {
        RunError::SourceUnreadable(format!("{}: {e}", s.display()))
    };
    let mut roots = Vec::with_capacity(sources.len());
    for source in sources {
        let root = dunce::canonicalize(source).map_err(|e| unreadable(source, e))?;
        std::fs::read_dir(&root).map_err(|e| unreadable(source, e))?;
        roots.push(root);
    }
    Ok(outermost(&roots))
}

/// Scans the request's sources, reads every clip in worker processes, and
/// writes the PDF, HTML and CSV into a new folder under the output location.
/// The run is logged to its own file (the `run` span).
pub fn run(
    req: &RunRequest,
    workers: &WorkerSpec,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &AtomicBool,
) -> Result<RunOutcome, RunError> {
    let span = tracing::info_span!(target: "luna::run", "run", source = %run_name(&req.sources));
    let _run = span.enter();
    let result = run_inner(req, workers, progress, cancel);
    match &result {
        Ok(_) => {}
        Err(RunError::Cancelled) => tracing::info!("cancelled"),
        Err(e) => tracing::error!("{e}"),
    }
    result
}

/// The run log's name: the first source's folder name (its full path when it
/// has none, like a drive root), with ` +N` for the other sources.
fn run_name(sources: &[PathBuf]) -> String {
    let Some(first) = sources.first() else {
        return String::new();
    };
    let name = first.file_name().map_or_else(
        || first.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    match sources.len() {
        1 => name,
        n => format!("{name} +{}", n - 1),
    }
}

/// The header of a run log: Luna, the machine, the workers and the request.
fn log_header(req: &RunRequest, workers: &WorkerSpec) {
    let cores = std::thread::available_parallelism().map_or(0, |n| n.get());
    tracing::info!(
        cores,
        workers = workers.workers,
        timeout = ?workers.timeout,
        "Luna {} {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    tracing::debug!("settings: {:?}", req.settings);
    for source in &req.sources {
        tracing::info!("source: {}", source.display());
    }
    if req.cameras.is_empty() {
        tracing::info!("cameras: all");
    } else {
        tracing::info!("cameras: {}", req.cameras.join(", "));
    }
    tracing::info!("output: {}", req.settings.output_dir.display());
}

/// How the decoded clips came out, for the run log's summary. `ok` and
/// `warnings` split every clip; `failed` (nothing could be read), `crashed`
/// and `timed_out` are clips counted under `warnings`.
#[derive(Debug, Default)]
struct Tally {
    ok: usize,
    warnings: usize,
    failed: usize,
    crashed: usize,
    timed_out: usize,
}

impl Tally {
    fn of(done: &[(u64, JobOutcome)]) -> Self {
        let mut t = Tally::default();
        for (_, outcome) in done {
            if !outcome_has_problems(outcome) {
                t.ok += 1;
                continue;
            }
            t.warnings += 1;
            match outcome {
                JobOutcome::Done(r) if r.meta.is_none() => t.failed += 1,
                JobOutcome::Done(_) => {}
                JobOutcome::Crashed => t.crashed += 1,
                JobOutcome::TimedOut => t.timed_out += 1,
            }
        }
        t
    }
}

fn run_inner(
    req: &RunRequest,
    workers: &WorkerSpec,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &AtomicBool,
) -> Result<RunOutcome, RunError> {
    let started = Instant::now();
    check_lut(&req.settings)?;
    log_header(req, workers);
    let settings = &req.settings;
    let roots = source_roots(&req.sources)?;
    let scanned = scan_roots(&roots);
    scanned.log(&roots);
    let files = scanned
        .clips
        .iter()
        .map(|c| 1 + c.segments.len() + c.sidecars.len())
        .sum::<usize>()
        + scanned.not_included.len()
        + scanned.unreadable.len();
    progress(Progress::FilesFound(files));
    let filter = CameraFilter::new(&req.cameras);
    let total = scanned.clips.len();

    // With a camera filter, probe every clip first: identity needs every
    // clip's metadata (neighbours lend evidence), but only the selected
    // cameras' clips are decoded.
    let (probed, selected) = if filter.is_all() {
        (
            vec![JobOutcome::Crashed; total],
            (0..total).collect::<Vec<_>>(),
        )
    } else {
        let probed = probe_all(&scanned, workers, progress, cancel)?;
        let selected = selected_clips(&roots, &scanned, &probed, &filter)?;
        (probed, selected)
    };

    let selected_total = selected.len();
    progress(Progress::ClipsTotal(selected_total));
    let counts = Mutex::new((0usize, 0usize));
    let on_done = |_: u64, outcome: &JobOutcome| {
        let mut c = counts.lock().unwrap_or_else(|e| e.into_inner());
        c.0 += 1;
        c.1 += usize::from(outcome_has_problems(outcome));
        progress(Progress::ClipDone {
            done: c.0,
            total: selected_total,
            problems: c.1,
        });
    };
    let colour = colour_job(&req.settings);
    let done = run_jobs(
        &scanned, &selected, STRIP, &colour, workers, cancel, &on_done,
    )?;
    let tally = Tally::of(&done);
    let outcomes = merge_outcomes(probed, done, &selected);
    if cancel.load(Ordering::SeqCst) {
        return Err(RunError::Cancelled);
    }

    let now = jiff::Zoned::now();
    let report = build_report(
        &roots,
        settings,
        scanned,
        outcomes,
        &now.strftime("%Y-%m-%d %H:%M").to_string(),
        (!filter.is_all()).then_some(selected.as_slice()),
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
    let files =
        write_outputs(&target, &pdf, &html, &csv).map_err(|e| RunError::Output(e.to_string()))?;
    // The copies take the name the report got.
    let stem = files
        .folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Luna Report".into());
    let (copies, copy_errors) = if settings.copy_to_source {
        copy_to_drives(&files.folder, &roots, &stem, &pdf, &html, &csv)
    } else {
        Default::default()
    };
    tracing::info!("report: {}", files.folder.display());
    tracing::info!(
        clips = selected_total,
        ok = tally.ok,
        warnings = tally.warnings,
        failed = tally.failed,
        crashed = tally.crashed,
        timed_out = tally.timed_out,
        wall = ?started.elapsed(),
        "summary:"
    );
    let totals = report.totals();
    Ok(RunOutcome {
        summary: summary(&report),
        files,
        copies,
        copy_errors,
        clips: totals.clips,
        clips_with_problems: totals.clips_with_problems,
        not_included: report.not_included.len(),
        unreadable: report.unreadable.len(),
    })
}

/// Writes the report again into `Luna` at the root of each source drive, once
/// per drive ([`copy_drives`]).
fn copy_to_drives(
    report: &Path,
    roots: &[PathBuf],
    stem: &str,
    pdf: &[u8],
    html: &[u8],
    csv: &[u8],
) -> (Vec<PathBuf>, Vec<String>) {
    let report = dunce::canonicalize(report).unwrap_or_else(|_| report.to_path_buf());
    let (mut copies, mut errors) = (Vec::new(), Vec::new());
    for drive in copy_drives(&report, roots) {
        let target = output_folder(&drive.join("Luna"), "", Some(stem), "");
        match write_outputs(&target, pdf, html, csv) {
            Ok(files) => copies.push(files.folder),
            Err(e) => errors.push(format!(
                "The copy on {} couldn't be saved: {e}",
                drive.display()
            )),
        }
    }
    (copies, errors)
}

/// Runs the given clips (indices into the scan) through the worker pool.
/// `strip: 0` reads metadata only.
fn run_jobs(
    scanned: &Scan,
    indices: &[usize],
    strip: u32,
    colour: &ColourJob,
    workers: &WorkerSpec,
    cancel: &AtomicBool,
    on_done: &(dyn Fn(u64, &JobOutcome) + Sync),
) -> Result<Vec<(u64, JobOutcome)>, RunError> {
    let jobs = indices
        .iter()
        .map(|&i| ClipJob {
            id: i as u64,
            files: scanned.clips[i].clone(),
            strip,
            thumb_width: THUMB_WIDTH,
            colour: colour.clone(),
        })
        .collect();
    let config = PoolConfig {
        exe: workers.exe.clone(),
        args: workers.args.clone(),
        env: vec![],
        workers: workers.workers,
        timeout: workers.timeout,
        on_line: None,
    };
    match run_pool(&config, jobs, cancel, on_done) {
        Ok(d) => Ok(d),
        Err(PoolError::Cancelled) => Err(RunError::Cancelled),
        Err(PoolError::Spawn(m)) => Err(RunError::Workers(m)),
    }
}

/// Every clip's metadata, no frames: one outcome per scanned clip, in order.
pub(crate) fn probe_all(
    scanned: &Scan,
    workers: &WorkerSpec,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &AtomicBool,
) -> Result<Vec<JobOutcome>, RunError> {
    let total = scanned.clips.len();
    let done_count = Mutex::new(0usize);
    let on_done = |_: u64, _: &JobOutcome| {
        let mut d = done_count.lock().unwrap_or_else(|e| e.into_inner());
        *d += 1;
        progress(Progress::Probed { done: *d, total });
    };
    let all: Vec<usize> = (0..total).collect();
    let colour = ColourJob::default();
    let done = run_jobs(scanned, &all, 0, &colour, workers, cancel, &on_done)?;
    if cancel.load(Ordering::SeqCst) {
        return Err(RunError::Cancelled);
    }
    let mut outcomes = vec![JobOutcome::Crashed; total];
    for (id, outcome) in done {
        if let Some(slot) = outcomes.get_mut(id as usize) {
            *slot = outcome;
        }
    }
    Ok(outcomes)
}

/// One outcome per scanned clip: the decode outcome for the `selected` clips
/// (in `done`), the probe outcome for the rest. A selected clip whose decode
/// crashed, timed out or lost its metadata keeps the probed metadata, so it
/// stays in its camera and card with its row filled in.
pub fn merge_outcomes(
    probed: Vec<JobOutcome>,
    done: Vec<(u64, JobOutcome)>,
    selected: &[usize],
) -> Vec<JobOutcome> {
    let mut outcomes = probed.clone();
    for &i in selected {
        if let Some(slot) = outcomes.get_mut(i) {
            *slot = JobOutcome::Crashed;
        }
    }
    for (id, outcome) in done {
        if let Some(slot) = outcomes.get_mut(id as usize) {
            *slot = outcome;
        }
    }
    for &i in selected {
        let (Some(slot), Some(JobOutcome::Done(probe))) = (outcomes.get_mut(i), probed.get(i))
        else {
            continue;
        };
        let mut result = match std::mem::replace(slot, JobOutcome::Crashed) {
            JobOutcome::Done(r) => *r,
            JobOutcome::Crashed => failed(i, CRASHED, CRASHED),
            JobOutcome::TimedOut => failed(i, TIMED_OUT, TIMED_OUT),
        };
        if result.meta.is_none() {
            result.meta = probe.meta.clone();
            result.codec = result.codec.or_else(|| probe.codec.clone());
            if result.bytes == 0 {
                result.bytes = probe.bytes;
            }
        }
        *slot = JobOutcome::Done(Box::new(result));
    }
    outcomes
}

fn meta_of(outcome: &JobOutcome) -> Option<&luna_formats::ClipMeta> {
    match outcome {
        JobOutcome::Done(r) => r.meta.as_ref(),
        _ => None,
    }
}

/// The clips of the cameras the filter selects, by scan index. No match is
/// an error that lists the cameras found.
fn selected_clips(
    roots: &[PathBuf],
    scanned: &Scan,
    probed: &[JobOutcome],
    filter: &CameraFilter,
) -> Result<Vec<usize>, RunError> {
    let input: Vec<_> = scanned
        .clips
        .iter()
        .zip(probed)
        .map(|(files, o)| (files.clone(), meta_of(o)))
        .collect();
    let groups = resolve_identity(roots, &input);
    let mut selected: Vec<usize> = groups
        .iter()
        .filter(|cam| filter.matches(&cam.name))
        .flat_map(|cam| {
            cam.cards
                .iter()
                .flat_map(|c| c.clips.iter().map(|id| id.index))
        })
        .collect();
    if selected.is_empty() {
        return Err(RunError::NoSuchCamera {
            asked: filter.selectors().to_vec(),
            found: groups.into_iter().map(|g| g.name).collect(),
        });
    }
    selected.sort_unstable();
    Ok(selected)
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
/// same order) and the settings. `selected` (sorted scan indices) keeps only
/// those clips; `None` keeps every clip.
pub fn build_report(
    roots: &[PathBuf],
    settings: &Settings,
    scanned: Scan,
    outcomes: Vec<JobOutcome>,
    generated_at: &str,
    selected: Option<&[usize]>,
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

    let plain = colour_job(settings).is_plain();
    let cameras = groups
        .into_iter()
        .filter_map(|mut camera| {
            // Keep only the selected clips; drop cards and cameras left empty.
            if let Some(keep) = selected {
                for card in &mut camera.cards {
                    card.clips
                        .retain(|id| keep.binary_search(&id.index).is_ok());
                }
                camera.cards.retain(|c| !c.clips.is_empty());
            }
            (!camera.cards.is_empty()).then_some(camera)
        })
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
                                colour: r.colour.clone().filter(|_| !plain),
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
        show_problems: settings.show_problems,
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
        colour: None,
    }
}

/// Stops a LUT run before any work when its LUT can't be used.
fn check_lut(settings: &Settings) -> Result<(), RunError> {
    if settings.colour != ThumbnailColour::Lut {
        return Ok(());
    }
    let Some(path) = &settings.lut else {
        return Err(RunError::Lut(
            "No LUT is chosen: choose one in Settings, or pass --lut <file>".into(),
        ));
    };
    settings.clone().set_lut(path).map_err(RunError::Lut)
}

fn colour_job(settings: &Settings) -> ColourJob {
    ColourJob {
        colour: settings.colour,
        lut: settings.lut.clone(),
        camera_look: settings.camera_look,
    }
}

/// The header's Colour field; `None` for plain Rec.709.
fn colour_field(settings: &Settings) -> Option<CustomField> {
    if colour_job(settings).is_plain() {
        return None;
    }
    let base = match (settings.colour, &settings.lut) {
        (ThumbnailColour::Rec709, _) => "Rec.709".to_owned(),
        (ThumbnailColour::AsRecorded, _) => "None".to_owned(),
        (ThumbnailColour::Lut, Some(p)) => p.file_name().map_or_else(
            || p.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        ),
        (ThumbnailColour::Lut, None) => "LUT".to_owned(),
    };
    let value = if settings.camera_look {
        format!("{base} + camera LUT")
    } else {
        base
    };
    Some(CustomField {
        label: "Colour".into(),
        value,
    })
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
            .chain(colour_field(settings))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_stops_when_the_lut_is_missing() {
        let settings = Settings {
            colour: ThumbnailColour::Lut,
            lut: Some(PathBuf::from("D:/nowhere/show.cube")),
            ..Settings::default()
        };
        let err = check_lut(&settings).unwrap_err();
        assert!(matches!(err, RunError::Lut(_)));
        assert!(err.to_string().contains("show.cube"), "{err}");
    }

    #[test]
    fn lut_choice_without_a_file_is_an_error() {
        let settings = Settings {
            colour: ThumbnailColour::Lut,
            ..Settings::default()
        };
        let err = check_lut(&settings).unwrap_err().to_string();
        assert!(err.contains("--lut"), "{err}");
    }

    #[test]
    fn other_choices_need_no_lut() {
        assert!(check_lut(&Settings::default()).is_ok());
    }
}
