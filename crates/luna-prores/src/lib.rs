//! A clean-room Apple ProRes decoder written from SMPTE RDD 36.
//!
//! Decodes one coded frame (the `icpf` frame container) into planar Y′CbCr
//! (4:2:2 or 4:4:4, 10-/12-bit significant bits held in u16), plus alpha when
//! present. Never panics on bad input.

/// One decoded picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// 2 for 4:2:2, 3 for 4:4:4 (chroma subsampling flag in the frame header).
    pub chroma_format: u8,
    /// Planar samples, `width × height` for luma; chroma is `width/2` wide for 4:2:2.
    pub y: Vec<u16>,
    pub cb: Vec<u16>,
    pub cr: Vec<u16>,
    /// Significant bits per sample in the planes (10 for 4:2:2, 12 for 4:4:4).
    pub bits: u8,
    /// Colour primaries / transfer / matrix codes from the frame header (ITU-T H.273).
    pub color_primaries: u8,
    pub transfer_characteristic: u8,
    pub matrix_coefficients: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Corrupt(String),
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Corrupt(m) => write!(f, "corrupt ProRes frame: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported ProRes frame: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Decodes a whole frame. `downscale` of 1 is full resolution; 2, 4 or 8 decode
/// only the DC (and low) coefficients for a fast reduced picture.
pub fn decode(frame: &[u8], downscale: u8) -> Result<Picture, Error> {
    let _ = (frame, downscale);
    Err(Error::Unsupported("not implemented yet".into()))
}
