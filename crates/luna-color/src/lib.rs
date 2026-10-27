//! Turning a decoded frame into a display-referred Rec.709 thumbnail.
//!
//! Every camera goes through the same steps so thumbnails from different
//! cameras look alike next to each other:
//!
//! 1. **Reduce while reading.** The decoded picture is box-averaged by a
//!    whole-number factor towards the thumbnail size (raw mosaics as 2×2 CFA
//!    super-pixels, Y′CbCr per plane), so an 8K frame is read once, in integers.
//! 2. **Exact resample** (area filter) to the final size, desqueezing
//!    anamorphic pictures.
//! 3. **Colour** on the small picture only: code values → scene-linear with the
//!    camera maker's published log curve (`curves`), camera gamut → Rec.709
//!    primaries from published primaries (`gamut`), then one shared display
//!    rendering (`tone`) to BT.1886 Rec.709.
//!
//! Averaging happens on code values for log pictures (the colour maths runs
//! after the reduction to keep it cheap). Strictly, averaging should be in
//! linear light, but at thumbnail scale the difference only shows at
//! high-contrast edges as a hair of extra darkening.

mod curves;
mod gamut;
mod lut;
mod mosaic;
mod scale;
mod tone;
mod ycbcr;

#[cfg(test)]
mod tests;

pub use lut::{Lut, LutError};

use luna_decode::{Decoded, Frame};
use luna_formats::{Gamut, Transfer};

use gamut::Mat3;
use scale::Image;

/// An 8-bit sRGB/Rec.709 display image, interleaved RGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The outcome of rendering, so the report can say when it had to guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub image: Rgb8,
    /// True when the frame's transfer function was unknown and it is shown as decoded.
    pub transform_unknown: bool,
    /// What was actually applied; [`Applied::Rec709`] when the camera signal
    /// couldn't be worked out for a non-Rec.709 choice.
    pub applied: Applied,
}

/// How a thumbnail is coloured.
#[derive(Debug, Clone, Copy)]
pub enum Colour<'a> {
    /// Luna's own rendering to Rec.709.
    Rec709,
    /// The camera's own signal (its log curve in its gamut), shown as is.
    CameraLog,
    /// The camera signal through a LUT whose output is Rec.709 signal.
    Lut(&'a Lut),
}

/// What [`render_with`] applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Rec709,
    CameraLog,
    Lut,
}

/// JPEG quality for report thumbnails.
pub const JPEG_QUALITY: u8 = 82;

/// A thumbnail whose brightest luma is at or below this looks black.
pub const NEAR_BLACK_MAX: u8 = 16;

/// Rec.709 luma of a display image, on its 8-bit codes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LumaStats {
    pub min: u8,
    pub mean: f32,
    pub max: u8,
}

/// `image` turned clockwise by `degrees` (90, 180 or 270); any other value
/// returns it unchanged.
pub fn rotate(image: &Rgb8, degrees: u16) -> Rgb8 {
    let (w, h) = (image.width as usize, image.height as usize);
    let (nw, nh) = match degrees {
        90 | 270 => (h, w),
        180 => (w, h),
        _ => return image.clone(),
    };
    // The source pixel each output pixel takes.
    let source = |x: usize, y: usize| match degrees {
        90 => (y, h - 1 - x),
        180 => (w - 1 - x, h - 1 - y),
        _ => (w - 1 - y, x),
    };
    let mut data = Vec::with_capacity(image.data.len());
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = source(x, y);
            data.extend_from_slice(&image.data[(sy * w + sx) * 3..][..3]);
        }
    }
    Rgb8 {
        width: nw as u32,
        height: nh as u32,
        data,
    }
}

