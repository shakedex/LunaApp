//! ARRIRAW: uncompressed, bit-packed sensor data. We read the frame bytes from
//! the parser's index and unpack them ourselves.
//!
//! - **Packing.** ALEXA 35 (13-bit) packs samples LSB-first into a continuous
//!   little-endian bitstream; checked on the T_CAM sample, where it gives smooth
//!   neighbouring values and every other order gives noise. Older ALEXA 12-bit
//!   ARRIRAW is documented elsewhere (dcraw/LibRaw's ARRI loader) as MSB-first
//!   within little-endian 32-bit words. We have no 12-bit sample, so for any
//!   depth both orders are tried on a strip of the frame and the smoother one
//!   wins.
//! - **Layout.** The picture is the *last* `W·H·bits/8` bytes of the frame: MXF
//!   frames lead with 0xFF fill, `.ari` files with their 4096-byte header.
//! - **CFA.** ALEXA 35: RGGB. Older ALEXA: GRBG (dcraw's ARRI filter pattern,
//!   unverified here).
//! - **Levels.** 12-bit: black 256, white 4095 (dcraw/LibRaw). 13-bit: no ARRI
//!   figure to hand; black 256 is estimated from the sample (lowest code in a
//!   frame is 260, with sensor noise around it), white is the 13-bit maximum.

use std::path::Path;

use luna_formats::{ClipFiles, ColourHint, FrameIndex, Gamut, Probe, Transfer};

use crate::mosaic::{self, Rect};
use crate::util;
use crate::{Cfa, DecodeError, Decoded, Frame};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let layout = probe.layout;
    let (w, h) = (layout.stored_width as usize, layout.stored_height as usize);
    let bits = layout.raw_bits.unwrap_or(12);
    if w == 0 || h == 0 || !w.is_multiple_of(2) || !h.is_multiple_of(2) {
        return util::all_err(
            positions.len(),
            DecodeError::Failed(format!("ARRIRAW frame size is unknown ({w}×{h})")),
        );
    }
    if !(10..=16).contains(&bits) {
        return util::all_err(
            positions.len(),
            DecodeError::Unsupported(format!("{bits}-bit ARRIRAW isn't supported")),
        );
    }
    let count = match &probe.frames {
        FrameIndex::Ranges(r) => r.len(),
        FrameIndex::Files(f) => f.len(),
        FrameIndex::DecoderOwned => 0,
    } as u64;
    let f = mosaic::bin_factor(w, min_width);
    let (cfa, gamut, black) = if bits >= 13 {
        (Cfa::Rggb, Gamut::ArriWideGamut4, 256.0)
    } else {
        (Cfa::Grbg, Gamut::ArriWideGamut3, 256.0)
    };
    let white = ((1u32 << bits) - 1) as f32;
    let hint = ColourHint {
        transfer: Some(Transfer::Linear),
        gamut: probe.meta.colour.gamut.or(Some(gamut)),
    };
    util::per_frame("ARRIRAW", positions, count, |i| {
        let bytes = read_frame(clip, &probe.frames, i as usize)?;
        let samples = unpack_frame(&bytes, w, h, bits)?;
        let all = Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        };
        let (data, ow, oh) = mosaic::crop_bin(&samples, w, h, all, f);
        Ok(Frame {
            image: Decoded::Mosaic {
                width: ow as u32,
                height: oh as u32,
                cfa,
                data,
                black,
                white,
                wb: None,
                camera_to_xyz: None,
            },
            hint,
        })
    })
}

fn read_frame(clip: &ClipFiles, frames: &FrameIndex, i: usize) -> Result<Vec<u8>, DecodeError> {
    match frames {
        FrameIndex::Ranges(r) => {
            let r = r[i];
            util::read_range(&clip.main, r.offset, r.len)
        }
        FrameIndex::Files(f) => read_file(&f[i]),
        FrameIndex::DecoderOwned => Err(DecodeError::Failed(
            "ARRIRAW frames were not indexed".into(),
        )),
    }
}

fn read_file(p: &Path) -> Result<Vec<u8>, DecodeError> {
    std::fs::read(p).map_err(|e| DecodeError::Failed(format!("Could not read the frame: {e}")))
}

/// How samples are packed into bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Packing {
    /// Continuous bitstream, least significant bit first (ALEXA 35).
    LsbStream,
    /// Most significant bit first within little-endian 32-bit words.
    MsbWords32,
}

/// Unpacks the `w × h` picture at the end of `bytes`.
fn unpack_frame(bytes: &[u8], w: usize, h: usize, bits: u8) -> Result<Vec<u16>, DecodeError> {
    let need = (w * h * bits as usize).div_ceil(8);
    if bytes.len() < need {
        return Err(DecodeError::Failed(format!(
            "ARRIRAW frame is truncated ({} of {need} bytes)",
            bytes.len()
        )));
    }
    let pic = &bytes[bytes.len() - need..];
    let packing = detect_packing(pic, w, h, bits);
    let mut out = vec![0u16; w * h];
    unpack(pic, bits, packing, &mut out);
    Ok(out)
}

