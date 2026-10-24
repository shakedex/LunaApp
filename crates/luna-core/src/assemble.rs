//! Turning worker results into report rows, and writing the three report files.

use std::io;
use std::path::{Component, Path, PathBuf};

use luna_formats::{ClipMeta, Codec, Timecode};
use luna_report::{ClipFields, Thumbnail};

/// One report row's values from what the worker read. Nothing is guessed:
/// anything the file doesn't say stays `None`.
pub fn clip_fields(
    file_name: &str,
    meta: Option<&ClipMeta>,
    codec: Option<&Codec>,
    bytes: u64,
) -> ClipFields {
    let mut f = ClipFields {
        file_name: file_name.to_owned(),
        codec: codec.map(Codec::label),
        file_size: Some(bytes),
        ..ClipFields::default()
    };
    let Some(m) = meta else {
        return f;
    };
    let base = m
        .start_timecode
        .map(|tc| tc.base)
        .or_else(|| m.frame_rate.map(|r| r.as_f64().round() as u32))
        .filter(|b| *b > 0);
    if let Some(tc) = m.start_timecode {
        f.tc_in = Some(tc.to_string());
        if let Some(n) = m.frame_count.filter(|n| *n > 0) {
            f.tc_out = Some(tc.offset(n - 1).to_string());
        }
    }
    if let (Some(n), Some(base)) = (m.frame_count, base) {
        let duration = Timecode {
            frames: n,
            base,
            drop_frame: false,
        };
        f.duration = Some(duration.to_string());
    }
    f.recording_date = m.recording_date.clone();
    f.fps = m.frame_rate.map(|r| r.to_string());
    f.sensor_fps = m.sensor_fps.map(|r| r.to_string());
    f.resolution = match (m.width, m.height) {
        (Some(w), Some(h)) => Some(format!("{w}×{h}")),
        _ => None,
    };
    f.camera_model = match (&m.camera_make, &m.camera_model) {
        (Some(make), Some(model)) if !names_maker(model, make) => Some(format!("{make} {model}")),
        (_, Some(model)) => Some(model.clone()),
        (Some(make), None) => Some(make.clone()),
        (None, None) => None,
    };
    f.camera_serial = m.camera_serial.clone();
    f.lens = m.lens.clone();
    f.focal_length = m.focal_length.clone();
    f.stop = m.stop.clone();
    f.iso = m.iso.clone();
    f.shutter = m.shutter.clone();
    f.white_balance = m.white_balance.clone();
    f.nd = m.nd.clone();
    f.audio = m.audio.map(|a| a.to_string());
    f
}

/// The model name already carries the maker: "ARRI ALEXA Mini" for ARRI, or
/// "Blackmagic Cinema Camera 6K" for "Blackmagic Design".
pub(crate) fn names_maker(model: &str, make: &str) -> bool {
    let model = model.to_lowercase();
    let make = make.to_lowercase();
    model.contains(&make)
        || make
            .split_whitespace()
            .next()
            .is_some_and(|w| model.contains(w))
}

/// Worker thumbnails as report thumbnails. Bytes that don't decode become a reason tile.
pub fn thumbnails(results: Vec<Result<Vec<u8>, String>>) -> Vec<Thumbnail> {
    results
        .into_iter()
        .map(|r| match r {
            Ok(jpeg) => Thumbnail::from_jpeg(jpeg),
            Err(reason) => Thumbnail::Missing { reason },
        })
        .collect()
}

/// `<output_dir>/<name>`, or `<output_dir>/<production> <stamp>` without a
/// name, with characters no file system accepts
/// replaced, and " (2)", " (3)"… appended when the folder already exists.
pub fn output_folder(
    output_dir: &Path,
    production: &str,
    name: Option<&str>,
    stamp: &str,
) -> PathBuf {
    let given = name.map(|n| sanitize(n.trim())).filter(|n| !n.is_empty());
    let production = sanitize(production.trim());
    let base = match given {
        Some(n) => n,
        None if production.is_empty() => format!("Luna Report {stamp}"),
        None => format!("{production} {stamp}"),
    };
    let mut candidate = output_dir.join(&base);
    let mut n = 2;
    while candidate.exists() {
        candidate = output_dir.join(format!("{base} ({n})"));
        n += 1;
    }
    candidate
}

/// The root of the drive `path` is on: `D:\`, `\\server\share\`,
/// `/Volumes/CARD`, or `/` for the startup disk.
pub fn drive_root(path: &Path) -> PathBuf {
    let root = path.ancestors().last().unwrap_or(path);
    if root == Path::new("/")
        && let Ok(rest) = path.strip_prefix("/Volumes")
        && let Some(Component::Normal(name)) = rest.components().next()
    {
        return Path::new("/Volumes").join(name);
    }
    root.to_path_buf()
}

/// The drives that get a copy of a report saved in `report`: the drive of each
/// source root, once each, except the report's own drive. The macOS startup
/// disk (`/`) is left out: its system volume is read-only, and footage on the
/// Mac's own disk has no card to carry a copy.
pub fn copy_drives(report: &Path, roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut drives: Vec<PathBuf> = Vec::new();
    for root in roots {
        let drive = drive_root(dunce::simplified(root));
        let startup = cfg!(unix) && drive == Path::new("/");
        if !startup && !same_drive(root, report) && !drives.iter().any(|d| same_drive(d, root)) {
            drives.push(drive);
        }
    }
    drives
}

/// Whether two paths are on the same drive. Drive letters and share names
/// compare without case.
pub fn same_drive(a: &Path, b: &Path) -> bool {
    let key = |p: &Path| {
        let root = drive_root(dunce::simplified(p));
        if cfg!(windows) {
            root.to_string_lossy().to_lowercase()
        } else {
            root.to_string_lossy().into_owned()
        }
    };
    key(a) == key(b)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim_end_matches(['.', ' '])
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFiles {
    pub folder: PathBuf,
    pub pdf: PathBuf,
    pub html: PathBuf,
    pub csv: PathBuf,
}

/// Writes the three files into a hidden temporary folder beside `target`, then
/// renames it into place, so a failed run leaves no partial report.
pub fn write_outputs(
    target: &Path,
    stem: &str,
    pdf: &[u8],
    html: &[u8],
    csv: &[u8],
) -> io::Result<OutputFiles> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("output folder has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".luna-writing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir(&tmp)?;
    let write_all = || -> io::Result<()> {
        std::fs::write(tmp.join(format!("{stem}.pdf")), pdf)?;
        std::fs::write(tmp.join(format!("{stem}.html")), html)?;
        std::fs::write(tmp.join(format!("{stem}.csv")), csv)?;
        std::fs::rename(&tmp, target)
    };
    if let Err(e) = write_all() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    Ok(OutputFiles {
        folder: target.to_path_buf(),
        pdf: target.join(format!("{stem}.pdf")),
        html: target.join(format!("{stem}.html")),
        csv: target.join(format!("{stem}.csv")),
    })
}
