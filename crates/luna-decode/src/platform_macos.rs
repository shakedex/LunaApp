//! H.264 / HEVC in MOV/MP4 through AVFoundation, macOS's own decoders.
//!
//! The Objective-C shim (`shim/luna_avf.m`) opens one `AVAssetReader` per
//! position, limited to a time range that starts a quarter of a frame into it,
//! and hands back the first frame it decodes as the decoder's native bi-planar
//! Y′CbCr (no colour conversion), locked in place while we copy it. The output
//! format follows the stream's `avcC` / `hvcC` record: 8- or 10-bit, 4:2:0,
//! 4:2:2 or 4:4:4, video or full range.
//!
//! A stream the decoder refuses before any frame has decoded is reported as
//! unsupported, so H.264 the platform can't take falls back to our own decoder.

use std::ffi::{CString, c_char, c_void};

use luna_formats::{ClipFiles, Codec, ColourHint, Probe};

use crate::util;
use crate::{DecodeError, Decoded, Frame};

const OK: i32 = 0;
const UNSUPPORTED: i32 = 2;
const NOT_FOUND: i32 = 3;

#[repr(C)]
struct Info {
    fps: f64,
    start: f64,
    duration: f64,
    transfer: u8,
    primaries: u8,
    full_range: i8,
    config: *const u8,
    config_len: usize,
}

#[repr(C)]
struct Image {
    pixel_format: u32,
    width: u32,
    height: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    planes: [*const u8; 2],
    bytes_per_row: [usize; 2],
    plane_height: [usize; 2],
    matrix: u8,
    handle: *mut c_void,
}

unsafe extern "C" {
    fn luna_avf_open(
        path: *const c_char,
        info: *mut Info,
        err: *mut c_char,
        len: usize,
    ) -> *mut c_void;
    fn luna_avf_frame(
        reader: *mut c_void,
        seconds: f64,
        pixel_format: u32,
        out: *mut Image,
        err: *mut c_char,
        len: usize,
    ) -> i32;
    fn luna_avf_image_release(image: *mut Image);
    fn luna_avf_close(reader: *mut c_void);
}

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let mut reader = match Reader::open(clip, probe) {
        Ok(r) => r,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let count = probe
        .meta
        .frame_count
        .or_else(|| {
            let n = (reader.duration * reader.fps).round();
            (n >= 1.0).then_some(n as u64)
        })
        .unwrap_or(u64::MAX);
    util::per_frame("AVFoundation", positions, count, |i| reader.frame_at(i))
}

struct Reader {
    raw: *mut c_void,
    fps: f64,
    start: f64,
    duration: f64,
    pixel_format: u32,
    hint: ColourHint,
    label: &'static str,
    decoded_any: bool,
    /// Set once the decoder has refused the stream; later positions get the
    /// same error without trying again.
    broken: Option<DecodeError>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: `raw` came from luna_avf_open and is closed once.
        unsafe { luna_avf_close(self.raw) }
    }
}

impl Reader {
    fn open(clip: &ClipFiles, probe: &Probe) -> Result<Reader, DecodeError> {
        let path = CString::new(util::path_str(&clip.main)?).map_err(util::failed)?;
        let mut err = [0 as c_char; 512];
        // SAFETY: zeroed is a valid Info (null config pointer, zero length).
        let mut info: Info = unsafe { std::mem::zeroed() };
        // SAFETY: valid NUL-terminated path, out pointers to live locals.
        let raw = unsafe { luna_avf_open(path.as_ptr(), &mut info, err.as_mut_ptr(), err.len()) };
        if raw.is_null() {
            return Err(DecodeError::Failed(message(&err)));
        }
        let config = if info.config.is_null() {
            &[][..]
        } else {
            // SAFETY: the shim keeps the record alive until luna_avf_close.
            unsafe { std::slice::from_raw_parts(info.config, info.config_len) }
        };
        let hevc = probe.codec == Codec::Hevc;
        let (chroma, bits) = stream_format(hevc, config);
        let mut hint = probe.meta.colour;
        hint.transfer = hint.transfer.or(util::transfer_for(info.transfer));
        hint.gamut = hint.gamut.or(util::gamut_for(info.primaries));
        let fps = probe
            .meta
            .frame_rate
            .map(|r| r.as_f64())
            .filter(|f| *f > 0.0)
            .or((info.fps > 0.0).then_some(info.fps))
            .unwrap_or(25.0);
        Ok(Reader {
            raw,
            fps,
            start: info.start,
            duration: info.duration,
            pixel_format: pixel_format(chroma, bits, info.full_range == 1),
            hint,
            label: if hevc {
                "This HEVC stream"
            } else {
                "This H.264 stream"
            },
            decoded_any: false,
            broken: None,
        })
    }

