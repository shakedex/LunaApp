//! The shared vocabulary between scanning, parsing, decoding and the report.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What one file is, judged by its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sniffed {
    CameraOriginal(Container),
    Sidecar(SidecarKind),
    Other,
}

/// The container family of a camera original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Container {
    /// SMPTE MXF (ARRI, Sony, Canon XF-AVC, ...).
    Mxf,
    /// QuickTime `.mov` and ISO-BMFF `.mp4` (ftyp/moov/mdat family), excluding CRM.
    QuickTime,
    /// Canon Cinema RAW Light `.crm` (CR3-style ISO-BMFF with a `crx ` brand).
    Crm,
    /// Blackmagic RAW `.braw`.
    Braw,
    /// RED `.R3D` (one segment of a possibly spanned clip).
    R3d,
    /// One frame of an ARRIRAW `.ari` / `.arx` frame sequence.
    AriFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SidecarKind {
    /// Sony `...M01.XML` NonRealTimeMeta.
    SonyXml,
    /// Canon clip `.XML`.
    CanonXml,
    /// RED `.RMD`.
    RedRmd,
    /// Blackmagic `.sidecar`.
    BrawSidecar,
    /// Avid Log Exchange.
    Ale,
}

/// The files that make one clip. `main` is what the camera wrote first;
/// `segments` are the rest of a spanned clip in order (R3D `_002…`, `.ari` frames);
/// `sidecars` carry extra metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipFiles {
    pub container: Container,
    pub main: PathBuf,
    pub segments: Vec<PathBuf>,
    pub sidecars: Vec<(SidecarKind, PathBuf)>,
}

/// The coding inside the container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    ProRes(ProResProfile),
    ProResRaw,
    XOcn,
    ArriRaw,
    ArriCore,
    H264,
    Hevc,
    Braw,
    Redcode,
    CinemaRawLight,
    /// Anything else, named as the container names it.
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProResProfile {
    Proxy,
    Lt,
    Standard,
    Hq,
    P4444,
    P4444Xq,
}

impl Codec {
    /// The name printed in the report's Codec column.
    pub fn label(&self) -> String {
        match self {
            Codec::ProRes(p) => format!(
                "ProRes {}",
                match p {
                    ProResProfile::Proxy => "422 Proxy",
                    ProResProfile::Lt => "422 LT",
                    ProResProfile::Standard => "422",
                    ProResProfile::Hq => "422 HQ",
                    ProResProfile::P4444 => "4444",
                    ProResProfile::P4444Xq => "4444 XQ",
                }
            ),
            Codec::ProResRaw => "ProRes RAW".into(),
            Codec::XOcn => "X-OCN".into(),
            Codec::ArriRaw => "ARRIRAW".into(),
            Codec::ArriCore => "ARRICORE".into(),
            Codec::H264 => "H.264".into(),
            Codec::Hevc => "H.265/HEVC".into(),
            Codec::Braw => "Blackmagic RAW".into(),
            Codec::Redcode => "REDCODE RAW".into(),
            Codec::CinemaRawLight => "Cinema RAW Light".into(),
            Codec::Other(s) => s.clone(),
        }
    }
}

/// A frame rate or other exact ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rational {
    pub num: u32,
    pub den: u32,
}

impl Rational {
    pub fn as_f64(self) -> f64 {
        if self.den == 0 {
            0.0
        } else {
            self.num as f64 / self.den as f64
        }
    }
}

impl fmt::Display for Rational {
    /// "24", "23.976", "29.97", "59.94".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.as_f64();
        if (v - v.round()).abs() < 1e-9 {
            write!(f, "{}", v.round() as u64)
        } else {
            let s = format!("{v:.3}");
            write!(f, "{}", s.trim_end_matches('0').trim_end_matches('.'))
        }
    }
}

/// A SMPTE timecode as a frame count at an integer timebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timecode {
    /// Frames since 00:00:00:00 at `base` frames per second.
    pub frames: u64,
    /// The integer counting rate (24, 25, 30, 50, 60 …; 30 for 29.97).
    pub base: u32,
    pub drop_frame: bool,
}

impl Timecode {
    pub fn offset(self, frames: u64) -> Timecode {
        Timecode {
            frames: self.frames + frames,
            ..self
        }
    }
}

