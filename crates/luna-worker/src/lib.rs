//! One clip job, run inside a worker process: probe → metadata → strip frames →
//! colour → JPEG. Also the IPC message types shared with `luna-core`.
//!
//! Wire format: each message is a little-endian u32 byte length followed by a
//! postcard-encoded value, over the worker's stdin (jobs) and stdout (results).

pub mod pool;
pub mod protocol;

use luna_formats::{ClipFiles, ClipMeta, Codec};
use serde::{Deserialize, Serialize};

/// Core → worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    Job(ClipJob),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipJob {
    pub id: u64,
    pub files: ClipFiles,
    /// Strip size (spec: 3 — first, middle, last).
    pub strip: u32,
    /// Thumbnail width in pixels (spec: about 640).
    pub thumb_width: u32,
}

/// Worker → core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipResult {
    pub id: u64,
    /// `None` when the container couldn't be read at all; `warnings` says why.
    pub meta: Option<ClipMeta>,
    pub codec: Option<Codec>,
    /// Total size of the clip's files in bytes.
    pub bytes: u64,
    /// One entry per strip position: JPEG bytes, or the reason there's no image.
    pub thumbnails: Vec<Result<Vec<u8>, String>>,
    pub warnings: Vec<String>,
}

/// The worker process loop: jobs on stdin, results on stdout, until Shutdown or EOF.
pub fn worker_main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = std::io::BufReader::new(stdin.lock());
    let mut output = std::io::BufWriter::new(stdout.lock());
    let hooks = std::env::var_os("LUNA_WORKER_TEST_HOOKS").is_some();
    while let Ok(Some(request)) = protocol::read_message::<Request>(&mut input) {
        let job = match request {
            Request::Job(job) => job,
            Request::Shutdown => break,
        };
        if hooks {
            test_hooks(&job);
        }
        let result = std::panic::catch_unwind(|| run_job(&job)).unwrap_or_else(|_| ClipResult {
            id: job.id,
            meta: None,
            codec: None,
            bytes: 0,
            thumbnails: vec![
                Err("Luna hit an internal error on this clip".into());
                job.strip as usize
            ],
            warnings: vec!["Luna hit an internal error reading this clip".into()],
        });
        if protocol::write_message(&mut output, &result).is_err() {
            break;
        }
    }
}

/// Test-only misbehaviour, enabled by `LUNA_WORKER_TEST_HOOKS`, so the pool's
/// crash and timeout handling can be exercised with a real process.
fn test_hooks(job: &ClipJob) {
    let name = job.files.main.to_string_lossy();
    if name.contains("__crash__") {
        std::process::abort();
    }
    if name.contains("__hang__") {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    }
}

/// Frame numbers for a strip of `strip` thumbnails: first, evenly spaced, last.
pub fn strip_positions(frame_count: u64, strip: u32) -> Vec<u64> {
    if frame_count == 0 || strip == 0 {
        return Vec::new();
    }
    if strip == 1 {
        return vec![0];
    }
    let last = frame_count - 1;
    (0..strip as u64)
        .map(|i| i * last / (strip as u64 - 1))
        .collect()
}

/// Runs one job to completion. Never panics on bad input files.
pub fn run_job(job: &ClipJob) -> ClipResult {
    let files = &job.files;
    let bytes = std::iter::once(&files.main)
        .chain(&files.segments)
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let strip = job.strip as usize;
    let mut warnings = Vec::new();

    let probe = match luna_formats::probe(files) {
        Ok(p) => p,
        Err(e) => {
            let reason = format!("Could not read this clip: {e}");
            return ClipResult {
                id: job.id,
                meta: None,
                codec: None,
                bytes,
                thumbnails: vec![Err(reason.clone()); strip],
                warnings: vec![reason],
            };
        }
    };
    warnings.extend(probe.warnings.iter().cloned());

    let positions = probe
        .meta
        .frame_count
        .map(|n| strip_positions(n, job.strip))
        .unwrap_or_default();
    let thumbnails = if positions.is_empty() {
        vec![Err("The clip's frame count is unknown".into()); strip]
    } else {
        let aspect = probe.layout.pixel_aspect.map(|r| (r.num, r.den));
        let mut unknown_transform = false;
        let thumbs = luna_decode::decode_frames(files, &probe, &positions, job.thumb_width)
            .into_iter()
            .map(|frame| {
                let frame = frame.map_err(|e| e.to_string())?;
                let rendered = luna_color::render(&frame, job.thumb_width, aspect);
                unknown_transform |= rendered.transform_unknown;
                Ok(luna_color::encode_jpeg(&rendered.image))
            })
            .collect();
        if unknown_transform {
            warnings.push("Display transform unknown: thumbnails are shown as decoded".into());
        }
        thumbs
    };

    ClipResult {
        id: job.id,
        meta: Some(probe.meta),
        codec: Some(probe.codec),
        bytes,
        thumbnails,
        warnings,
    }
}
