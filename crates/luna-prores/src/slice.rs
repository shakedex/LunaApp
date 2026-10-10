//! Slice decoding: slice header, per-component entropy decoding (DC then
//! interleaved AC run/level), dequantisation, inverse transform and sample
//! reconstruction into the output planes.

use crate::Error;
use crate::bits::{BitReader, CachedReader, LUT_BITS, end_of_ones};
use crate::idct::idct8x8;
use crate::tables::{DC, FIRST_DC, LEVEL, RUN, ac_luts};

fn corrupt(msg: &str) -> Error {
    Error::Corrupt(msg.into())
}

/// Everything constant across the slices of one picture.
pub(crate) struct PictureCtx<'a> {
    pub qmat_luma: &'a [u8; 64],
    pub qmat_chroma: &'a [u8; 64],
    pub scan: &'a [u8; 64],
    pub chroma444: bool,
    pub alpha: bool,
    /// 1, 2, 4 or 8.
    pub downscale: usize,
    /// 10 or 12.
    pub bits: u8,
}

/// A horizontal band of one output plane.
pub(crate) struct PlaneBand<'a> {
    pub data: &'a mut [u16],
    pub stride: usize,
    pub rows: usize,
}

/// Per-thread working memory, reused across slices.
#[derive(Default)]
pub(crate) struct Scratch {
    coefs: Vec<f32>,
    has_ac: Vec<bool>,
    padded: Vec<u8>,
}

/// Quantiser scale for a slice's quantisation index.
fn qscale(index: u8) -> Result<i32, Error> {
    match index {
        0 => Err(corrupt("zero quantisation index")),
        1..=128 => Ok(index as i32),
        _ => Ok((index as i32 - 96) * 4),
    }
}

/// Maps a codeword to a signed value: 0, -1, 1, -2, 2, …
#[inline]
fn signed(code: u32) -> i32 {
    ((code >> 1) as i32) ^ -((code & 1) as i32)
}

/// Entropy-decodes one component of a slice into dequantised coefficient
/// blocks (raster order within each block). With `dc_only` the AC data is
/// not parsed at all.
fn decode_component(
    data: &[u8],
    log2_blocks: u32,
    factors: &[f32; 64],
    scan: &[u8; 64],
    dc_only: bool,
    scratch: &mut Scratch,
) -> Result<(), Error> {
    let blocks = 1usize << log2_blocks;
    let coefs = &mut scratch.coefs[..blocks * 64];
    let has_ac = &mut scratch.has_ac[..blocks];
    coefs.fill(0.0);
    has_ac.fill(false);
    let bad = || corrupt("bad coefficient code");

    // A zero-padded copy keeps every 64-bit peek on the fast path.
    let padded = &mut scratch.padded;
    padded.clear();
    padded.extend_from_slice(data);
    padded.resize(data.len() + 16, 0);
    let mut r = BitReader::padded(padded, data.len());
    let mut dc = signed(r.read_code(FIRST_DC).ok_or_else(bad)?);
    coefs[0] = dc as f32 * factors[0];
    let mut prev = 5u32;
    let mut negative = false;
    for b in 1..blocks {
        let code = r.read_code(DC[prev.min(6) as usize]).ok_or_else(bad)?;
        // An odd codeword flips the running sign; zero resets it.
        if code == 0 {
            negative = false;
        } else if code & 1 == 1 {
            negative = !negative;
        }
        let magnitude = code.div_ceil(2) as i32;
        dc = dc.saturating_add(if negative { -magnitude } else { magnitude });
        coefs[b * 64] = dc as f32 * factors[0];
        prev = code;
    }

    if !dc_only {
        let end = end_of_ones(data);
        let limit = 64 << log2_blocks;
        let mask = blocks - 1;
        let mut pos = blocks - 1;
        let luts = ac_luts();
        let mut run = 4usize;
        let mut level = 2usize;
        let mut c = CachedReader::at(padded, r.position());
        while c.position() < end {
            // After a refill at least 33 bits are buffered: enough for a
            // table-decoded run and level (≤ LUT_BITS each).
            c.refill();
            let prev_run = run.min(15);
            let entry = luts.run(prev_run).0[(c.peek() >> (64 - LUT_BITS)) as usize];
            if entry != 0 {
                c.consume(entry & 0xff);
                run = (entry >> 8) as usize;
            } else {
                r.seek(c.position());
                run = r.read_code(RUN[prev_run]).ok_or_else(bad)? as usize;
                c = CachedReader::at(padded, r.position());
            }
            pos += run + 1;
            if pos >= limit {
                return Err(corrupt("coefficient run past the block"));
            }
            let prev_level = level.min(9);
            let entry = luts.level(prev_level).0[(c.peek() >> (64 - LUT_BITS)) as usize];
            let negative = if entry != 0 {
                c.consume(entry & 0xff);
                level = (entry >> 9) as usize;
                entry & 0x100 != 0
            } else {
                r.seek(c.position());
                level = r.read_code(LEVEL[prev_level]).ok_or_else(bad)? as usize + 1;
                let negative = r.read_bit() == 1;
                c = CachedReader::at(padded, r.position());
                negative
            };
            let block = pos & mask;
            let raster = scan[pos >> log2_blocks] as usize;
            let value = level as f32 * factors[raster];
            coefs[block * 64 + raster] = if negative { -value } else { value };
            has_ac[block] = true;
        }
        r.seek(c.position());
    }
    if r.overrun() {
        return Err(corrupt("component data overrun"));
    }
    Ok(())
}