impl fmt::Display for Timecode {
    /// "HH:MM:SS:FF", with ';' before frames for drop-frame. Wraps at 24 h.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let base = self.base.max(1) as u64;
        let mut n = self.frames;
        if self.drop_frame && (base == 30 || base == 60) {
            // SMPTE 12M drop-frame: skip 2 (or 4) frame numbers each minute
            // except every tenth minute.
            let drop = if base == 30 { 2 } else { 4 };
            let per_10min = base * 600 - drop * 9;
            let per_min = base * 60 - drop;
            let tens = n / per_10min;
            let rem = n % per_10min;
            let extra = if rem < drop {
                0
            } else {
                drop * ((rem - drop) / per_min)
            };
            n += drop * 9 * tens + extra;
        }
        let ff = n % base;
        let total_s = n / base;
        let ss = total_s % 60;
        let mm = (total_s / 60) % 60;
        let hh = (total_s / 3600) % 24;
        let sep = if self.drop_frame { ';' } else { ':' };
        write!(f, "{hh:02}:{mm:02}:{ss:02}{sep}{ff:02}")
    }
}

/// Transfer function a picture was recorded with, as far as the file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transfer {
    Rec709,
    /// ARRI LogC3 (ALEXA classic/Mini/LF/65); EI-dependent but EI 800 is the reference.
    LogC3,
    /// ARRI LogC4 (ALEXA 35).
    LogC4,
    SLog3,
    CanonLog2,
    CanonLog3,
    VLog,
    DLog,
    /// Blackmagic Film Gen 5.
    BmdFilmGen5,
    /// RED Log3G10.
    Log3G10,
    Hlg,
    Pq,
    /// Scene-linear (raw mosaics after linearisation).
    Linear,
}

/// Colour primaries a picture was recorded with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gamut {
    Rec709,
    Rec2020,
    ArriWideGamut3,
    ArriWideGamut4,
    SGamut3,
    SGamut3Cine,
    CinemaGamut,
    VGamut,
    DGamut,
    BmdWideGamutGen5,
    RedWideGamutRgb,
}

/// What the file says about how to display the picture. `None` means the file
/// doesn't say, and the report labels the thumbnail "display transform unknown".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColourHint {
    pub transfer: Option<Transfer>,
    pub gamut: Option<Gamut>,
}

/// Every per-clip value the report shows, plus identity and colour hints.
/// All text is display-ready; `None` means the file doesn't carry it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipMeta {
    /// Local recording date, "YYYY-MM-DD" (time appended as " HH:MM:SS" when known).
    pub recording_date: Option<String>,
    pub start_timecode: Option<Timecode>,
    pub frame_rate: Option<Rational>,
    pub frame_count: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub camera_serial: Option<String>,
    pub lens: Option<String>,
    /// e.g. "47 mm".
    pub focal_length: Option<String>,
    /// e.g. "T2.8" or "f/4".
    pub stop: Option<String>,
    /// e.g. "800".
    pub iso: Option<String>,
    /// e.g. "180°" or "1/50".
    pub shutter: Option<String>,
    /// e.g. "5600 K".
    pub white_balance: Option<String>,
    /// e.g. "0.6" or "ND 1.2".
    pub nd: Option<String>,
    /// Identity from inside the file (step 1 of the identity chain).
    pub embedded_reel: Option<String>,
    /// Camera letter or ID as the camera records it (e.g. "A").
    pub embedded_camera_id: Option<String>,
    pub colour: ColourHint,
}

/// Where the coded frames of a clip sit, for decoders that read them from our index.
/// Decoders that open the file themselves (vendor SDKs, X-OCN, CRM, platform
/// decoders) ignore this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameIndex {
    /// Byte ranges of each coded frame in `ClipFiles::main`, in display order.
    Ranges(Vec<ByteRange>),
    /// One file per frame (`.ari` sequences), in display order.
    Files(Vec<PathBuf>),
    /// The decoder opens the file itself.
    DecoderOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub offset: u64,
    pub len: u64,
}

/// Picture geometry the decoders need beyond the codec's own headers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PictureLayout {
    pub stored_width: u32,
    pub stored_height: u32,
    /// Bits per raw sample for uncompressed raw (ARRIRAW 12/13).
    pub raw_bits: Option<u8>,
    /// Pixel aspect ratio (anamorphic squeeze), 1/1 when square.
    pub pixel_aspect: Option<Rational>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    pub codec: Codec,
    pub meta: ClipMeta,
    pub frames: FrameIndex,
    pub layout: PictureLayout,
    /// The codec's out-of-band configuration when the container carries one
    /// (QuickTime `avcC` / `hvcC` record body), for decoders that need it.
    #[serde(default)]
    pub codec_config: Option<Vec<u8>>,
    /// Things the parser noticed but could not settle, for the report's clip warnings.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatError {
    Io(String),
    /// The structure is broken or truncated.
    Corrupt(String),
    /// Recognised, but this parser doesn't read it (yet).
    Unsupported(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Io(m) => write!(f, "could not read the file: {m}"),
            FormatError::Corrupt(m) => write!(f, "the file is damaged: {m}"),
            FormatError::Unsupported(m) => write!(f, "not supported: {m}"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<std::io::Error> for FormatError {
    fn from(e: std::io::Error) -> Self {
        FormatError::Io(e.to_string())
    }
}
