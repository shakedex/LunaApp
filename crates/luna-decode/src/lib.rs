//! Decoding a few frames of a clip for thumbnails.
//!
//! One entry point, [`decode_frames`], picks a backend from the probed codec:
//! our own decoders (ProRes, ARRIRAW, X-OCN, CRM), bundled vendor SDKs (BRAW;
//! R3D through the closed add-on loaded at runtime), or the platform decoder
//! (Media Foundation on Windows, AVFoundation on macOS). Platform APIs live only
//! in this crate, behind `cfg(target_os)`.

use luna_formats::{ClipFiles, ColourHint, Gamut, Probe, Transfer};

/// The colour filter array layout of a mosaic, naming the top-left 2×2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cfa {
    Rggb,
    Grbg,
    Gbrg,
    Bggr,
}

/// A decoded picture, in whatever form its decoder produces. `luna-color` turns
/// any of these into a display-referred Rec.709 thumbnail.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// A raw sensor mosaic. Samples are linear after subtracting `black`;
    /// `white` is the clip point. `wb` are R,G,B multipliers when known.
    Mosaic {
        width: u32,
        height: u32,
        cfa: Cfa,
        data: Vec<u16>,
        black: f32,
        white: f32,
        wb: Option<[f32; 3]>,
        /// 3×3 camera-RGB → XYZ (D65), row-major, when the decoder knows it.
        camera_to_xyz: Option<[f32; 9]>,
    },
    /// Interleaved RGB, 16-bit, encoded with `transfer` in `gamut`.
    Rgb16 {
        width: u32,
        height: u32,
        data: Vec<u16>,
        transfer: Transfer,
        gamut: Gamut,
    },
    /// Interleaved RGB, 8-bit, already display-referred Rec.709 (e.g. a vendor
    /// SDK's own Rec.709 output). Passed through untouched.
    Display8 {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    /// Planar Y′CbCr as coded. `bits` is the sample depth (8–16) held in u16.
    /// Chroma planes are `chroma_width × chroma_height`.
    Ycbcr {
        width: u32,
        height: u32,
        chroma_width: u32,
        chroma_height: u32,
        bits: u8,
        y: Vec<u16>,
        cb: Vec<u16>,
        cr: Vec<u16>,
        matrix: YcbcrMatrix,
        full_range: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YcbcrMatrix {
    Bt601,
    Bt709,
    Bt2020,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// No decoder for this codec on this platform. The message names the codec.
    Unsupported(String),
    /// The frame could not be decoded.
    Failed(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Unsupported(m) => write!(f, "{m}"),
            DecodeError::Failed(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A decoded frame plus how to display it. `hint` overrides the clip-level
/// colour hint when the decoder knows better (e.g. per-frame metadata).
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub image: Decoded,
    pub hint: ColourHint,
}

/// Decodes the frames at `positions` (0-based frame numbers), each at the
/// smallest native scale that is at least `min_width` wide where the decoder
/// offers scales. One result per position, in order; a failed frame never
/// stops the others.
pub fn decode_frames(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let _ = (clip, min_width);
    positions
        .iter()
        .map(|_| {
            Err(DecodeError::Unsupported(format!(
                "No decoder for {} yet",
                probe.codec.label()
            )))
        })
        .collect()
}