/// Converts a spatial value to a clipped output sample.
#[inline(always)]
fn to_sample(v: f32, bits: u8) -> u16 {
    // Rounds half up: clipping first keeps the value positive, where
    // truncation equals floor (and avoids a libm call per sample).
    if bits == 10 {
        (v * 0.25 + 512.5).clamp(4.0, 1019.0) as u16
    } else {
        (v + 2048.5).clamp(4.0, 4091.0) as u16
    }
}

/// [`to_sample`] over one row of eight values.
#[inline(always)]
fn to_samples(row: &[f32], bits: u8) -> [u16; 8] {
    let (scale, offset, hi) = if bits == 10 {
        (0.25, 512.5, 1019.0)
    } else {
        (1.0, 2048.5, 4091.0)
    };
    std::array::from_fn(|i| (row[i] * scale + offset).clamp(4.0, hi) as u16)
}

/// Writes one reconstructed block whose full-resolution top-left corner is
/// (`x`, `y`) in band coordinates.
#[allow(clippy::too_many_arguments)]
fn put_block(
    coefs: &[f32],
    has_ac: bool,
    x: usize,
    y: usize,
    ctx: &PictureCtx,
    band: &mut PlaneBand,
) {
    let ds = ctx.downscale;
    let size = 8 / ds;
    let (ox, oy) = (x / ds, y / ds);
    if ox >= band.stride || oy >= band.rows {
        return;
    }
    let cols = size.min(band.stride - ox);
    let rows = size.min(band.rows - oy);
    let block: &[f32; 64] = coefs[..64].try_into().unwrap();

    if !has_ac || ds == 8 {
        let s = to_sample(block[0] / 8.0, ctx.bits);
        for r in 0..rows {
            let at = (oy + r) * band.stride + ox;
            band.data[at..at + cols].fill(s);
        }
        return;
    }

    let mut spatial = [0f32; 64];
    idct8x8(block, &mut spatial);
    if ds == 1 {
        for r in 0..rows {
            let at = (oy + r) * band.stride + ox;
            let row = to_samples(&spatial[r * 8..r * 8 + 8], ctx.bits);
            band.data[at..at + cols].copy_from_slice(&row[..cols]);
        }
    } else {
        let n = (ds * ds) as u32;
        for r in 0..rows {
            let at = (oy + r) * band.stride + ox;
            for c in 0..cols {
                let mut sum = 0u32;
                for dy in 0..ds {
                    for dx in 0..ds {
                        sum += to_sample(spatial[(r * ds + dy) * 8 + c * ds + dx], ctx.bits) as u32;
                    }
                }
                band.data[at + c] = ((sum + n / 2) / n) as u16;
            }
        }
    }
}

