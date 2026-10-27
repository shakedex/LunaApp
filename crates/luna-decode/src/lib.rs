//! Decoding a few frames of a clip for thumbnails.
//!
//! One entry point, [`decode_frames`], picks a backend from the probed codec:
//! our own decoders (ProRes, intra H.264, ARRIRAW/ARRICORE, X-OCN, CRM), bundled vendor SDKs (BRAW;
//! R3D through the closed add-on loaded at runtime; ProRes RAW through FFmpeg's
//! LGPL DLLs on Windows), or the platform decoder
//! (Media Foundation on Windows, AVFoundation on macOS, which also takes
//! ProRes RAW). Platform APIs live only
//! in this crate, behind `cfg(target_os)`.

use luna_formats::{ClipFiles, Codec, ColourHint, Container, Gamut, Probe, Transfer};

mod arri_decoder;
mod braw_sdk;
mod crm_raw;
mod h264;
#[cfg(windows)]
mod mf;
#[cfg(windows)]
mod mf_es;
mod mosaic;
#[cfg(target_os = "macos")]
mod platform_macos;
mod prores;
#[cfg(windows)]
mod proresraw;
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

impl Frame {
    /// One short line for the log: the picture's form, size, levels, value
    /// range and colour, never its samples (only how many there are).
    pub fn describe(&self) -> String {
        let hint = format!("hint={}/{}", opt(self.hint.transfer), opt(self.hint.gamut));
        match &self.image {
            Decoded::Mosaic {
                width,
                height,
                cfa,
                data,
                black,
                white,
                wb,
                camera_to_xyz,
                scene_scale,
            } => format!(
                "Mosaic {width}x{height} cfa={cfa:?} samples={} range={} black={black} white={white} wb={} camera_to_xyz={} scene_scale={} {hint}",
                data.len(),
                range(data),
                match wb {
                    Some([r, g, b]) => format!("{r:.4},{g:.4},{b:.4}"),
                    None => "none".into(),
                },
                if camera_to_xyz.is_some() { "yes" } else { "no" },
                match scene_scale {
                    Some(s) => format!("{s}"),
                    None => "none".into(),
                },
            ),
            Decoded::Rgb16 {
                width,
                height,
                data,
                transfer,
                gamut,
            } => format!(
                "Rgb16 {width}x{height} samples={} range={} transfer={transfer:?} gamut={gamut:?} {hint}",
                data.len(),
                range(data)
            ),
            Decoded::Display8 {
                width,
                height,
                data,
            } => format!(
                "Display8 {width}x{height} samples={} range={} {hint}",
                data.len(),
                range(data)
            ),
            Decoded::Ycbcr {
                width,
                height,
                chroma_width,
                chroma_height,
                bits,
                y,
                matrix,
                full_range,
                ..
            } => format!(
                "Ycbcr {width}x{height} chroma={chroma_width}x{chroma_height} bits={bits} samples={} range={} matrix={matrix:?} levels={} {hint}",
                y.len(),
                range(y),
                if *full_range { "full" } else { "legal" }
            ),
        }
    }
}

/// How many samples [`range`] looks at, at most.
const RANGE_SAMPLES: usize = 65_536;

/// `min..max` of an evenly spaced sample of the codes (and the last one), so
/// a full-size frame stays cheap; `none` when there are no samples.
fn range<T: Copy + Ord + std::fmt::Display>(data: &[T]) -> String {
    let step = data.len().div_ceil(RANGE_SAMPLES).max(1);
    let sample = data.iter().step_by(step).chain(data.last()).copied();
    match sample.clone().min().zip(sample.max()) {
        Some((min, max)) => format!("{min}..{max}"),
        None => "none".into(),
    }
}

