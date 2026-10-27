//! One clip job, run inside a worker process: probe → metadata → strip frames →
//! colour → JPEG. Also the IPC message types shared with `luna-core`.
//!
//! Wire format: each message is a little-endian u32 byte length followed by a
//! postcard-encoded value, over the worker's stdin (jobs) and stdout (results).

pub mod pool;
pub mod protocol;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use luna_color::{Applied, Colour, LumaStats, Lut, NEAR_BLACK_MAX};
use luna_decode::Frame;
use luna_formats::{ClipFiles, ClipMeta, Codec};
use serde::{Deserialize, Serialize};
use tracing::Level;

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
    pub colour: ColourJob,
}

/// How thumbnails are coloured (Settings → Colour).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailColour {
    /// Luna's own rendering.
    #[default]
    #[serde(rename = "rec709")]
    Rec709,
    /// The camera's own signal, shown as is ("None" in the app).
    #[serde(rename = "none")]
    AsRecorded,
    /// One LUT on every clip.
    #[serde(rename = "lut")]
    Lut,
}

/// The colour settings a job carries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColourJob {
    pub colour: ThumbnailColour,
    /// The `.cube` file for [`ThumbnailColour::Lut`].
    pub lut: Option<PathBuf>,
    /// Use the LUT a clip carries, when it has one.
    pub camera_look: bool,
}