fn be16(b: &[u8], at: usize) -> usize {
    u16::from_be_bytes([b[at], b[at + 1]]) as usize
}

/// Decodes one slice `mbs` macroblocks wide whose top-left macroblock is at
/// column `mb_x` and row `mb_y` of the band.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_slice(
    data: &[u8],
    mb_x: usize,
    mb_y: usize,
    mbs: usize,
    ctx: &PictureCtx,
    planes: &mut [PlaneBand; 3],
    scratch: &mut Scratch,
) -> Result<(), Error> {
    let min_header = if ctx.alpha { 8 } else { 6 };
    if data.len() < min_header {
        return Err(corrupt("slice header truncated"));
    }
    let header_size = (data[0] >> 3) as usize;
    if header_size < min_header || header_size > data.len() {
        return Err(corrupt("bad slice header size"));
    }
    let qs = qscale(data[1])?;
    let y_size = be16(data, 2);
    let cb_size = be16(data, 4);
    let body = &data[header_size..];
    if y_size + cb_size > body.len() {
        return Err(corrupt("slice component sizes exceed the slice"));
    }
    let cr_size = if ctx.alpha {
        be16(data, 6)
    } else {
        body.len() - y_size - cb_size
    };
    if y_size + cb_size + cr_size > body.len() {
        return Err(corrupt("slice component sizes exceed the slice"));
    }
    let (y_data, rest) = body.split_at(y_size);
    let (cb_data, rest) = rest.split_at(cb_size);
    let cr_data = &rest[..cr_size];

    let log2_mbs = mbs.trailing_zeros();
    let dc_only = ctx.downscale == 8;
    let blocks = mbs * 4;
    if scratch.coefs.len() < blocks * 64 {
        scratch.coefs.resize(blocks * 64, 0.0);
        scratch.has_ac.resize(blocks, false);
    }

    let factors =
        |m: &[u8; 64]| -> [f32; 64] { std::array::from_fn(|i| (m[i] as i32 * qs) as f32) };
    let luma_factors = factors(ctx.qmat_luma);
    let chroma_factors = factors(ctx.qmat_chroma);

    let y0 = mb_y * 16;
    // Luma: four blocks per macroblock, left-to-right then top-to-bottom.
    decode_component(
        y_data,
        log2_mbs + 2,
        &luma_factors,
        ctx.scan,
        dc_only,
        scratch,
    )?;
    for b in 0..blocks {
        let x = (mb_x + (b >> 2)) * 16 + (b & 1) * 8;
        let y = y0 + ((b >> 1) & 1) * 8;
        put_block(
            &scratch.coefs[b * 64..],
            scratch.has_ac[b],
            x,
            y,
            ctx,
            &mut planes[0],
        );
    }

    let (log2_chroma, chroma_blocks) = if ctx.chroma444 {
        (log2_mbs + 2, blocks)
    } else {
        (log2_mbs + 1, mbs * 2)
    };
    for (plane, comp) in [(1usize, cb_data), (2, cr_data)] {
        decode_component(
            comp,
            log2_chroma,
            &chroma_factors,
            ctx.scan,
            dc_only,
            scratch,
        )?;
        for b in 0..chroma_blocks {
            let (x, y) = if ctx.chroma444 {
                (
                    (mb_x + (b >> 2)) * 16 + ((b >> 1) & 1) * 8,
                    y0 + (b & 1) * 8,
                )
            } else {
                ((mb_x + (b >> 1)) * 8, y0 + (b & 1) * 8)
            };
            put_block(
                &scratch.coefs[b * 64..],
                scratch.has_ac[b],
                x,
                y,
                ctx,
                &mut planes[plane],
            );
        }
    }
    Ok(())
}
