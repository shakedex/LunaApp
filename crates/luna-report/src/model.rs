//! The report model: everything the writers render, and nothing they compute
//! beyond formatting.

use serde::{Deserialize, Serialize};
use typst::foundations::Bytes;
use typst::visualize::{ExchangeFormat, RasterImage};

use crate::logo::Logo;

/// Printed wherever a value is missing. Never guessed, never borrowed from another clip.
pub const MISSING: &str = "—";

/// Per-clip columns, in report order.
pub const FIELD_LABELS: [&str; 20] = [
    "File name",
    "Recording date",
    "TC in",
    "TC out",
    "Duration",
    "FPS",
    "Sensor FPS",
    "Resolution",
    "Codec",
    "Camera model",
    "Camera serial",
    "Lens",
    "Focal length",
    "T/f-stop",
    "ISO/EI",
    "Shutter",
    "WB",
    "ND",
    "Audio",
    "File size",
];

/// Which step of the identity chain named a clip's camera or reel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    Metadata,
    FileName,
    Folder,
    FolderName,
    /// Not named by the clip itself; taken from the other clips on its card.
    Card,
}

impl IdentitySource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Metadata => "clip metadata",
            Self::FileName => "file name",
            Self::Folder => "folder structure",
            Self::FolderName => "folder name",
            Self::Card => "other clips on the card",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CustomField {
    pub label: String,
    pub value: String,
}

/// Report details from Settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    pub production: Option<String>,
    pub company: Option<String>,
    pub author: Option<String>,
    pub logo: Option<Logo>,
    pub custom: Vec<CustomField>,
}

/// Paper for printing, or dark for reading on a screen on set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub header: Header,
    pub theme: ReportTheme,
    /// The source folders as given.
    pub sources: Vec<String>,
    /// Already formatted for display, e.g. "2026-10-10 14:32".
    pub generated_at: String,
    pub luna_version: String,
    pub cameras: Vec<Camera>,
    /// Paths relative to the source root.
    pub not_included: Vec<String>,
    pub unreadable: Vec<Unreadable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Camera {
    pub name: String,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub reel: String,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub fields: ClipFields,
    pub camera_source: IdentitySource,
    pub reel_source: IdentitySource,
    /// One entry per strip position, in strip order.
    pub thumbnails: Vec<Thumbnail>,
    pub warnings: Vec<String>,
}

impl Clip {
    pub fn has_problems(&self) -> bool {
        !self.warnings.is_empty()
            || self
                .thumbnails
                .iter()
                .any(|t| matches!(t, Thumbnail::Missing { .. }))
    }
}

/// Display-ready values. `None` prints as [`MISSING`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipFields {
    pub file_name: String,
    pub recording_date: Option<String>,
    pub tc_in: Option<String>,
    pub tc_out: Option<String>,
    pub duration: Option<String>,
    pub fps: Option<String>,
    /// Only when the file records the sensor rate apart from the playback rate.
    pub sensor_fps: Option<String>,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub camera_model: Option<String>,
    pub camera_serial: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<String>,
    pub stop: Option<String>,
    pub iso: Option<String>,
    pub shutter: Option<String>,
    pub white_balance: Option<String>,
    pub nd: Option<String>,
    /// e.g. "4 ch, 48 kHz, 24-bit", or "None" for a clip without sound.
    pub audio: Option<String>,
    pub file_size: Option<u64>,
}

impl ClipFields {
    /// Values in [`FIELD_LABELS`] order. The file size is formatted with [`format_bytes`].
    pub fn display_values(&self) -> [Option<String>; 20] {
        [
            Some(self.file_name.clone()),
            self.recording_date.clone(),
            self.tc_in.clone(),
            self.tc_out.clone(),
            self.duration.clone(),
            self.fps.clone(),
            self.sensor_fps.clone(),
            self.resolution.clone(),
            self.codec.clone(),
            self.camera_model.clone(),
            self.camera_serial.clone(),
            self.lens.clone(),
            self.focal_length.clone(),
            self.stop.clone(),
            self.iso.clone(),
            self.shutter.clone(),
            self.white_balance.clone(),
            self.nd.clone(),
            self.audio.clone(),
            self.file_size.map(format_bytes),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Thumbnail {
    Jpeg(Jpeg),
    /// Same footprint as an image, with the reason printed in it.
    Missing {
        reason: String,
    },
}

impl Thumbnail {
    /// A thumbnail that decodes, or a reason tile: one bad image must never sink the report.
    pub fn from_jpeg(bytes: Vec<u8>) -> Self {
        match RasterImage::plain(Bytes::new(bytes.clone()), ExchangeFormat::Jpg) {
            Ok(_) => Self::Jpeg(Jpeg(bytes)),
            Err(_) => Self::Missing {
                reason: "Thumbnail could not be read".into(),
            },
        }
    }
}

/// JPEG bytes known to decode. Only [`Thumbnail::from_jpeg`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jpeg(Vec<u8>);

impl Jpeg {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadableKind {
    File,
    Folder,
}

impl UnreadableKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Folder => "Folder",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// Relative to the source root.
    pub path: String,
    pub kind: UnreadableKind,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub cameras: usize,
    pub cards: usize,
    pub clips: usize,
    pub clips_with_problems: usize,
    pub bytes: u64,
}

impl Report {
    pub fn totals(&self) -> Totals {
        let mut t = Totals {
            cameras: self.cameras.len(),
            ..Totals::default()
        };
        for card in self.cameras.iter().flat_map(|c| &c.cards) {
            t.cards += 1;
            for clip in &card.clips {
                t.clips += 1;
                t.clips_with_problems += usize::from(clip.has_problems());
                t.bytes += clip.fields.file_size.unwrap_or(0);
            }
        }
        t
    }
}

/// Decimal units, two decimals above bytes: 4_294_967_296 → "4.29 GB".
pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["kB", "MB", "GB", "TB", "PB"];
    if n < 1000 {
        return format!("{n} B");
    }
    let mut value = n as f64;
    for (i, unit) in UNITS.iter().enumerate() {
        value /= 1000.0;
        let shown = format!("{value:.2}");
        let rolls_over = shown.parse::<f64>().unwrap_or(0.0) >= 1000.0;
        if !rolls_over || i == UNITS.len() - 1 {
            return format!("{shown} {unit}");
        }
    }
    unreachable!("the last unit always returns")
}
