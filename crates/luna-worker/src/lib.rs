//! One clip job, run inside a worker process: probe → metadata → strip frames →
//! colour → JPEG. Also the IPC message types shared with `luna-core`.
//!
//! Wire format: each message is a little-endian u32 byte length followed by a
//! postcard-encoded value, over the worker's stdin (jobs) and stdout (results).

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

/// Runs one job to completion. Never panics on bad input files.
pub fn run_job(job: &ClipJob) -> ClipResult {
    ClipResult {
        id: job.id,
        meta: None,
        codec: None,
        bytes: 0,
        thumbnails: vec![Err("Not implemented yet".into()); job.strip as usize],
        warnings: vec![],
    }
}