/// Tries both packings on two rows from the middle of the picture and keeps the
/// one whose same-colour neighbours differ least.
fn detect_packing(pic: &[u8], w: usize, h: usize, bits: u8) -> Packing {
    // Start on a row whose first sample index is a multiple of 32, so the strip
    // begins on a 32-bit word boundary for both packings (else the top row).
    let start = (h / 2..h.saturating_sub(2))
        .map(|row| row * w)
        .find(|s| s.is_multiple_of(32))
        .unwrap_or(0);
    let n = (2 * w).min(w * h - start);
    let first_byte = start * bits as usize / 8;
    let strip = &pic[first_byte..];
    let rough = |p: Packing| {
        let mut v = vec![0u16; n];
        unpack(strip, bits, p, &mut v);
        v.windows(3)
            .map(|t| (t[0] as i64 - t[2] as i64).unsigned_abs())
            .sum::<u64>()
    };
    if rough(Packing::MsbWords32) < rough(Packing::LsbStream) {
        Packing::MsbWords32
    } else {
        Packing::LsbStream
    }
}

/// Unpacks `out.len()` samples of `bits` each from the start of `src`.
fn unpack(src: &[u8], bits: u8, packing: Packing, out: &mut [u16]) {
    let bits = bits as u32;
    let mask = (1u64 << bits) - 1;
    let mut acc = 0u64;
    let mut have = 0u32;
    match packing {
        Packing::LsbStream => {
            let mut bytes = src.iter();
            for o in out.iter_mut() {
                while have < bits {
                    acc |= (*bytes.next().unwrap_or(&0) as u64) << have;
                    have += 8;
                }
                *o = (acc & mask) as u16;
                acc >>= bits;
                have -= bits;
            }
        }
        Packing::MsbWords32 => {
            let mut words = src.chunks(4);
            for o in out.iter_mut() {
                if have < bits {
                    let word = match words.next() {
                        Some(c) if c.len() == 4 => u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                        Some(c) => {
                            let mut b = [0u8; 4];
                            b[..c.len()].copy_from_slice(c);
                            u32::from_le_bytes(b)
                        }
                        None => 0,
                    };
                    acc = (acc << 32) | word as u64;
                    have += 32;
                }
                *o = ((acc >> (have - bits)) & mask) as u16;
                have -= bits;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_lsb(vals: &[u16], bits: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut acc, mut have) = (0u64, 0u32);
        for &v in vals {
            acc |= (v as u64) << have;
            have += bits;
            while have >= 8 {
                out.push(acc as u8);
                acc >>= 8;
                have -= 8;
            }
        }
        if have > 0 {
            out.push(acc as u8);
        }
        out
    }

    fn pack_msb32(vals: &[u16], bits: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut acc, mut have) = (0u64, 0u32);
        for &v in vals {
            acc = (acc << bits) | v as u64;
            have += bits;
            while have >= 32 {
                let word = (acc >> (have - 32)) as u32;
                out.extend_from_slice(&word.to_le_bytes());
                have -= 32;
            }
        }
        if have > 0 {
            let word = ((acc << (32 - have)) & 0xffff_ffff) as u32;
            out.extend_from_slice(&word.to_le_bytes());
        }
        out
    }

    /// A smooth synthetic mosaic: two interleaved gradients.
    fn picture(w: usize, h: usize, max: u16) -> Vec<u16> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let base = ((x / 2 + y / 2) * 7 % (max as usize / 2)) as u16;
                base + if (x + y) % 2 == 0 { 0 } else { max / 4 }
            })
            .collect()
    }

    #[test]
    fn unpacks_lsb_13_bit_with_leading_fill() {
        let (w, h) = (64, 8);
        let pic = picture(w, h, 8191);
        let mut frame = vec![0xffu8; 40];
        frame.extend(pack_lsb(&pic, 13));
        assert_eq!(unpack_frame(&frame, w, h, 13).unwrap(), pic);
    }

    #[test]
    fn unpacks_msb_words_12_bit() {
        let (w, h) = (64, 8);
        let pic = picture(w, h, 4095);
        let mut frame = vec![0u8; 4096]; // .ari header
        frame.extend(pack_msb32(&pic, 12));
        assert_eq!(unpack_frame(&frame, w, h, 12).unwrap(), pic);
    }

    #[test]
    fn truncated_frames_are_errors() {
        assert!(unpack_frame(&[0u8; 10], 64, 8, 13).is_err());
    }
}
