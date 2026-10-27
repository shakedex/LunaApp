//! Test-only helpers: corpus lookup and minimal frame indexes for the codecs
//! whose real parsers live in `luna-formats` (MXF essence, MOV sample tables).
#![allow(dead_code)]

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use luna_formats::{
    ByteRange, ClipFiles, ClipMeta, Codec, Container, FrameIndex, PictureLayout, Probe,
};

/// A corpus file, or `None` (test skipped) when `LUNA_CORPUS` is unset or the
/// file is missing.
pub fn corpus(rel: &str) -> Option<PathBuf> {
    let root = std::env::var_os("LUNA_CORPUS")?;
    let p = Path::new(&root).join(rel);
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("skipping: {} not found", p.display());
        None
    }
}

pub fn files(container: Container, main: &Path) -> ClipFiles {
    ClipFiles {
        container,
        main: main.to_path_buf(),
        segments: vec![],
        sidecars: vec![],
    }
}

pub fn probe(codec: Codec, frames: FrameIndex, layout: PictureLayout) -> Probe {
    Probe {
        codec,
        meta: ClipMeta::default(),
        frames,
        layout,
        codec_config: None,
        warnings: vec![],
        embedded_lut: None,
    }
}

/// Byte ranges of every MXF picture essence element (GC picture item, key
/// byte 12 = 0x15), walking top-level KLVs.
pub fn mxf_picture_ranges(path: &Path) -> std::io::Result<Vec<ByteRange>> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let mut off = 0u64;
    let mut out = Vec::new();
    let mut head = [0u8; 25];
    while off + 17 <= size {
        f.seek(SeekFrom::Start(off))?;
        let n = f.read(&mut head)?;
        if n < 17 || head[..4] != [0x06, 0x0e, 0x2b, 0x34] {
            break;
        }
        let (len, hl) = if head[16] < 0x80 {
            (head[16] as u64, 17u64)
        } else {
            let nb = (head[16] & 0x7f) as usize;
            if nb == 0 || nb > 8 || 17 + nb > n {
                break;
            }
            let mut l = 0u64;
            for &b in &head[17..17 + nb] {
                l = (l << 8) | b as u64;
            }
            (l, 17 + nb as u64)
        };
        if head[4] == 0x01 && head[8..12] == [0x0d, 0x01, 0x03, 0x01] && head[12] == 0x15 {
            out.push(ByteRange {
                offset: off + hl,
                len,
            });
        }
        off += hl + len;
    }
    Ok(out)
}

fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn be64(b: &[u8], o: usize) -> u64 {
    (be32(b, o) as u64) << 32 | be32(b, o + 4) as u64
}

/// Child boxes of a box body: (type, body).
fn boxes(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut o = 0usize;
    while o + 8 <= data.len() {
        let size = be32(data, o) as usize;
        let typ: [u8; 4] = data[o + 4..o + 8].try_into().unwrap();
        let (hl, size) = match size {
            1 if o + 16 <= data.len() => (16, be64(data, o + 8) as usize),
            0 => (8, data.len() - o),
            s => (8, s),
        };
        if size < hl || o + size > data.len() {
            break;
        }
        out.push((typ, &data[o + hl..o + size]));
        o += size;
    }
    out
}

fn child<'a>(data: &'a [u8], typ: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data)
        .into_iter()
        .find(|(t, _)| t == typ)
        .map(|(_, b)| b)
}

/// Byte ranges of every video sample in a QuickTime/MP4 file, from the first
/// video track's sample table.
pub fn mov_video_ranges(path: &Path) -> std::io::Result<Vec<ByteRange>> {
    let bad = || std::io::Error::other("no video sample table");
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    // Find moov at top level.
    let mut off = 0u64;
    let mut moov = None;
    while off + 8 <= size {
        f.seek(SeekFrom::Start(off))?;
        let mut h = [0u8; 16];
        f.read_exact(&mut h[..8])?;
        let mut len = be32(&h, 0) as u64;
        let mut hl = 8;
        if len == 1 {
            f.read_exact(&mut h[8..16])?;
            len = be64(&h, 8);
            hl = 16;
        } else if len == 0 {
            len = size - off;
        }
        if &h[4..8] == b"moov" {
            let mut body = vec![0u8; (len - hl) as usize];
            f.read_exact(&mut body)?;
            moov = Some(body);
            break;
        }
        if len < hl {
            break;
        }
        off += len;
    }
    let moov = moov.ok_or_else(bad)?;
    for (t, trak) in boxes(&moov) {
        if &t != b"trak" {
            continue;
        }
        let Some(mdia) = child(trak, b"mdia") else {
            continue;
        };
        let Some(hdlr) = child(mdia, b"hdlr") else {
            continue;
        };
        if hdlr.len() < 12 || &hdlr[8..12] != b"vide" {
            continue;
        }
        let stbl = child(mdia, b"minf")
            .and_then(|m| child(m, b"stbl"))
            .ok_or_else(bad)?;
        let stsz = child(stbl, b"stsz").ok_or_else(bad)?;
        let fixed = be32(stsz, 4);
        let count = be32(stsz, 8) as usize;
        let sizes: Vec<u64> = (0..count)
            .map(|i| {
                if fixed != 0 {
                    fixed as u64
                } else {
                    be32(stsz, 12 + 4 * i) as u64
                }
            })
            .collect();
        let chunks: Vec<u64> = if let Some(stco) = child(stbl, b"stco") {
            (0..be32(stco, 4) as usize)
                .map(|i| be32(stco, 8 + 4 * i) as u64)
                .collect()
        } else {
            let co64 = child(stbl, b"co64").ok_or_else(bad)?;
            (0..be32(co64, 4) as usize)
                .map(|i| be64(co64, 8 + 8 * i))
                .collect()
        };
        let stsc = child(stbl, b"stsc").ok_or_else(bad)?;
        let entries: Vec<(usize, usize)> = (0..be32(stsc, 4) as usize)
            .map(|i| {
                (
                    be32(stsc, 8 + 12 * i) as usize,
                    be32(stsc, 12 + 12 * i) as usize,
                )
            })
            .collect();
        let mut out = Vec::with_capacity(count);
        let mut sample = 0usize;
        for (ci, &chunk_off) in chunks.iter().enumerate() {
            let chunk_no = ci + 1;
            let per = entries
                .iter()
                .rev()
                .find(|(first, _)| *first <= chunk_no)
                .map(|e| e.1)
                .unwrap_or(1);
            let mut o = chunk_off;
            for _ in 0..per {
                if sample >= count {
                    break;
                }
                out.push(ByteRange {
                    offset: o,
                    len: sizes[sample],
                });
                o += sizes[sample];
                sample += 1;
            }
        }
        return Ok(out);
    }
    Err(bad())
}

