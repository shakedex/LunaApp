//! A clean-room Apple ProRes decoder written from SMPTE RDD 36.
//!
//! Decodes one coded frame (the `icpf` frame container) into planar Y′CbCr
//! (4:2:2 or 4:4:4, 10-/12-bit significant bits held in u16). An alpha
//! channel, when present, is skipped (its coded size is known from the slice
//! header). Never panics on bad input.
//!
//! Every profile (Proxy, LT, 422, HQ, 4444, 4444 XQ), progressive and
//! interlaced frames, custom quantisation matrices and all slice widths are
//! supported. Samples are clipped to 4..=1019 (10-bit) or 4..=4091 (12-bit).
//!
//! The inverse DCT follows RDD 36's mathematical definition accurately. The
//! spec does not mandate a bit-exact integer transform, so other decoders'
//! fixed-point IDCTs can land one code away on some samples (against
//! FFmpeg's 12-bit path, about 15% of samples differ by exactly one; nothing
//! differs by more).

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

mod bits;
mod header;
mod idct;
mod slice;
mod tables;

use header::{Interlace, PictureLayout, parse_frame, parse_picture};
use slice::{PictureCtx, PlaneBand, Scratch, decode_slice};

/// Decodes a whole frame on the calling thread. `downscale` of 1 is full
/// resolution; 2 and 4 box-average each reconstructed block; 8 decodes only
/// the DC coefficients (one sample per 8×8 block). Reduced output planes are
/// `ceil(width / downscale)` × `ceil(height / downscale)`.
pub fn decode(frame: &[u8], downscale: u8) -> Result<Picture, Error> {
    decode_with_threads(frame, downscale, 1)
}

/// [`decode`], spreading macroblock rows over up to `threads` scoped threads.
pub fn decode_with_threads(frame: &[u8], downscale: u8, threads: usize) -> Result<Picture, Error> {
    let ds = match downscale {
        1 | 2 | 4 | 8 => downscale as usize,
        _ => return Err(Error::Unsupported(format!("downscale {downscale}"))),
    };
    let threads = threads.max(1);
    let (hdr, pictures) = parse_frame(frame)?;
    let chroma444 = hdr.chroma_format == 3;
    let bits = if chroma444 { 12 } else { 10 };
    let width = hdr.width.div_ceil(ds);
    let height = hdr.height.div_ceil(ds);
    let chroma_width = if chroma444 { width } else { width.div_ceil(2) };
    let ctx = PictureCtx {
        qmat_luma: &hdr.qmat_luma,
        qmat_chroma: &hdr.qmat_chroma,
        scan: if hdr.interlace == Interlace::Progressive {
            &tables::PROGRESSIVE_SCAN
        } else {
            &tables::INTERLACED_SCAN
        },
        chroma444,
        alpha: hdr.alpha_type != 0,
        downscale: ds,
        bits,
    };

    // Parse every picture's slice table before allocating anything, so a
    // corrupt header cannot request huge planes.
    let mut layouts = Vec::with_capacity(2);
    if hdr.interlace == Interlace::Progressive {
        layouts.push(parse_picture(pictures, hdr.width, hdr.height.div_ceil(16))?);
    } else {
        // Both fields are coded with the macroblock height of the taller one.
        let mut rest = pictures;
        for _ in 0..2 {
            let layout = parse_picture(rest, hdr.width, hdr.height.div_ceil(32))?;
            rest = &rest[layout.size..];
            layouts.push(layout);
        }
    }

    let mut out = Planes::new(width, chroma_width, height);
    if let [layout] = layouts.as_slice() {
        decode_picture(layout, &ctx, out.bands(), threads)?;
    } else {
        // Each output line spans `ds` frame lines: `ds / 2` from each field.
        // So fields decode at half the reduction (full size when ds is 1)
        // and are then merged: interleaved at full size, otherwise averaged
        // over the two fields and horizontal pairs, which keeps the result a
        // box filter of the frame.
        let field_ds = (ds / 2).max(1);
        let field_ctx = PictureCtx {
            downscale: field_ds,
            ..ctx
        };
        let field_width = hdr.width.div_ceil(field_ds);
        let field_chroma = if chroma444 {
            field_width
        } else {
            field_width.div_ceil(2)
        };
        let field_rows = layouts[0].mb_height * 16 / field_ds;
        let mut fields = Vec::with_capacity(2);
        for layout in &layouts {
            let mut field = Planes::new(field_width, field_chroma, field_rows);
            decode_picture(layout, &field_ctx, field.bands(), threads)?;
            fields.push(field);
        }
        if hdr.interlace == Interlace::BottomFieldFirst {
            fields.swap(0, 1);
        }
        let [top, bottom] = &mut fields[..] else {
            unreachable!("two fields")
        };
        for plane in 0..3 {
            let stride = out.stride(plane);
            let field_stride = top.stride(plane);
            let (top, bottom) = (top.plane_mut(plane), bottom.plane_mut(plane));
            for (j, line) in out.plane_mut(plane).chunks_exact_mut(stride).enumerate() {
                if ds == 1 {
                    // Line j comes from field j % 2 (0 = top), line j / 2.
                    let src = if j & 1 == 0 { &*top } else { &*bottom };
                    let at = (j >> 1) * stride;
                    line.copy_from_slice(&src[at..at + stride]);
                    continue;
                }
                let row = j * field_stride;
                for (i, out) in line.iter_mut().enumerate() {
                    let (mut sum, mut n) = (0u32, 0u32);
                    for x in [2 * i, 2 * i + 1] {
                        if x < field_stride {
                            sum += top[row + x] as u32 + bottom[row + x] as u32;
                            n += 2;
                        }
                    }
                    *out = ((sum + n / 2) / n) as u16;
                }
            }
        }
    }
    let Planes { y, cb, cr, .. } = out;

    Ok(Picture {
        width: width as u32,
        height: height as u32,
        chroma_format: hdr.chroma_format,
        y,
        cb,
        cr,
        bits,
        color_primaries: hdr.color_primaries,
        transfer_characteristic: hdr.transfer_characteristic,
        matrix_coefficients: hdr.matrix_coefficients,
    })
}

