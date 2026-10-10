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

    let mut y = vec![0u16; width * height];
    let mut cb = vec![0u16; chroma_width * height];
    let mut cr = vec![0u16; chroma_width * height];

    if hdr.interlace == Interlace::Progressive {
        let layout = parse_picture(pictures, hdr.width, hdr.height.div_ceil(16))?;
        let planes = [
            PlaneBand {
                data: &mut y,
                stride: width,
                rows: height,
            },
            PlaneBand {
                data: &mut cb,
                stride: chroma_width,
                rows: height,
            },
            PlaneBand {
                data: &mut cr,
                stride: chroma_width,
                rows: height,
            },
        ];
        decode_picture(&layout, &ctx, planes, threads)?;
    } else {
        // Both fields are coded with the macroblock height of the taller one.
        let mb_height = hdr.height.div_ceil(32);
        let field_rows = mb_height * 16 / ds;
        let mut fields = Vec::with_capacity(2);
        let mut rest = pictures;
        for _ in 0..2 {
            let layout = parse_picture(rest, hdr.width, mb_height)?;
            rest = &rest[layout.size..];
            let mut fy = vec![0u16; width * field_rows];
            let mut fcb = vec![0u16; chroma_width * field_rows];
            let mut fcr = vec![0u16; chroma_width * field_rows];
            let planes = [
                PlaneBand {
                    data: &mut fy,
                    stride: width,
                    rows: field_rows,
                },
                PlaneBand {
                    data: &mut fcb,
                    stride: chroma_width,
                    rows: field_rows,
                },
                PlaneBand {
                    data: &mut fcr,
                    stride: chroma_width,
                    rows: field_rows,
                },
            ];
            decode_picture(&layout, &ctx, planes, threads)?;
            fields.push([fy, fcb, fcr]);
        }
        if hdr.interlace == Interlace::BottomFieldFirst {
            fields.swap(0, 1);
        }
        // Output line j comes from field j % 2 (0 = top), line j / 2.
        for (plane, (out, stride)) in [
            (&mut y, width),
            (&mut cb, chroma_width),
            (&mut cr, chroma_width),
        ]
        .into_iter()
        .enumerate()
        {
            for (j, line) in out.chunks_exact_mut(stride).enumerate() {
                let src = &fields[j & 1][plane];
                let at = (j >> 1) * stride;
                line.copy_from_slice(&src[at..at + stride]);
            }
        }
    }

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