/// Min, mean and max Rec.709 luma over every pixel. All zero for an empty image.
pub fn luma_stats(image: &Rgb8) -> LumaStats {
    let (mut min, mut max, mut sum, mut n) = (u8::MAX, 0u8, 0f64, 0u64);
    for px in image.data.as_chunks::<3>().0 {
        let y = 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
        let code = y.round().clamp(0.0, 255.0) as u8;
        min = min.min(code);
        max = max.max(code);
        sum += y as f64;
        n += 1;
    }
    if n == 0 {
        return LumaStats {
            min: 0,
            mean: 0.0,
            max: 0,
        };
    }
    LumaStats {
        min,
        mean: (sum / n as f64) as f32,
        max,
    }
}

/// Renders `frame` to Rec.709 and scales it to at most `max_width` wide
/// (aspect kept, never upscaled). `pixel_aspect` widens anamorphic pictures.
///
/// Per input:
/// - `Mosaic`: black/white levels, white balance (1,1,1 when unknown), 2×2
///   binning, raw exposure from the camera family, then camera RGB → Rec.709
///   through `camera_to_xyz` when given, else by treating white-balanced
///   camera RGB as the hint's gamut (an approximation: sensor RGB is not
///   exactly AWG / S-Gamut3 / Cinema Gamut, but those gamuts were designed
///   around their sensors, so it is close).
/// - `Ycbcr`: depth, range and matrix honoured, then the hint's transfer and
///   gamut. No transfer in the hint → shown as decoded, flagged unknown.
/// - `Rgb16`: its own transfer and gamut (full-range 16-bit codes).
/// - `Display8`: already display-referred; scaled only.
///
/// A frame whose buffers are shorter than its size promises renders black.
pub fn render(frame: &Frame, max_width: u32, pixel_aspect: Option<(u32, u32)>) -> Rendered {
    render_with(frame, max_width, pixel_aspect, Colour::Rec709)
}

