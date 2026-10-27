//! The report flattened into display strings. HTML and PDF both render this, so
//! they can't disagree. Each writer only decides how an image is referenced.

use serde::Serialize;

use crate::logo::LogoFormat;
use crate::model::{
    Card, Clip, ClipFields, CustomField, IdentitySource, MISSING, Report, ReportTheme, Thumbnail,
    Totals, format_bytes,
};

/// An image a writer must make referable: return the string the template uses as its source.
pub(crate) enum ImageRef<'a> {
    Logo(LogoFormat, &'a [u8]),
    Thumb(&'a [u8]),
}

#[derive(Serialize)]
pub(crate) struct View {
    pub theme: ReportTheme,
    /// The production name, or "Camera report" when Settings has none.
    pub title: String,
    /// "Camera report" under a production name; absent when it is the title.
    pub subtitle: Option<String>,
    pub header: HeaderView,
    pub source_root: String,
    pub generated_at: String,
    pub luna_version: String,
    pub totals: Totals,
    pub show_problems: bool,
    pub total_size: String,
    pub cameras: Vec<CameraView>,
    pub not_included: Vec<String>,
    pub unreadable: Vec<UnreadableView>,
}

#[derive(Serialize)]
pub(crate) struct HeaderView {
    pub company: Option<String>,
    pub author: Option<String>,
    pub logo: Option<String>,
    pub custom: Vec<CustomField>,
}

#[derive(Serialize)]
pub(crate) struct CameraView {
    pub name: String,
    /// e.g. "2 cards, 3 clips, 12.88 GB".
    pub summary: String,
    pub cards: Vec<CardView>,
}

#[derive(Serialize)]
pub(crate) struct CardView {
    pub reel: String,
    /// e.g. "2 clips, 8.59 GB".
    pub summary: String,
    pub clips: Vec<ClipView>,
}

#[derive(Serialize)]
pub(crate) struct ClipView {
    pub name: String,
    pub recorded: Option<String>,
    /// Three columns of label/value cells: time and format, camera and optics,
    /// exposure and the rest.
    pub groups: [Vec<Cell>; 3],
    pub thumbs: Vec<ThumbView>,
    /// Where the camera and reel names came from, only when they were guessed
    /// from the file or folder rather than read from the clip.
    pub identity: Option<String>,
    pub warnings: Vec<String>,
    pub problem: bool,
    /// The sensor ran at a different rate from playback.
    pub off_speed: bool,
}

#[derive(Serialize)]
pub(crate) struct Cell {
    pub label: &'static str,
    /// [`MISSING`] when the file doesn't say.
    pub value: String,
    pub missing: bool,
    /// Worth a second look: an off-speed frame rate.
    pub accent: bool,
}

/// A thumbnail, or `None` for a frame that couldn't be shown; the reason is
/// in the clip's warnings.
#[derive(Serialize)]
pub(crate) struct ThumbView {
    pub src: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct UnreadableView {
    pub kind: &'static str,
    pub path: String,
    pub reason: String,
}

pub(crate) fn build(report: &Report, image_ref: &mut dyn FnMut(ImageRef) -> String) -> View {
    let h = &report.header;
    let logo = h
        .logo
        .as_ref()
        .map(|l| image_ref(ImageRef::Logo(l.format(), l.bytes())));
    let totals = report.totals();

    let cameras = report
        .cameras
        .iter()
        .map(|camera| {
            let clips: Vec<&Clip> = camera.cards.iter().flat_map(|c| &c.clips).collect();
            CameraView {
                name: camera.name.clone(),
                summary: format!(
                    "{}, {}, {}",
                    count(camera.cards.len(), "card", "cards"),
                    count(clips.len(), "clip", "clips"),
                    format_bytes(bytes(clips.iter().copied())),
                ),
                cards: camera
                    .cards
                    .iter()
                    .map(|card| card_view(card, report.show_problems, image_ref))
                    .collect(),
            }
        })
        .collect();

    let (title, subtitle) = match &h.production {
        Some(p) => (p.clone(), Some("Camera report".to_owned())),
        None => ("Camera report".to_owned(), None),
    };

    View {
        theme: report.theme,
        title,
        subtitle,
        header: HeaderView {
            company: h.company.clone(),
            author: h.author.clone(),
            logo,
            custom: h.custom.clone(),
        },
        source_root: report.sources.join(", "),
        generated_at: report.generated_at.clone(),
        luna_version: report.luna_version.clone(),
        totals,
        show_problems: report.show_problems,
        total_size: format_bytes(totals.bytes),
        cameras,
        not_included: report.not_included.clone(),
        unreadable: report
            .unreadable
            .iter()
            .filter(|_| report.show_problems)
            .map(|u| UnreadableView {
                kind: u.kind.label(),
                path: u.path.clone(),
                reason: u.reason.clone(),
            })
            .collect(),
    }
}

fn card_view(
    card: &Card,
    show_problems: bool,
    image_ref: &mut dyn FnMut(ImageRef) -> String,
) -> CardView {
    CardView {
        reel: card.reel.clone(),
        summary: format!(
            "{}, {}",
            count(card.clips.len(), "clip", "clips"),
            format_bytes(bytes(card.clips.iter())),
        ),
        clips: card
            .clips
            .iter()
            .map(|clip| clip_view(clip, show_problems, image_ref))
            .collect(),
    }
}

fn clip_view(
    clip: &Clip,
    show_problems: bool,
    image_ref: &mut dyn FnMut(ImageRef) -> String,
) -> ClipView {
    let f = &clip.fields;
    let off_speed = is_off_speed(f);
    let fps = match (&f.fps, &f.sensor_fps) {
        (Some(fps), Some(sensor)) if off_speed => Some(format!("{fps} (sensor {sensor})")),
        (Some(fps), _) => Some(fps.clone()),
        (None, Some(sensor)) => Some(format!("sensor {sensor}")),
        (None, None) => None,
    };
    let cell = |label, value: Option<String>| Cell {
        label,
        missing: value.is_none(),
        value: value.unwrap_or_else(|| MISSING.to_owned()),
        accent: false,
    };
    let mut fps_cell = cell("FPS", fps);
    fps_cell.accent = off_speed;
    let mut optics = vec![
        cell("Camera", f.camera_model.clone()),
        cell("Serial", f.camera_serial.clone()),
        cell("Lens", f.lens.clone()),
        cell("Focal length", f.focal_length.clone()),
        cell("T/f-stop", f.stop.clone()),
    ];
    if let Some(slate) = slate(f) {
        optics.push(cell("Slate", Some(slate)));
    }

    ClipView {
        name: f.file_name.clone(),
        recorded: f.recording_date.clone(),
        groups: [
            vec![
                cell("TC in", f.tc_in.clone()),
                cell("TC out", f.tc_out.clone()),
                cell("Duration", f.duration.clone()),
                fps_cell,
                cell("Resolution", f.resolution.clone()),
                cell("Codec", f.codec.clone()),
            ],
            optics,
            vec![
                cell("ISO/EI", f.iso.clone()),
                cell("Shutter", f.shutter.clone()),
                cell("WB", f.white_balance.clone()),
                cell("ND", f.nd.clone()),
                cell("Audio", f.audio.clone()),
                cell("File size", f.file_size.map(format_bytes)),
            ],
        ],
        thumbs: clip
            .thumbnails
            .iter()
            .map(|t| ThumbView {
                src: match t {
                    Thumbnail::Jpeg(jpeg) => Some(image_ref(ImageRef::Thumb(jpeg.bytes()))),
                    Thumbnail::Missing { .. } => None,
                },
            })
            .collect(),
        identity: guessed_identity(clip),
        warnings: if show_problems {
            warnings(clip)
        } else {
            Vec::new()
        },
        problem: show_problems && clip.has_problems(),
        off_speed,
    }
}

/// "Sc 12 · Sh 3 · Tk 2" from whichever parts the file has; `None` without any.
fn slate(f: &ClipFields) -> Option<String> {
    let parts: Vec<String> = [("Sc", &f.scene), ("Sh", &f.shot), ("Tk", &f.take)]
        .into_iter()
        .filter_map(|(tag, v)| v.as_ref().map(|v| format!("{tag} {v}")))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn guessed_identity(clip: &Clip) -> Option<String> {
    let (c, r) = (clip.camera_source, clip.reel_source);
    match (c == IdentitySource::Metadata, r == IdentitySource::Metadata) {
        (true, true) => None,
        _ if c == r => Some(format!("Camera and reel named from the {}", c.label())),
        (true, false) => Some(format!("Reel named from the {}", r.label())),
        (false, true) => Some(format!("Camera named from the {}", c.label())),
        (false, false) => Some(format!(
            "Camera named from the {}, reel from the {}",
            c.label(),
            r.label()
        )),
    }
}

/// The clip's warnings, then each thumbnail failure not already said, once.
fn warnings(clip: &Clip) -> Vec<String> {
    let mut out = clip.warnings.clone();
    for t in &clip.thumbnails {
        if let Thumbnail::Missing { reason } = t
            && !out.iter().any(|w| w == reason)
        {
            out.push(reason.clone());
        }
    }
    out
}

/// Both rates are known and they differ. Compared as numbers, so "25" and
/// "25.000" agree.
fn is_off_speed(f: &ClipFields) -> bool {
    let rate = |s: &Option<String>| s.as_deref().and_then(|v| v.trim().parse::<f64>().ok());
    match (rate(&f.fps), rate(&f.sensor_fps)) {
        (Some(a), Some(b)) => (a - b).abs() > 0.001,
        _ => false,
    }
}

fn bytes<'a>(clips: impl Iterator<Item = &'a Clip>) -> u64 {
    clips.map(|c| c.fields.file_size.unwrap_or(0)).sum()
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