    fn frame_at(&mut self, frame: u64) -> Result<Frame, DecodeError> {
        if let Some(e) = &self.broken {
            return Err(e.clone());
        }
        // A quarter frame in, so rounding never lands on the frame before.
        let seconds = self.start + (frame as f64 + 0.25) / self.fps;
        let mut err = [0 as c_char; 512];
        // SAFETY: zeroed is a valid empty Image (null planes and handle).
        let mut image: Image = unsafe { std::mem::zeroed() };
        // SAFETY: live reader, out pointers to live locals.
        let code = unsafe {
            luna_avf_frame(
                self.raw,
                seconds,
                self.pixel_format,
                &mut image,
                err.as_mut_ptr(),
                err.len(),
            )
        };
        match code {
            OK => {}
            UNSUPPORTED if !self.decoded_any => {
                let e = DecodeError::Unsupported(format!(
                    "{} isn't supported by this Mac's decoders ({})",
                    self.label,
                    message(&err)
                ));
                self.broken = Some(e.clone());
                return Err(e);
            }
            NOT_FOUND => {
                return Err(DecodeError::Failed(format!(
                    "Frame {frame} could not be found"
                )));
            }
            _ => {
                return Err(DecodeError::Failed(format!(
                    "Could not decode frame {frame}: {}",
                    message(&err)
                )));
            }
        }
        self.decoded_any = true;
        let decoded = copy_image(&image);
        // SAFETY: releases the buffer the shim locked for us, once.
        unsafe { luna_avf_image_release(&mut image) };
        decoded.map(|image| Frame {
            image,
            hint: self.hint,
        })
    }
}