impl ColourJob {
    /// Plain Rec.709: nothing to say in the report.
    pub fn is_plain(&self) -> bool {
        self.colour == ThumbnailColour::Rec709 && !self.camera_look
    }
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
    /// What the thumbnails got ("Rec.709", "None" or a LUT's name); `None`
    /// when the job asked for plain Rec.709 or no frame was rendered.
    pub colour: Option<String>,
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
        let result = std::panic::catch_unwind(|| {
            if hooks {
                test_hooks(&job);
            }
            run_job(&job)
        })
        .unwrap_or_else(|_| ClipResult {
            id: job.id,
            meta: None,
            codec: None,
            bytes: 0,
            thumbnails: vec![
                Err("Luna hit an internal error on this clip".into());
                job.strip as usize
            ],
            warnings: vec!["Luna hit an internal error reading this clip".into()],
            colour: None,
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
    if name.contains("__log__") {
        eprintln!("12:00:00.000 WARN  [x] hook: hello");
    }
    if name.contains("__crash__") {
        std::process::abort();
    }
    if name.contains("__panic__") {
        panic!("boom");
    }
    if name.contains("__hang__") {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    }
}

/// How far the outer thumbnails sit inside the clip. A clip's very first and
/// last frames are often a flash frame, a slate or the lens cap.
pub const OUTER_INSET_SECONDS: f64 = 1.0;

/// Frame numbers for a strip of `strip` thumbnails: evenly spaced from `inset`
/// frames after the start to `inset` frames before the end. The inset never
/// takes more than a tenth of the clip from either end.
pub fn strip_positions(frame_count: u64, strip: u32, inset: u64) -> Vec<u64> {
    if frame_count == 0 || strip == 0 {
        return Vec::new();
    }
    let inset = inset.min(frame_count / 10);
    let first = inset;
    if strip == 1 {
        return vec![first];
    }
    let span = frame_count - 1 - 2 * inset;
    (0..strip as u64)
        .map(|i| first + i * span / (strip as u64 - 1))
        .collect()
}

/// Runs one job to completion. Never panics on bad input files.
pub fn run_job(job: &ClipJob) -> ClipResult {
    let files = &job.files;
    let stem = files.main.file_stem().unwrap_or_default().to_string_lossy();
    let _clip = tracing::info_span!("clip", clip = %stem).entered();
    let bytes = std::iter::once(&files.main)
        .chain(&files.segments)
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let strip = job.strip as usize;
    let mut warnings = Vec::new();

    // Vendor formats (CRM, BRAW, R3D) are read through their decoders; the rest by our parsers.
    let (path, probe) = match luna_decode::vendor_probe(files) {
        Some(p) => ("vendor", p),
        None => ("own", luna_formats::probe(files)),
    };
    let probe = match probe {
        Ok(p) => p,
        Err(e) => {
            let reason = e.to_string();
            tracing::warn!(container = ?files.container, "probe failed: {path}: {reason}");
            return ClipResult {
                id: job.id,
                meta: None,
                codec: None,
                bytes,
                thumbnails: vec![Err(reason.clone()); strip],
                warnings: vec![reason],
                colour: None,
            };
        }
    };
    tracing::info!(
        container = ?files.container,
        codec = %probe.codec.label(),
        bytes,
        "probe: {path}"
    );
    for w in &probe.warnings {
        tracing::warn!("warning: {w}");
    }
    warnings.extend(probe.warnings.iter().cloned());
    log_meta(&probe.meta);

    let inset = probe
        .meta
        .frame_rate
        .map(|r| (r.as_f64() * OUTER_INSET_SECONDS).round() as u64)
        .unwrap_or(0);
    let positions = probe
        .meta
        .frame_count
        .map(|n| strip_positions(n, job.strip, inset))
        .unwrap_or_default();
    tracing::debug!(positions = ?positions, inset, "strip");
    let mut applied = None;
    let thumbnails = if positions.is_empty() {
        no_thumbnails(probe.meta.frame_count, job.strip)
    } else {
        let aspect = probe.layout.pixel_aspect.map(|r| (r.num, r.den));
        let turn = probe.layout.rotation;
        let mut unknown_transform = false;
        let lut = match job.colour.colour {
            ThumbnailColour::Lut => job.colour.lut.as_deref().map(|p| (p, load_lut(p))),
            _ => None,
        };
        let (colour, lut_name) = match (job.colour.colour, &lut) {
            (ThumbnailColour::AsRecorded, _) => (Colour::CameraLog, None),
            (ThumbnailColour::Lut, Some((path, Ok(l)))) => (Colour::Lut(l), Some(file_name(path))),
            (ThumbnailColour::Lut, Some((path, Err(e)))) => {
                let w = format!("The LUT {} can't be read: {e}", file_name(path));
                tracing::warn!("warning: {w}");
                warnings.push(w);
                (Colour::Rec709, None)
            }
            _ => (Colour::Rec709, None),
        };
        let thumbs = luna_decode::decode_frames(files, &probe, &positions, job.thumb_width)
            .into_iter()
            .zip(&positions)
            .map(|(frame, &position)| match frame {
                Ok(frame) => {
                    let (jpeg, unknown, got) =
                        thumbnail(&frame, position, job.thumb_width, aspect, turn, colour);
                    unknown_transform |= unknown;
                    applied = Some(colour_label(got, lut_name.as_deref()));
                    Ok(jpeg)
                }
                Err(e) => {
                    tracing::warn!("decode failed: frame {position}: {e}");
                    Err(e.to_string())
                }
            })
            .collect();
        if unknown_transform {
            let w = "Display transform unknown: thumbnails are shown as decoded";
            tracing::warn!("warning: {w}");
            warnings.push(w.into());
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
        colour: applied.filter(|_| !job.colour.is_plain()),
    }
}

/// The report's name for what a thumbnail got.
fn colour_label(applied: Applied, lut_name: Option<&str>) -> String {
    match (applied, lut_name) {
        (Applied::Rec709, _) => "Rec.709".into(),
        (Applied::CameraLog, _) => "None".into(),
        (Applied::Lut, Some(name)) => name.into(),
        (Applied::Lut, None) => "LUT".into(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The LUT at `path`, parsed once per worker process.
fn load_lut(path: &Path) -> Result<Arc<Lut>, String> {
    type Cached = Option<(PathBuf, Result<Arc<Lut>, String>)>;
    static CACHE: Mutex<Cached> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, lut)) = cache.as_ref()
        && p == path
    {
        return lut.clone();
    }
    let lut = std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| Lut::parse_cube(&text).map_err(|e| e.to_string()))
        .map(Arc::new);
    *cache = Some((path.to_path_buf(), lut.clone()));
    lut
}

/// The strip when no frame can be picked, with the reason logged. A probe-only
/// job (`strip` 0) asked for none, so it gets none and no warning.
fn no_thumbnails(frame_count: Option<u64>, strip: u32) -> Vec<Result<Vec<u8>, String>> {
    let reason = match frame_count {
        _ if strip == 0 => return Vec::new(),
        None => {
            tracing::warn!("no thumbnails: frame count unknown");
            "The clip's frame count is unknown"
        }
        Some(_) => {
            tracing::warn!("no thumbnails: no frames");
            "The clip has no frames"
        }
    };
    vec![Err(reason.into()); strip as usize]
}

/// Every metadata field as `name=value`, `missing` when the file doesn't say.
fn log_meta(m: &ClipMeta) {
    fn v<T: std::fmt::Display>(v: &Option<T>) -> String {
        v.as_ref()
            .map_or_else(|| "missing".into(), |v| v.to_string())
    }
    fn d<T: std::fmt::Debug>(v: &Option<T>) -> String {
        v.as_ref()
            .map_or_else(|| "missing".into(), |v| format!("{v:?}"))
    }
    tracing::debug!(
        recording_date = %v(&m.recording_date),
        start_timecode = %v(&m.start_timecode),
        frame_rate = %v(&m.frame_rate),
        frame_count = %v(&m.frame_count),
        width = %v(&m.width),
        height = %v(&m.height),
        camera_make = %v(&m.camera_make),
        camera_model = %v(&m.camera_model),
        camera_serial = %v(&m.camera_serial),
        lens = %v(&m.lens),
        focal_length = %v(&m.focal_length),
        stop = %v(&m.stop),
        iso = %v(&m.iso),
        shutter = %v(&m.shutter),
        white_balance = %v(&m.white_balance),
        nd = %v(&m.nd),
        sensor_fps = %v(&m.sensor_fps),
        audio = %v(&m.audio),
        embedded_reel = %v(&m.embedded_reel),
        embedded_camera_id = %v(&m.embedded_camera_id),
        scene = %v(&m.slate.scene),
        shot = %v(&m.slate.shot),
        take = %v(&m.slate.take),
        transfer = %d(&m.colour.transfer),
        gamut = %d(&m.colour.gamut),
        "meta"
    );
}

/// Renders one decoded frame to a JPEG and logs what went in and came out.
/// Also returns whether the display transform was unknown.
fn thumbnail(
    frame: &Frame,
    position: u64,
    width: u32,
    aspect: Option<(u32, u32)>,
    rotation: u16,
    colour: Colour,
) -> (Vec<u8>, bool, Applied) {
    let start = std::time::Instant::now();
    let mut rendered = luna_color::render_with(frame, width, aspect, colour);
    rendered.image = luna_color::rotate(&rendered.image, rotation);
    let render_ms = start.elapsed().as_secs_f64() * 1000.0;
    let stats = luna_color::luma_stats(&rendered.image);
    let (luma_min, luma_mean, luma_max) = (stats.min, stats.mean, stats.max);
    let frame = frame.describe();
    let unknown = rendered.transform_unknown;
    let fields = format_args!("thumb: frame {position}: {frame}");
    if thumb_event(&stats) == Level::WARN {
        tracing::warn!(
            render_ms,
            rotation,
            transform_unknown = unknown,
            luma_min,
            luma_mean,
            luma_max,
            "{fields} (near black)"
        );
    } else {
        tracing::debug!(
            render_ms,
            rotation,
            transform_unknown = unknown,
            luma_min,
            luma_mean,
            luma_max,
            "{fields}"
        );
    }
    (
        luna_color::encode_jpeg(&rendered.image),
        unknown,
        rendered.applied,
    )
}

/// WARN for a thumbnail that looks black, DEBUG otherwise.
fn thumb_event(stats: &LumaStats) -> Level {
    if stats.max <= NEAR_BLACK_MAX {
        Level::WARN
    } else {
        Level::DEBUG
    }
}

#[cfg(test)]
#[path = "../tests/common/collect.rs"]
mod collect;

#[cfg(test)]
mod tests {
    use super::*;
    use luna_decode::Decoded;
    use luna_formats::ColourHint;

    #[test]
    fn labels_name_what_was_applied() {
        use luna_color::Applied;
        assert_eq!(colour_label(Applied::Rec709, Some("show.cube")), "Rec.709");
        assert_eq!(colour_label(Applied::CameraLog, None), "None");
        assert_eq!(colour_label(Applied::Lut, Some("show.cube")), "show.cube");
    }

    fn display8(value: u8) -> Frame {
        Frame {
            image: Decoded::Display8 {
                width: 64,
                height: 36,
                data: vec![value; 64 * 36 * 3],
            },
            hint: ColourHint::default(),
        }
    }

    #[test]
    fn thumb_event_warns_only_when_near_black() {
        let stats = |max| LumaStats {
            min: 0,
            mean: 1.0,
            max,
        };
        assert_eq!(thumb_event(&stats(0)), Level::WARN);
        assert_eq!(thumb_event(&stats(NEAR_BLACK_MAX)), Level::WARN);
        assert_eq!(thumb_event(&stats(NEAR_BLACK_MAX + 1)), Level::DEBUG);
    }

    #[test]
    fn no_thumbnails_says_why_only_when_some_were_asked_for() {
        let (thumbs, events) = collect::capture(|| no_thumbnails(None, 0));
        assert!(thumbs.is_empty());
        assert!(events.is_empty(), "{events:#?}");

        let (thumbs, events) = collect::capture(|| no_thumbnails(Some(0), 0));
        assert!(thumbs.is_empty());
        assert!(events.is_empty(), "{events:#?}");

        let (thumbs, events) = collect::capture(|| no_thumbnails(None, 3));
        assert_eq!(
            thumbs,
            vec![Err("The clip's frame count is unknown".into()); 3]
        );
        assert_eq!(events.len(), 1, "{events:#?}");
        assert_eq!(events[0].level, Level::WARN);
        assert!(
            events[0]
                .text
                .contains("no thumbnails: frame count unknown")
        );

        let (thumbs, events) = collect::capture(|| no_thumbnails(Some(0), 3));
        assert_eq!(thumbs, vec![Err("The clip has no frames".into()); 3]);
        assert_eq!(events.len(), 1, "{events:#?}");
        assert_eq!(events[0].level, Level::WARN);
        assert!(events[0].text.contains("no thumbnails: no frames"));
    }

    #[test]
    fn black_thumbnail_logs_warn() {
        let (jpeg, events) =
            collect::capture(|| thumbnail(&display8(0), 24, 640, None, 0, Colour::Rec709));
        assert!(!jpeg.0.is_empty());
        let thumb: Vec<_> = events
            .iter()
            .filter(|e| e.text.contains("thumb: frame 24"))
            .collect();
        assert_eq!(thumb.len(), 1, "{events:#?}");
        assert_eq!(thumb[0].level, Level::WARN);
        assert_eq!(thumb[0].clip, None);
        for field in [
            "luma_min=0",
            "luma_mean=0",
            "luma_max=0",
            "render_ms=",
            "Display8 64x36",
        ] {
            assert!(thumb[0].text.contains(field), "{field}: {thumb:?}");
        }

        let (_, events) =
            collect::capture(|| thumbnail(&display8(128), 24, 640, None, 0, Colour::Rec709));
        assert!(
            events
                .iter()
                .any(|e| e.level == Level::DEBUG && e.text.contains("luma_max=128")),
            "{events:#?}"
        );
    }
}