/// [`render`] with a colour choice. The camera signal for [`Colour::CameraLog`]
/// and [`Colour::Lut`], per input:
/// - `Ycbcr`: as decoded, legal range expanded (64 → 0, 940 → 1 at 10 bits),
///   whatever the clip's colour tag.
/// - `Rgb16`: as decoded, except `Linear` in a camera gamut, which is encoded
///   with that gamut's log curve.
/// - `Mosaic`: linear camera RGB in its native gamut (through `camera_to_xyz`
///   when given), encoded with the native log curve.
/// - `Display8`: as is.
///
/// A raw frame whose native gamut isn't known falls back to Rec.709.
pub fn render_with(
    frame: &Frame,
    max_width: u32,
    pixel_aspect: Option<(u32, u32)>,
    colour: Colour,
) -> Rendered {
    let (w, h) = match &frame.image {
        Decoded::Mosaic { width, height, .. }
        | Decoded::Rgb16 { width, height, .. }
        | Decoded::Display8 { width, height, .. }
        | Decoded::Ycbcr { width, height, .. } => (*width, *height),
    };
    let (tw, th) = scale::fit(w, h, max_width, pixel_aspect);
    let (tw, th) = (tw as usize, th as usize);

    let (img, look, transform_unknown, signal) = match &frame.image {
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
        } => {
            let width = *width as usize;
            let rows = (*height as usize).min(data.len() / width.max(1));
            let (cw, ch) = mosaic::cells(width, rows);
            let img = (cw > 0 && ch > 0).then(|| {
                let (kx, ky) = (scale::reduction(cw, tw), scale::reduction(ch, th));
                let mut img = mosaic::reduce(width, rows, *cfa, data, kx, ky);
                let (stops, from) = match scene_scale.filter(|s| s.is_finite() && *s > 0.0) {
                    Some(s) => ((s * (white - black) / 0.18).log2(), "scene_scale"),
                    None => (
                        mosaic::clip_stops_above_grey(frame.hint.gamut),
                        "camera family",
                    ),
                };
                tracing::debug!(stops, from, "render: clip point above grey");
                mosaic::linearise(&mut img, *black, *white, *wb, stops);
                img
            });
            let matrix = match camera_to_xyz {
                Some(m) => camera_matrix(m),
                None => gamut::to_rec709(frame.hint.gamut.unwrap_or(Gamut::Rec709)),
            };
            let look = Look::Scene {
                decode: None,
                matrix: to_f32(&matrix),
            };
            let native = frame
                .hint
                .gamut
                .or_else(|| frame.hint.transfer.and_then(curves::native_gamut));
            let signal = native.and_then(|g| {
                let log = curves::native_log(g)?;
                let matrix = camera_to_xyz.as_ref().map(|m| {
                    let d65 = gamut::primaries(Gamut::Rec709).w;
                    to_f32(&gamut::mul(&gamut::xyz_to(g, d65), &camera_xyz(m)))
                });
                Some(Signal::Encode { matrix, log })
            });
            (img, look, false, signal)
        }
        Decoded::Rgb16 {
            width,
            height,
            data,
            transfer,
            gamut,
        } => {
            let img = reduce_interleaved(data, *width, *height, tw, th, 1.0 / 65535.0);
            let signal = match transfer {
                Transfer::Linear => {
                    curves::native_log(*gamut).map(|log| Signal::Encode { matrix: None, log })
                }
                _ => Some(Signal::AsIs),
            };
            let look = Look::new(Some(*transfer), Some(*gamut), |c| c);
            (img, look, false, signal)
        }
        Decoded::Display8 {
            width,
            height,
            data,
        } => {
            let img = reduce_interleaved(data, *width, *height, tw, th, 1.0 / 255.0);
            (img, Look::Passthrough, false, Some(Signal::AsIs))
        }
        Decoded::Ycbcr {
            width,
            height,
            chroma_width,
            chroma_height,
            bits,
            y,
            cb,
            cr,
            matrix,
            full_range,
        } => {
            let planes = ycbcr::Planes {
                width: *width as usize,
                height: *height as usize,
                chroma_width: *chroma_width as usize,
                chroma_height: *chroma_height as usize,
                bits: *bits,
                y,
                cb,
                cr,
                matrix: *matrix,
                full_range: *full_range,
            };
            let img = planes.is_complete().then(|| planes.reduce_to_rgb(tw, th));
            let to_code = ycbcr::signal_to_code(*bits, *full_range);
            let hint = frame.hint;
            let look = Look::new(hint.transfer, hint.gamut, &to_code);
            // Legal-range video reaches LUTs expanded, whatever its tag, as
            // Resolve feeds .cube LUTs for video-level clips.
            (img, look, hint.transfer.is_none(), Some(Signal::AsIs))
        }
    };
    let (look, applied) = match (colour, signal) {
        (Colour::CameraLog, Some(s)) => (Look::Signal(s, None), Applied::CameraLog),
        (Colour::Lut(lut), Some(s)) => (Look::Signal(s, Some(lut)), Applied::Lut),
        _ => (look, Applied::Rec709),
    };

    let image = match img {
        Some(img) if img.width > 0 && img.height > 0 => {
            tracing::debug!(look = look.name(), width = tw, height = th, "render: done");
            look.apply(&scale::area_resample(&img, tw, th))
        }
        _ => {
            tracing::warn!("render: frame data is shorter than its size, rendered black");
            Rgb8 {
                width: tw as u32,
                height: th as u32,
                data: vec![0; tw * th * 3],
            }
        }
    };
    Rendered {
        image,
        // Only Rec.709 needs the transfer; the other choices show the signal.
        transform_unknown: transform_unknown && applied == Applied::Rec709,
        applied,
    }
}

/// Baseline JPEG at [`JPEG_QUALITY`] (4:2:0). Empty if the image's buffer
/// does not match its size or the size does not fit a JPEG.
pub fn encode_jpeg(image: &Rgb8) -> Vec<u8> {
    let (Ok(w), Ok(h)) = (u16::try_from(image.width), u16::try_from(image.height)) else {
        return Vec::new();
    };
    if w == 0 || h == 0 || image.data.len() != w as usize * h as usize * 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY);
    match encoder.encode(&image.data, w, h, jpeg_encoder::ColorType::Rgb) {
        Ok(()) => out,
        Err(_) => Vec::new(),
    }
}