/// Renders a decoded frame to an 8-bit RGB preview for eyeballing: mosaics
/// get a 2×2 superpixel debayer, grey-world balance and a gamma; log RGB is
/// shown as is; Y′CbCr through its matrix.
pub fn preview_rgb(img: &luna_decode::Decoded) -> (u32, u32, Vec<u8>) {
    use luna_decode::{Cfa, Decoded, YcbcrMatrix};
    match img {
        Decoded::Mosaic {
            width,
            height,
            cfa,
            data,
            black,
            white,
            wb,
            ..
        } => {
            let (w, h) = (*width as usize / 2, *height as usize / 2);
            let (ri, gi, gj, bi) = match cfa {
                Cfa::Rggb => (0, 1, 2, 3),
                Cfa::Bggr => (3, 1, 2, 0),
                Cfa::Grbg => (1, 0, 3, 2),
                Cfa::Gbrg => (2, 0, 3, 1),
            };
            let mut px = vec![[0f32; 3]; w * h];
            let mut sums = [0f64; 3];
            for y in 0..h {
                for x in 0..w {
                    let at = |k: usize| {
                        let (dx, dy) = (k % 2, k / 2);
                        let v = data[(2 * y + dy) * *width as usize + 2 * x + dx] as f32;
                        ((v - black) / (white - black)).max(0.0)
                    };
                    let p = [at(ri), (at(gi) + at(gj)) / 2.0, at(bi)];
                    for c in 0..3 {
                        sums[c] += p[c] as f64;
                    }
                    px[y * w + x] = p;
                }
            }
            let gains = wb.unwrap_or([
                (sums[1] / sums[0].max(1e-9)) as f32,
                1.0,
                (sums[1] / sums[2].max(1e-9)) as f32,
            ]);
            let mean_g = (sums[1] / (w * h) as f64) as f32;
            let exposure = 0.18 / mean_g.max(1e-6);
            let mut out = Vec::with_capacity(w * h * 3);
            for p in px {
                for c in 0..3 {
                    let v = (p[c] * gains[c] * exposure).clamp(0.0, 1.0).powf(1.0 / 2.2);
                    out.push((v * 255.0) as u8);
                }
            }
            (w as u32, h as u32, out)
        }
        Decoded::Rgb16 {
            width,
            height,
            data,
            ..
        } => (
            *width,
            *height,
            data.iter().map(|v| (v >> 8) as u8).collect(),
        ),
        Decoded::Display8 {
            width,
            height,
            data,
        } => (*width, *height, data.clone()),
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
            let (kr, kb) = match matrix {
                YcbcrMatrix::Bt601 => (0.299, 0.114),
                YcbcrMatrix::Bt709 => (0.2126, 0.0722),
                YcbcrMatrix::Bt2020 => (0.2627, 0.0593),
            };
            let max = ((1u32 << bits) - 1) as f32;
            let s = (1u32 << (bits - 8)) as f32;
            let (w, h) = (*width as usize, *height as usize);
            let (sx, sy) = (w / *chroma_width as usize, h / *chroma_height as usize);
            let mut out = Vec::with_capacity(w * h * 3);
            for yy in 0..h {
                for xx in 0..w {
                    let ci = (yy / sy) * *chroma_width as usize + xx / sx;
                    let (yv, u, v) = (
                        y[yy * w + xx] as f32,
                        cb[ci] as f32 - 128.0 * s,
                        cr[ci] as f32 - 128.0 * s,
                    );
                    let (yn, un, vn) = if *full_range {
                        (yv / max, u / max, v / max)
                    } else {
                        (
                            (yv - 16.0 * s) / (219.0 * s),
                            u / (224.0 * s),
                            v / (224.0 * s),
                        )
                    };
                    let r = yn + 2.0 * (1.0 - kr) * vn;
                    let b = yn + 2.0 * (1.0 - kb) * un;
                    let g = (yn - kr * r - kb * b) / (1.0 - kr - kb);
                    for c in [r, g, b] {
                        out.push((c.clamp(0.0, 1.0) * 255.0) as u8);
                    }
                }
            }
            (*width, *height, out)
        }
    }
}

/// Writes an RGB8 PNG with stored (uncompressed) deflate blocks. Good enough
/// for eyeballing thumbnails; no dependency.
pub fn write_png(path: &Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, typ: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut c = typ.to_vec();
        c.extend_from_slice(body);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    let mut raw = Vec::with_capacity((w as usize * 3 + 1) * h as usize);
    for row in rgb.chunks(w as usize * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut bsum) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        bsum = (bsum + a) % 65521;
    }
    z.extend_from_slice(&((bsum << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out)
}
