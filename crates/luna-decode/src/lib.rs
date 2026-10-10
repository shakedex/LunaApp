//! Decoding a few frames of a clip for thumbnails.
//!
//! One entry point, [`decode_frames`], picks a backend from the probed codec:
//! our own decoders (ProRes, ARRIRAW, X-OCN, CRM), bundled vendor SDKs (BRAW;
//! R3D through the closed add-on loaded at runtime), or the platform decoder
//! (Media Foundation on Windows, AVFoundation on macOS). Platform APIs live only
//! in this crate, behind `cfg(target_os)`.

use luna_formats::{ClipFiles, Codec, ColourHint, Container, Gamut, Probe, Transfer};

mod arri_raw;
mod braw_sdk;
mod crm_raw;
#[cfg(windows)]
mod mf;
mod mosaic;
#[cfg(target_os = "macos")]
mod platform_macos;
mod prores;
mod r3d;
mod util;
mod vendor;
mod xocn_raw;

pub use vendor::vendor_probe;

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
        /// Scene-linear = (sample − black) × scale, with 18 % grey at 0.18, when
        /// the decoder knows the clip's exposure. `None`: the renderer falls back
        /// to a per-camera-family estimate.
        scene_scale: Option<f32>,
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
/// stops the others. Positions past the end are clamped to the last frame.
///
/// A [`Decoded::Mosaic`] counts as half its width (one picture pixel per 2×2
/// photosites), so mosaics come out at least `2 × min_width` photosites wide.
pub fn decode_frames(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    if positions.is_empty() {
        return Vec::new();
    }
    let backend = backend_for(clip, probe);
    let out = util::guard(backend.name(), || {
        Ok(match backend {
            Backend::XOcn => xocn_raw::decode(clip, probe, positions, min_width),
            Backend::Crm => crm_raw::decode(clip, probe, positions, min_width),
            Backend::Braw => braw_sdk::decode(clip, probe, positions, min_width),
            Backend::R3d => r3d::decode(clip, probe, positions, min_width),
            Backend::ArriRaw => arri_raw::decode(clip, probe, positions, min_width),
            Backend::ProRes => prores::decode(clip, probe, positions, min_width),
            Backend::Platform => platform_decode(clip, probe, positions, min_width),
            Backend::None => util::all_err(
                positions.len(),
                DecodeError::Unsupported(format!("No decoder for {} yet", probe.codec.label())),
            ),
        })
    });
    match out {
        // Backends return one result per position; never trust that blindly.
        Ok(mut v) if v.len() == positions.len() => {
            v.shrink_to_fit();
            v
        }
        Ok(_) => util::all_err(
            positions.len(),
            DecodeError::Failed(format!("{} decoder lost frames", backend.name())),
        ),
        Err(e) => util::all_err(positions.len(), e),
    }
}

/// Which decoder handles a clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    XOcn,
    Crm,
    Braw,
    R3d,
    ArriRaw,
    ProRes,
    Platform,
    None,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::XOcn => "X-OCN",
            Backend::Crm => "Cinema RAW Light",
            Backend::Braw => "Blackmagic RAW",
            Backend::R3d => "R3D",
            Backend::ArriRaw => "ARRIRAW",
            Backend::ProRes => "ProRes",
            Backend::Platform => "platform video",
            Backend::None => "no",
        }
    }
}

fn backend_for(clip: &ClipFiles, probe: &Probe) -> Backend {
    match (&probe.codec, clip.container) {
        (Codec::XOcn, _) => Backend::XOcn,
        (Codec::CinemaRawLight, _) | (_, Container::Crm) => Backend::Crm,
        (Codec::Braw, _) | (_, Container::Braw) => Backend::Braw,
        (Codec::Redcode, _) | (_, Container::R3d) => Backend::R3d,
        (Codec::ArriRaw, _) => Backend::ArriRaw,
        (Codec::ProRes(_), Container::Mxf | Container::QuickTime) => Backend::ProRes,
        (Codec::H264 | Codec::Hevc, Container::QuickTime) => Backend::Platform,
        _ => Backend::None,
    }
}

#[cfg(windows)]
fn platform_decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    mf::decode(clip, probe, positions, min_width)
}