/// Owned output planes.
struct Planes {
    y: Vec<u16>,
    cb: Vec<u16>,
    cr: Vec<u16>,
    width: usize,
    chroma_width: usize,
    rows: usize,
}

impl Planes {
    fn new(width: usize, chroma_width: usize, rows: usize) -> Self {
        Self {
            y: vec![0; width * rows],
            cb: vec![0; chroma_width * rows],
            cr: vec![0; chroma_width * rows],
            width,
            chroma_width,
            rows,
        }
    }

    fn stride(&self, plane: usize) -> usize {
        if plane == 0 {
            self.width
        } else {
            self.chroma_width
        }
    }

    fn plane_mut(&mut self, plane: usize) -> &mut [u16] {
        match plane {
            0 => &mut self.y,
            1 => &mut self.cb,
            _ => &mut self.cr,
        }
    }

    fn bands(&mut self) -> [PlaneBand<'_>; 3] {
        let rows = self.rows;
        [
            PlaneBand {
                data: &mut self.y,
                stride: self.width,
                rows,
            },
            PlaneBand {
                data: &mut self.cb,
                stride: self.chroma_width,
                rows,
            },
            PlaneBand {
                data: &mut self.cr,
                stride: self.chroma_width,
                rows,
            },
        ]
    }
}

/// Splits `band` after `rows` rows (clamped to what it has).
fn split_band(band: PlaneBand<'_>, rows: usize) -> (PlaneBand<'_>, PlaneBand<'_>) {
    let rows = rows.min(band.rows);
    let (top, bottom) = band.data.split_at_mut(rows * band.stride);
    (
        PlaneBand {
            data: top,
            stride: band.stride,
            rows,
        },
        PlaneBand {
            data: bottom,
            stride: band.stride,
            rows: band.rows - rows,
        },
    )
}

/// Decodes the macroblock rows `first..first + count` into `planes`, whose
/// row 0 is the top of macroblock row `first`.
fn decode_rows(
    layout: &PictureLayout,
    ctx: &PictureCtx,
    first: usize,
    count: usize,
    mut planes: [PlaneBand; 3],
) -> Result<(), Error> {
    let mut scratch = Scratch::default();
    let per_row = layout.row_slices.len();
    for row in first..first + count {
        let mut mb_x = 0;
        for (i, &mbs) in layout.row_slices.iter().enumerate() {
            let data = layout.slices[row * per_row + i];
            decode_slice(data, mb_x, row - first, mbs, ctx, &mut planes, &mut scratch)?;
            mb_x += mbs;
        }
    }
    Ok(())
}

/// Decodes one picture, optionally over several threads by macroblock row.
fn decode_picture(
    layout: &PictureLayout,
    ctx: &PictureCtx,
    planes: [PlaneBand; 3],
    threads: usize,
) -> Result<(), Error> {
    let rows_per_mb = 16 / ctx.downscale;
    let threads = threads.min(layout.mb_height).max(1);
    if threads == 1 {
        return decode_rows(layout, ctx, 0, layout.mb_height, planes);
    }
    let per_thread = layout.mb_height.div_ceil(threads);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        let mut rest = planes;
        let mut first = 0;
        while first < layout.mb_height {
            let count = per_thread.min(layout.mb_height - first);
            let [y, cb, cr] = rest;
            let (y, y_rest) = split_band(y, count * rows_per_mb);
            let (cb, cb_rest) = split_band(cb, count * rows_per_mb);
            let (cr, cr_rest) = split_band(cr, count * rows_per_mb);
            rest = [y_rest, cb_rest, cr_rest];
            let start = first;
            handles.push(scope.spawn(move || decode_rows(layout, ctx, start, count, [y, cb, cr])));
            first += count;
        }
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(Error::Corrupt("decoder thread panicked".into())))
            })
            .collect::<Result<Vec<()>, Error>>()
            .map(|_| ())
    })
}