fn opt<T: std::fmt::Debug>(v: Option<T>) -> String {
    v.map_or_else(|| "unknown".into(), |v| format!("{v:?}"))
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
    tracing::debug!(positions = ?positions, min_width, "decode: {}", backend.name());
    let out = util::guard(backend.name(), || {
        Ok(match backend {
            Backend::XOcn => xocn_raw::decode(clip, probe, positions, min_width),
            Backend::Crm => crm_raw::decode(clip, probe, positions, min_width),
            Backend::Braw => braw_sdk::decode(clip, probe, positions, min_width),
            Backend::R3d => r3d::decode(clip, probe, positions, min_width),
            Backend::Arri => arri_decoder::decode(clip, probe, positions, min_width),
            Backend::ProRes => prores::decode(clip, probe, positions, min_width),
            Backend::ProResRaw => proresraw_decode(clip, probe, positions, min_width),
            Backend::H264 => h264::decode(clip, probe, positions, min_width),
            Backend::PlatformEs => platform_es_decode(clip, probe, positions, min_width),
            Backend::Platform => with_h264_fallback(
                clip,
                probe,
                positions,
                min_width,
                platform_decode(clip, probe, positions, min_width),
            ),
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
    Arri,
    ProRes,
    ProResRaw,
    H264,
    Platform,
    /// HEVC elementary frames from MXF, through the platform decoder.
    PlatformEs,
    None,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::XOcn => "X-OCN",
            Backend::Crm => "Cinema RAW Light",
            Backend::Braw => "Blackmagic RAW",
            Backend::R3d => "R3D",
            Backend::Arri => "ARRI",
            Backend::ProRes => "ProRes",
            Backend::ProResRaw => "ProRes RAW",
            Backend::H264 => "H.264",
            Backend::Platform => "platform video",
            Backend::PlatformEs => "platform HEVC",
            Backend::None => "no",
        }
    }
}

fn backend_for(clip: &ClipFiles, probe: &Probe) -> Backend {
    match (&probe.codec, clip.container) {
        (Codec::XOcn, _) => Backend::XOcn,
        (Codec::CinemaRawLight, _) | (_, Container::Crm) => Backend::Crm,
        (Codec::Braw, _) | (_, Container::Braw) => Backend::Braw,
        (Codec::Redcode | Codec::NRaw | Codec::R3dNe, _) | (_, Container::R3d | Container::Nev) => {
            Backend::R3d
        }
        (Codec::ArriRaw | Codec::ArriRawHde | Codec::ArriCore, _) => Backend::Arri,
        (Codec::ProRes(_), Container::Mxf | Container::QuickTime) => Backend::ProRes,
        (Codec::ProResRaw, Container::QuickTime) => Backend::ProResRaw,
        (Codec::H264, Container::Mxf) => Backend::H264,
        (Codec::H264 | Codec::Hevc, Container::QuickTime) => Backend::Platform,
        (Codec::Hevc, Container::Mxf) => Backend::PlatformEs,
        _ => Backend::None,
    }
}

/// H.264 positions the platform decoder can't take (High 4:2:2 and High 10
/// on Windows) go to our own intra decoder when the clip's frames are indexed.
fn with_h264_fallback(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
    mut out: Vec<Result<Frame, DecodeError>>,
) -> Vec<Result<Frame, DecodeError>> {
    if probe.codec != Codec::H264 || !matches!(probe.frames, luna_formats::FrameIndex::Ranges(_)) {
        return out;
    }
    let redo: Vec<usize> = (0..out.len().min(positions.len()))
        .filter(|&k| matches!(out[k], Err(DecodeError::Unsupported(_))))
        .collect();
    if redo.is_empty() {
        return out;
    }
    let again: Vec<u64> = redo.iter().map(|&k| positions[k]).collect();
    for (k, r) in redo
        .into_iter()
        .zip(h264::decode(clip, probe, &again, min_width))
    {
        out[k] = r;
    }
    out
}

#[cfg(windows)]
fn proresraw_decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    proresraw::decode(clip, probe, positions, min_width)
}

#[cfg(target_os = "macos")]
fn proresraw_decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    platform_macos::decode_raw(clip, probe, positions, min_width)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn proresraw_decode(
    _clip: &ClipFiles,
    _probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    util::all_err(
        positions.len(),
        DecodeError::Unsupported("ProRes RAW isn't supported on this platform yet".into()),
    )
}

#[cfg(windows)]
fn platform_es_decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    mf_es::decode(clip, probe, positions, min_width)
}