fn message(err: &[c_char]) -> String {
    let bytes: Vec<u8> = err
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// (chroma_format_idc, bit depth) of the stream from its `hvcC` or `avcC`
/// record body; 8-bit 4:2:0 when the record doesn't say.
fn stream_format(hevc: bool, config: &[u8]) -> (u8, u8) {
    let found = if hevc {
        // ISO/IEC 14496-15 8.3.3: chromaFormat and bitDepthLumaMinus8 sit at
        // fixed offsets after the profile, level and constraint fields.
        (config.len() >= 19).then(|| (config[16] & 3, (config[17] & 7) + 8))
    } else {
        avcc_format(config)
    };
    match found {
        Some((0, bits)) => (1, bits), // monochrome: read as 4:2:0
        Some(f) => f,
        None => (1, 8),
    }
}

/// The High-profile extension of an `avcC` record (ISO/IEC 14496-15 5.3.3.1),
/// or what the profile implies when an encoder left it out.
fn avcc_format(b: &[u8]) -> Option<(u8, u8)> {
    let profile = *b.get(1)?;
    let high = matches!(
        profile,
        100 | 110 | 122 | 144 | 244 | 44 | 83 | 86 | 118 | 128
    );
    let ext = || -> Option<(u8, u8)> {
        let mut i = 5;
        let sps = (*b.get(i)? & 0x1f) as usize;
        i += 1;
        for _ in 0..sps {
            i += 2 + u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize;
        }
        let pps = *b.get(i)? as usize;
        i += 1;
        for _ in 0..pps {
            i += 2 + u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize;
        }
        Some((*b.get(i)? & 3, (*b.get(i + 1)? & 7) + 8))
    };
    if high && let Some(f) = ext() {
        return Some(f);
    }
    Some(match profile {
        110 => (1, 10),
        122 => (2, 10),
        244 | 44 => (3, 10),
        _ => (1, 8),
    })
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

/// The CoreVideo bi-planar formats we read (CVPixelBuffer.h), as
/// (format, chroma_format_idc, bits, full range).
const FORMATS: [(u32, u8, u8, bool); 12] = [
    (fourcc(b"420v"), 1, 8, false),
    (fourcc(b"420f"), 1, 8, true),
    (fourcc(b"422v"), 2, 8, false),
    (fourcc(b"422f"), 2, 8, true),
    (fourcc(b"444v"), 3, 8, false),
    (fourcc(b"444f"), 3, 8, true),
    (fourcc(b"x420"), 1, 10, false),
    (fourcc(b"xf20"), 1, 10, true),
    (fourcc(b"x422"), 2, 10, false),
    (fourcc(b"xf22"), 2, 10, true),
    (fourcc(b"x444"), 3, 10, false),
    (fourcc(b"xf44"), 3, 10, true),
];

/// The output format for a stream: its own subsampling, 10-bit for anything
/// deeper than 8, and its own range.
fn pixel_format(chroma: u8, bits: u8, full_range: bool) -> u32 {
    let bits = if bits > 8 { 10 } else { 8 };
    FORMATS
        .iter()
        .find(|f| (f.1, f.2, f.3) == (chroma.clamp(1, 3), bits, full_range))
        .map(|f| f.0)
        .unwrap_or(FORMATS[0].0)
}

/// Copies a locked pixel buffer into planar samples, cropped to its clean
/// aperture.
fn copy_image(im: &Image) -> Result<Decoded, DecodeError> {
    let Some(&(_, chroma, bits, full_range)) = FORMATS.iter().find(|f| f.0 == im.pixel_format)
    else {
        return Err(DecodeError::Failed(format!(
            "AVFoundation chose an unexpected output format ({:08x})",
            im.pixel_format
        )));
    };
    let (w, h) = (im.width as usize, im.height as usize);
    if w == 0 || h == 0 || w > 16384 || h > 16384 || im.planes.iter().any(|p| p.is_null()) {
        return Err(DecodeError::Failed(format!(
            "AVFoundation frame size {w}×{h}"
        )));
    }
    let (sx, sy) = match chroma {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    let bytes = (bits as usize).div_ceil(8);
    let need = [w * bytes, w.div_ceil(sx) * 2 * bytes];
    let rows = [h, h.div_ceil(sy)];
    for p in 0..2 {
        if im.bytes_per_row[p] < need[p] || im.plane_height[p] < rows[p] {
            return Err(DecodeError::Failed("The decoded frame is short".into()));
        }
    }
    // SAFETY: the shim holds the buffer locked; each plane is
    // bytes_per_row × plane_height bytes.
    let planes = [0, 1].map(|p| unsafe {
        std::slice::from_raw_parts(im.planes[p], im.bytes_per_row[p] * im.plane_height[p])
    });
    let mut crop = (im.crop_x, im.crop_y, im.crop_w, im.crop_h);
    if crop.2 == 0 || crop.3 == 0 || crop.0 + crop.2 > im.width || crop.1 + crop.3 > im.height {
        crop = (0, 0, im.width, im.height);
    }
    let layout = Layout {
        chroma: (sx, sy),
        bits,
        pitch: im.bytes_per_row,
        crop,
    };
    let mut d = copy_planes(planes, &layout);
    if let Decoded::Ycbcr {
        height,
        matrix,
        full_range: fr,
        ..
    } = &mut d
    {
        *matrix = util::matrix_for(im.matrix, *height);
        *fr = full_range;
    }
    Ok(d)
}

struct Layout {
    /// Chroma subsampling (horizontal, vertical).
    chroma: (usize, usize),
    bits: u8,
    pitch: [usize; 2],
    /// x, y, width, height of the display area.
    crop: (u32, u32, u32, u32),
}

/// Y plane, then interleaved CbCr; 8-bit samples, or 16-bit little-endian with
/// the value in the top 10 bits.
fn copy_planes(planes: [&[u8]; 2], l: &Layout) -> Decoded {
    let (sx, sy) = l.chroma;
    let (cx, cy) = (l.crop.0 as usize & !(sx - 1), l.crop.1 as usize & !(sy - 1));
    let (w, h) = (l.crop.2 as usize, l.crop.3 as usize);
    let (cw, ch) = (w.div_ceil(sx), h.div_ceil(sy));
    let sample = |row: &[u8], i: usize| -> u16 {
        if l.bits == 8 {
            row[i] as u16
        } else {
            u16::from_le_bytes([row[2 * i], row[2 * i + 1]]) >> 6
        }
    };
    let mut y = Vec::with_capacity(w * h);
    for r in cy..cy + h {
        let row = &planes[0][r * l.pitch[0]..(r + 1) * l.pitch[0]];
        y.extend((cx..cx + w).map(|i| sample(row, i)));
    }
    let mut cb = Vec::with_capacity(cw * ch);
    let mut cr = Vec::with_capacity(cw * ch);
    for r in cy / sy..cy / sy + ch {
        let row = &planes[1][r * l.pitch[1]..(r + 1) * l.pitch[1]];
        for i in cx / sx..cx / sx + cw {
            cb.push(sample(row, 2 * i));
            cr.push(sample(row, 2 * i + 1));
        }
    }
    Decoded::Ycbcr {
        width: w as u32,
        height: h as u32,
        chroma_width: cw as u32,
        chroma_height: ch as u32,
        bits: l.bits,
        y,
        cb,
        cr,
        matrix: util::matrix_for(2, h as u32),
        full_range: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hvcc_gives_chroma_and_depth() {
        let mut rec = vec![0u8; 23];
        rec[16] = 0xfc | 2; // 4:2:2
        rec[17] = 0xf8 | 2; // 10-bit
        assert_eq!(stream_format(true, &rec), (2, 10));
        assert_eq!(stream_format(true, &rec[..10]), (1, 8));
    }

    #[test]
    fn avcc_high_extension_and_profile_fallback() {
        // profile 122, one 2-byte SPS, one 1-byte PPS, then 4:2:2 10-bit.
        let rec = [
            1,
            122,
            0,
            40,
            0xff,
            0xe1,
            0,
            2,
            0x67,
            0x7a,
            1,
            0,
            1,
            0x68,
            0xfc | 2,
            0xf8 | 2,
            0xf8 | 2,
            0,
        ];
        assert_eq!(stream_format(false, &rec), (2, 10));
        // Same profile with the extension cut off: what the profile implies.
        assert_eq!(stream_format(false, &rec[..14]), (2, 10));
        // Main profile has no extension.
        assert_eq!(stream_format(false, &[1, 77, 0, 40, 0xff, 0xe0, 0]), (1, 8));
        assert_eq!(stream_format(false, &[]), (1, 8));
    }

    #[test]
    fn pixel_formats_follow_the_stream() {
        assert_eq!(pixel_format(1, 8, false), fourcc(b"420v"));
        assert_eq!(pixel_format(2, 10, true), fourcc(b"xf22"));
        assert_eq!(pixel_format(3, 12, false), fourcc(b"x444"));
        assert_eq!(pixel_format(0, 8, true), fourcc(b"420f"));
    }

    fn planes(d: &Decoded) -> (u32, u32, u32, u32, &[u16], &[u16], &[u16]) {
        let Decoded::Ycbcr {
            width,
            height,
            chroma_width,
            chroma_height,
            y,
            cb,
            cr,
            ..
        } = d
        else {
            panic!("not ycbcr")
        };
        (*width, *height, *chroma_width, *chroma_height, y, cb, cr)
    }

    #[test]
    fn eight_bit_420_with_padding_and_crop() {
        // 4x4 luma, pitch 6, display 4x2 at the top.
        let pitch = 6;
        let mut luma = vec![0u8; pitch * 4];
        for r in 0..4 {
            for c in 0..4 {
                luma[r * pitch + c] = (r * 10 + c) as u8;
            }
        }
        let mut chroma = vec![0u8; pitch * 2];
        for r in 0..2 {
            for c in 0..2 {
                chroma[r * pitch + 2 * c] = 100 + (r * 2 + c) as u8;
                chroma[r * pitch + 2 * c + 1] = 200 + (r * 2 + c) as u8;
            }
        }
        let l = Layout {
            chroma: (2, 2),
            bits: 8,
            pitch: [pitch, pitch],
            crop: (0, 0, 4, 2),
        };
        let d = copy_planes([&luma, &chroma], &l);
        let (w, h, cw, ch, y, cb, cr) = planes(&d);
        assert_eq!((w, h, cw, ch), (4, 2, 2, 1));
        assert_eq!(y, &[0, 1, 2, 3, 10, 11, 12, 13]);
        assert_eq!(cb, &[100, 101]);
        assert_eq!(cr, &[200, 201]);
    }

    #[test]
    fn ten_bit_422_takes_the_top_bits_with_full_height_chroma() {
        let put = |v: &[u16]| {
            v.iter()
                .flat_map(|s| (s << 6).to_le_bytes())
                .collect::<Vec<u8>>()
        };
        let luma = put(&[1, 2, 3, 4]);
        let chroma = put(&[50, 60, 70, 80]);
        let l = Layout {
            chroma: (2, 1),
            bits: 10,
            pitch: [4, 4],
            crop: (0, 0, 2, 2),
        };
        let d = copy_planes([&luma, &chroma], &l);
        let (_, _, cw, ch, y, cb, cr) = planes(&d);
        assert_eq!((cw, ch), (1, 2));
        assert_eq!(y, &[1, 2, 3, 4]);
        assert_eq!(cb, &[50, 70]);
        assert_eq!(cr, &[60, 80]);
    }
}