/// Camera RGB → XYZ (D65, as the decoder states) → linear Rec.709.
///
/// The matrix is assumed to apply to white-balanced camera RGB, so camera
/// white (1,1,1) lands on D65 white.
fn camera_matrix(m: &[f32; 9]) -> Mat3 {
    let d65 = gamut::primaries(Gamut::Rec709).w;
    gamut::mul(&gamut::xyz_to_rec709(d65), &camera_xyz(m))
}

fn camera_xyz(m: &[f32; 9]) -> Mat3 {
    [
        [m[0] as f64, m[1] as f64, m[2] as f64],
        [m[3] as f64, m[4] as f64, m[5] as f64],
        [m[6] as f64, m[7] as f64, m[8] as f64],
    ]
}

/// How resampled values become the camera's own signal.
enum Signal {
    /// They already are.
    AsIs,
    /// Scene-linear (converted to the native gamut first when a matrix is
    /// given) → the native log curve.
    Encode {
        matrix: Option<[[f32; 3]; 3]>,
        log: Transfer,
    },
}

impl Signal {
    fn of(&self, px: [f32; 3]) -> [f32; 3] {
        match self {
            Signal::AsIs => px,
            Signal::Encode { matrix, log } => {
                let lin = matrix.as_ref().map_or(px, |m| mat(m, px));
                lin.map(|v| curves::encode(*log, v.max(0.0) as f64) as f32)
            }
        }
    }
}

fn to_f32(m: &Mat3) -> [[f32; 3]; 3] {
    m.map(|row| row.map(|v| v as f32))
}

/// Box-reduces interleaved RGB samples by whole-number factors towards
/// `tw × th`, scaling each mean by `unit`. `None` if the buffer is short.
fn reduce_interleaved<T: Copy + Into<u32>>(
    data: &[T],
    width: u32,
    height: u32,
    tw: usize,
    th: usize,
    unit: f32,
) -> Option<Image> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || data.len() < w * h * 3 {
        return None;
    }
    let (kx, ky) = (scale::reduction(w, tw), scale::reduction(h, th));
    let (ow, oh) = (w / kx, h / ky);
    let scale = unit / (kx * ky) as f32;
    let mut out = Vec::with_capacity(ow * oh * 3);
    let mut acc = vec![[0u64; 3]; ow];
    for oy in 0..oh {
        acc.iter_mut().for_each(|a| *a = [0; 3]);
        for y in oy * ky..(oy + 1) * ky {
            let row = &data[y * w * 3..(y + 1) * w * 3];
            for (ox, a) in acc.iter_mut().enumerate() {
                for px in row[ox * kx * 3..(ox + 1) * kx * 3].as_chunks::<3>().0 {
                    a[0] += px[0].into() as u64;
                    a[1] += px[1].into() as u64;
                    a[2] += px[2].into() as u64;
                }
            }
        }
        out.extend(acc.iter().flat_map(|a| a.map(|v| v as f32 * scale)));
    }
    Some(Image {
        width: ow,
        height: oh,
        data: out,
    })
}

/// Domain of the code → linear table, wide enough for sub-black and
/// super-white codes.
const CODE_LO: f32 = -0.25;
const CODE_HI: f32 = 1.25;
const CODE_ENTRIES: usize = 8192;

/// Code value → scene-linear, sampled from the exact curve.
struct CodeToLinear {
    table: Vec<f32>,
}

impl CodeToLinear {
    fn new(transfer: Transfer, to_code: impl Fn(f32) -> f32) -> Self {
        let table = (0..=CODE_ENTRIES)
            .map(|i| {
                let v = CODE_LO + (CODE_HI - CODE_LO) * i as f32 / CODE_ENTRIES as f32;
                curves::to_scene_linear(transfer, to_code(v) as f64) as f32
            })
            .collect();
        Self { table }
    }