#[cfg(not(windows))]
fn platform_es_decode(
    _clip: &ClipFiles,
    _probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    util::all_err(
        positions.len(),
        DecodeError::Unsupported("HEVC in MXF isn't supported on this platform yet".into()),
    )
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
            codec_config: None,
            warnings: vec![],
        }
    }

    #[test]
    fn picks_backend_from_codec_and_container() {
        let mxf = clip(Container::Mxf, "a.mxf");
        let mov = clip(Container::QuickTime, "a.mov");
        assert_eq!(backend_for(&mxf, &probe(Codec::XOcn)), Backend::XOcn);
        assert_eq!(backend_for(&mxf, &probe(Codec::ArriRaw)), Backend::Arri);
        assert_eq!(backend_for(&mxf, &probe(Codec::ArriRawHde)), Backend::Arri);
        assert_eq!(backend_for(&mxf, &probe(Codec::ArriCore)), Backend::Arri);
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
        assert_eq!(backend_for(&mxf, &probe(Codec::H264)), Backend::H264);
        assert_eq!(backend_for(&mxf, &probe(Codec::Hevc)), Backend::PlatformEs);
        assert_eq!(
            backend_for(&mov, &probe(Codec::ProResRaw)),
            Backend::ProResRaw
        );
        let crm = clip(Container::Crm, "a.crm");
        assert_eq!(
            backend_for(&crm, &probe(Codec::CinemaRawLight)),
            Backend::Crm
        );
        let braw = clip(Container::Braw, "a.braw");
        assert_eq!(backend_for(&braw, &probe(Codec::Braw)), Backend::Braw);
        let r3d = clip(Container::R3d, "a.R3D");
        assert_eq!(backend_for(&r3d, &probe(Codec::Redcode)), Backend::R3d);
        let nev = clip(Container::Nev, "DSC_0001.NEV");
        assert_eq!(backend_for(&nev, &probe(Codec::NRaw)), Backend::R3d);
    }

    #[test]
    fn unknown_codecs_are_unsupported_once_per_position() {
        let out = decode_frames(
            &clip(Container::Mxf, "a.mxf"),
            &probe(Codec::ProResRaw),
            &[0, 5, 9],
            640,
        );
        assert_eq!(out.len(), 3);
        for r in out {
            assert_eq!(
                r,
                Err(DecodeError::Unsupported(
                    "No decoder for ProRes RAW yet".into()
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
    fn describe_has_levels_not_data() {
        let frame = Frame {
            image: Decoded::Mosaic {
                width: 4448,
                height: 3096,
                cfa: Cfa::Rggb,
                data: {
                    let mut data = vec![1234; 4448 * 3096];
                    data[0] = 300;
                    *data.last_mut().unwrap() = 3999;
                    data
                },
                black: 256.0,
                white: 4095.0,
                wb: Some([1.9, 1.0, 1.6]),
                camera_to_xyz: Some([0.5; 9]),
                scene_scale: None,
            },
            hint: ColourHint {
                transfer: Some(Transfer::LogC3),
                gamut: Some(Gamut::ArriWideGamut3),
            },
        };
        let text = frame.describe();
        assert!(text.contains("black=256"), "{text}");
        assert!(text.contains("white=4095"), "{text}");
        assert!(text.contains("range=300..3999"), "{text}");
        assert!(!text.contains("1234"), "{text}");
        assert!(!text.contains("  "), "{text}");
        assert!(text.len() < 300, "{text}");
    }

    #[test]
    fn describe_gives_the_value_range_of_every_form() {
        let rgb = Frame {
            image: Decoded::Rgb16 {
                width: 2,
                height: 1,
                data: vec![40, 9000, 512, 7, 65535, 100],
                transfer: Transfer::Rec709,
                gamut: Gamut::Rec709,
            },
            hint: ColourHint::default(),
        };
        let text = rgb.describe();
        assert!(text.contains("range=7..65535"), "{text}");
        assert!(!text.contains("  "), "{text}");

        let ycbcr = Frame {
            image: Decoded::Ycbcr {
                width: 2,
                height: 2,
                chroma_width: 1,
                chroma_height: 1,
                bits: 10,
                y: vec![64, 940, 500, 70],
                cb: vec![0],
                cr: vec![1023],
                matrix: YcbcrMatrix::Bt709,
                full_range: false,
            },
            hint: ColourHint::default(),
        };
        let text = ycbcr.describe();
        assert!(text.contains("range=64..940"), "{text}");
        assert!(!text.contains("  "), "{text}");

        let empty = Frame {
            image: Decoded::Display8 {
                width: 0,
                height: 0,
                data: Vec::new(),
            },
            hint: ColourHint::default(),
        };
        assert!(
            empty.describe().contains("range=none"),
            "{}",
            empty.describe()
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
            (Container::Mxf, Codec::ArriCore),
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