#[cfg(target_os = "macos")]
fn platform_decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    platform_macos::decode(clip, probe, positions, min_width)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_decode(
    _clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    util::all_err(
        positions.len(),
        DecodeError::Unsupported(format!(
            "{} isn't supported on this platform",
            probe.codec.label()
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use luna_formats::{ClipMeta, FrameIndex, PictureLayout, ProResProfile};
    use std::path::PathBuf;

    fn clip(container: Container, main: &str) -> ClipFiles {
        ClipFiles {
            container,
            main: PathBuf::from(main),
            segments: vec![],
            sidecars: vec![],
        }
    }

    fn probe(codec: Codec) -> Probe {
        Probe {
            codec,
            meta: ClipMeta::default(),
            frames: FrameIndex::DecoderOwned,
            layout: PictureLayout::default(),
            warnings: vec![],
        }
    }

    #[test]
    fn picks_backend_from_codec_and_container() {
        let mxf = clip(Container::Mxf, "a.mxf");
        let mov = clip(Container::QuickTime, "a.mov");
        assert_eq!(backend_for(&mxf, &probe(Codec::XOcn)), Backend::XOcn);
        assert_eq!(backend_for(&mxf, &probe(Codec::ArriRaw)), Backend::ArriRaw);
        assert_eq!(backend_for(&mxf, &probe(Codec::ArriCore)), Backend::None);
        assert_eq!(
            backend_for(&mxf, &probe(Codec::ProRes(ProResProfile::Hq))),
            Backend::ProRes
        );
        assert_eq!(
            backend_for(&mov, &probe(Codec::ProRes(ProResProfile::P4444))),
            Backend::ProRes
        );
        assert_eq!(backend_for(&mov, &probe(Codec::H264)), Backend::Platform);
        assert_eq!(backend_for(&mov, &probe(Codec::Hevc)), Backend::Platform);
        assert_eq!(backend_for(&mxf, &probe(Codec::H264)), Backend::None);
        assert_eq!(backend_for(&mov, &probe(Codec::ProResRaw)), Backend::None);
        let crm = clip(Container::Crm, "a.crm");
        assert_eq!(
            backend_for(&crm, &probe(Codec::CinemaRawLight)),
            Backend::Crm
        );
        let braw = clip(Container::Braw, "a.braw");
        assert_eq!(backend_for(&braw, &probe(Codec::Braw)), Backend::Braw);
        let r3d = clip(Container::R3d, "a.R3D");
        assert_eq!(backend_for(&r3d, &probe(Codec::Redcode)), Backend::R3d);
    }

    #[test]
    fn unknown_codecs_are_unsupported_once_per_position() {
        let out = decode_frames(
            &clip(Container::Mxf, "a.mxf"),
            &probe(Codec::ArriCore),
            &[0, 5, 9],
            640,
        );
        assert_eq!(out.len(), 3);
        for r in out {
            assert_eq!(
                r,
                Err(DecodeError::Unsupported(
                    "No decoder for ARRICORE yet".into()
                ))
            );
        }
        let out = decode_frames(
            &clip(Container::Mxf, "a.mxf"),
            &probe(Codec::Other("AVC-Intra".into())),
            &[0],
            640,
        );
        assert_eq!(
            out,
            vec![Err(DecodeError::Unsupported(
                "No decoder for AVC-Intra yet".into()
            ))]
        );
    }

    #[test]
    fn no_positions_no_work() {
        let out = decode_frames(
            &clip(Container::Mxf, "missing.mxf"),
            &probe(Codec::XOcn),
            &[],
            640,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn missing_files_fail_every_position_without_panicking() {
        for (container, codec) in [
            (Container::Mxf, Codec::XOcn),
            (Container::Crm, Codec::CinemaRawLight),
            (Container::Mxf, Codec::ProRes(ProResProfile::Hq)),
            (Container::Mxf, Codec::ArriRaw),
            (Container::QuickTime, Codec::H264),
        ] {
            let out = decode_frames(
                &clip(container, "Z:/definitely/not/here.bin"),
                &probe(codec.clone()),
                &[0, 1],
                640,
            );
            assert_eq!(out.len(), 2, "{codec:?}");
            assert!(out.iter().all(|r| r.is_err()), "{codec:?}: {out:?}");
        }
    }
}