    #[inline]
    fn get(&self, v: f32) -> f32 {
        let pos = ((v - CODE_LO) / (CODE_HI - CODE_LO) * CODE_ENTRIES as f32)
            .clamp(0.0, CODE_ENTRIES as f32);
        let i = (pos as usize).min(CODE_ENTRIES - 1);
        let f = pos - i as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * f
    }
}

/// How the resampled picture becomes display code values.
enum Look<'a> {
    /// Values are display signal already (Rec.709, or unknown transfer).
    Passthrough,
    /// Display signal in a wider container: linearise (BT.1886), convert to
    /// Rec.709 primaries, gamut-clip, re-encode.
    DisplayGamut([[f32; 3]; 3]),
    /// Scene-referred: decode (or already linear), matrix, gamut clip, tone.
    Scene {
        decode: Option<CodeToLinear>,
        matrix: [[f32; 3]; 3],
    },
    /// The camera's own signal, through a LUT when given.
    Signal(Signal, Option<&'a Lut>),
}

impl Look<'_> {
    /// The look for a picture whose values are `transfer`-coded in `gamut`.
    /// `to_code` maps the picture's values to full-range code fractions for
    /// camera log curves (legal-range video carries the camera's codes as-is).
    fn new(transfer: Option<Transfer>, gamut: Option<Gamut>, to_code: impl Fn(f32) -> f32) -> Self {
        let gamut = gamut.unwrap_or(Gamut::Rec709);
        let matrix = to_f32(&gamut::to_rec709(gamut));
        match transfer {
            None => Look::Passthrough,
            Some(Transfer::Rec709) if gamut == Gamut::Rec709 => Look::Passthrough,
            Some(Transfer::Rec709) => Look::DisplayGamut(matrix),
            // BT.2100 and linear values are signal, not camera codes.
            Some(t @ (Transfer::Hlg | Transfer::Pq | Transfer::Linear)) => Look::Scene {
                decode: Some(CodeToLinear::new(t, |v| v)),
                matrix,
            },
            Some(t) => Look::Scene {
                decode: Some(CodeToLinear::new(t, to_code)),
                matrix,
            },
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Look::Passthrough => "passthrough",
            Look::DisplayGamut(_) => "display gamut",
            Look::Scene { decode: None, .. } => "scene linear",
            Look::Scene { .. } => "scene curve",
            Look::Signal(_, None) => "camera signal",
            Look::Signal(_, Some(_)) => "camera signal + LUT",
        }
    }

    fn apply(&self, img: &Image) -> Rgb8 {
        let mut data = Vec::with_capacity(img.data.len());
        match self {
            Look::Passthrough => data.extend(img.data.iter().map(|&v| tone::to_u8(v))),
            Look::DisplayGamut(m) => {
                for px in img.data.as_chunks::<3>().0 {
                    let lin = [px[0], px[1], px[2]].map(|v| tone::decode_display(v as f64) as f32);
                    let rgb = tone::gamut_clip(mat(m, lin));
                    data.extend(rgb.map(|v| tone::to_u8(tone::encode_display(v as f64) as f32)));
                }
            }
            Look::Scene { decode, matrix } => {
                let lut = tone::SceneToDisplay::new();
                for px in img.data.as_chunks::<3>().0 {
                    let mut lin = [px[0], px[1], px[2]];
                    if let Some(d) = decode {
                        lin = lin.map(|v| d.get(v));
                    }
                    let rgb = tone::gamut_clip(mat(matrix, lin));
                    data.extend(rgb.map(|v| tone::to_u8(lut.signal(v))));
                }
            }
            Look::Signal(signal, lut) => {
                for px in img.data.as_chunks::<3>().0 {
                    let mut v = signal.of([px[0], px[1], px[2]]);
                    if let Some(lut) = lut {
                        v = lut.apply(v);
                    }
                    data.extend(v.map(tone::to_u8));
                }
            }
        }
        Rgb8 {
            width: img.width as u32,
            height: img.height as u32,
            data,
        }
    }
}

#[inline]
fn mat(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}
