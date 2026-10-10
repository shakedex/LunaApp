//! A clean-room H.264 (ITU-T H.264 | ISO/IEC 14496-10) decoder for intra
//! pictures, written from the specification for camera thumbnails.
//!
//! It decodes I and IDR pictures of the profiles cameras record — High,
//! High 10, High 4:2:2 and High 4:2:2 Intra — in 4:2:0 or 4:2:2 at 8 to 14
//! bits: CAVLC and CABAC, Intra 4×4 / 8×8 / 16×16, scaling matrices, PCM,
//! multiple slices (decoded in parallel) and the deblocking filter. Field
//! and MBAFF coding, 4:4:4, monochrome, slice groups and inter prediction
//! are reported as [`Error::Unsupported`].
//!
//! P and B pictures are out of scope: a caller wanting frame *n* of a long-GOP
//! clip uses [`access_unit_kind`] to find the nearest I or IDR access unit at
//! or before *n* and decodes that one instead.
//!
//! ```no_run
//! # fn demo(avcc: &[u8], sample: &[u8]) -> Result<(), luna_h264::Error> {
//! let config = luna_h264::DecoderConfig::from_avcc(avcc)?; // or DecoderConfig::annex_b()
//! if luna_h264::access_unit_kind(sample, &config) != luna_h264::FrameKind::Other {
//!     let pic = luna_h264::decode_access_unit(sample, &config)?;
//!     println!("{}×{} {}-bit", pic.width, pic.height, pic.bit_depth);
//! }
//! # Ok(()) }
//! ```

// Codec loops index several parallel tables by position; iterator chains
// would obscure the correspondence with the specification.
#![allow(clippy::needless_range_loop)]

mod bits;
mod cabac;
mod cavlc;
mod deblock;
mod intra;
mod mb;
mod nal;
mod params;
mod picture;
mod slice;
mod tables;
mod transform;

pub use params::Colour;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Valid H.264 this decoder does not handle (the message says what).
    Unsupported(String),
    /// The data is damaged or not H.264.
    Corrupt(String),
}

impl Error {
    pub(crate) fn corrupt(m: impl Into<String>) -> Error {
        Error::Corrupt(m.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(m) => write!(f, "unsupported H.264: {m}"),
            Error::Corrupt(m) => write!(f, "damaged H.264: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// A decoded picture, cropped to the SPS frame cropping window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// 1 = 4:2:0, 2 = 4:2:2.
    pub chroma_format: u8,
    /// Luma sample depth.
    pub bit_depth: u8,
    pub bit_depth_chroma: u8,
    /// `width × height` luma samples, row-major.
    pub y: Vec<u16>,
    /// Chroma planes: `width/2 × height/2` (4:2:0) or `width/2 × height` (4:2:2).
    pub cb: Vec<u16>,
    pub cr: Vec<u16>,
    /// VUI colour description (H.273 codes; 2 = unspecified).
    pub colour: Colour,
}

impl Picture {
    pub fn chroma_width(&self) -> u32 {
        self.width / 2
    }

    pub fn chroma_height(&self) -> u32 {
        if self.chroma_format == 1 {
            self.height / 2
        } else {
            self.height
        }
    }
}

/// What an access unit holds, for choosing frames to decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// An IDR picture.
    Idr,
    /// A non-IDR picture made only of I slices.
    I,
    /// Anything else (P/B slices, or no picture).
    Other,
}

/// Parameter sets and framing for a stream.
#[derive(Debug, Clone, Default)]
pub struct DecoderConfig {
    /// NAL length prefix size for AVCC samples; `None` = Annex B start codes.
    nal_length_size: Option<u8>,
    /// Out-of-band parameter sets: (id, NAL payload after the header byte).
    sps: Vec<(u32, Vec<u8>)>,
    pps: Vec<(u32, Vec<u8>)>,
    /// Upper bound on threads for decoding slices in parallel (0 = all cores).
    max_threads: usize,
}

impl DecoderConfig {
    /// Annex B access units carrying their parameter sets in-band.
    pub fn annex_b() -> DecoderConfig {
        DecoderConfig::default()
    }

    /// Length-prefixed (MP4/MOV) samples described by an `avcC` record body.
    pub fn from_avcc(record: &[u8]) -> Result<DecoderConfig, Error> {
        let a = nal::avcc(record)?;
        let mut c = DecoderConfig {
            nal_length_size: Some(a.len_size),
            ..DecoderConfig::default()
        };
        for s in a.sps {
            if s.len() > 4 {
                let r = bits::Rbsp::new(&s[1..]);
                let mut br = r.reader();
                br.skip(24);
                let id = br.ue_max(31, "seq_parameter_set_id")?;
                c.sps.push((id, s[1..].to_vec()));
            }
        }
        for p in a.pps {
            if p.len() > 1 {
                let r = bits::Rbsp::new(&p[1..]);
                let (id, _) = params::Pps::ids(&mut r.reader())?;
                c.pps.push((id, p[1..].to_vec()));
            }
        }
        Ok(c)
    }

    /// Limits slice-parallel decoding to `n` threads (1 = the calling thread
    /// only; 0 = one per core, the default).
    pub fn with_max_threads(mut self, n: usize) -> DecoderConfig {
        self.max_threads = n;
        self
    }

    fn nals<'a>(&self, au: &'a [u8]) -> Result<Vec<nal::Nal<'a>>, Error> {
        match self.nal_length_size {
            Some(n) => nal::length_prefixed(au, n),
            None => Ok(nal::annex_b(au)),
        }
    }
}

/// Decodes the (intra) picture in one access unit.
pub fn decode_access_unit(au: &[u8], config: &DecoderConfig) -> Result<Picture, Error> {
    let nals = config.nals(au)?;
    picture::decode(&nals, config)
}

/// Classifies an access unit without decoding it.
pub fn access_unit_kind(au: &[u8], config: &DecoderConfig) -> FrameKind {
    let Ok(nals) = config.nals(au) else {
        return FrameKind::Other;
    };
    let mut seen = false;
    for n in &nals {
        if n.kind == nal::NAL_IDR {
            return FrameKind::Idr;
        }
        if n.kind == nal::NAL_SLICE {
            let r = bits::Rbsp::new(n.payload);
            match slice::peek(&mut r.reader()) {
                Ok((first_mb, st, _)) => {
                    if first_mb == 0 && seen {
                        break;
                    }
                    if st != slice::SLICE_I {
                        return FrameKind::Other;
                    }
                    seen = true;
                }
                Err(_) => return FrameKind::Other,
            }
        }
    }
    if seen { FrameKind::I } else { FrameKind::Other }
}
