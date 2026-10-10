//! Turning worker results into report rows, and writing the three report files.

use std::io;
use std::path::{Path, PathBuf};

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
    f.resolution = match (m.width, m.height) {
        (Some(w), Some(h)) => Some(format!("{w}×{h}")),
        _ => None,
    };
    f.camera_model = match (&m.camera_make, &m.camera_model) {
        (Some(make), Some(model)) if !model.to_lowercase().contains(&make.to_lowercase()) => {
            Some(format!("{make} {model}"))
        }
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
    f
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

/// `<output_dir>/<production> <stamp>`, with characters no file system accepts
/// replaced, and " (2)", " (3)"… appended when the folder already exists.
pub fn output_folder(output_dir: &Path, production: &str, stamp: &str) -> PathBuf {
    let name = sanitize(production.trim());
    let base = if name.is_empty() {
        format!("Luna Report {stamp}")
    } else {
        format!("{name} {stamp}")
    };
    let mut candidate = output_dir.join(&base);
    let mut n = 2;
    while candidate.exists() {
        candidate = output_dir.join(format!("{base} ({n})"));
        n += 1;
    }
    candidate
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
