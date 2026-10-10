//! Test-only helpers: reading reference planes, comparing pictures, and
//! pulling the first coded frame out of a MOV or MXF file. Deliberately tiny:
//! this is not a container parser, just enough to feed the decoder.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use luna_prores::Picture;

/// Per-plane comparison result.
#[derive(Debug, Default, Clone, Copy)]
pub struct Diff {
    pub samples: usize,
    pub exact: usize,
    pub max_err: u32,
}

impl Diff {
    pub fn exact_ratio(&self) -> f64 {
        if self.samples == 0 {
            1.0
        } else {
            self.exact as f64 / self.samples as f64
        }
    }

    fn add(&mut self, other: Diff) {
        self.samples += other.samples;
        self.exact += other.exact;
        self.max_err = self.max_err.max(other.max_err);
    }
}

fn diff_plane(got: &[u16], want: &[u16]) -> Diff {
    assert_eq!(got.len(), want.len(), "plane size mismatch");
    let mut d = Diff {
        samples: got.len(),
        ..Diff::default()
    };
    for (&g, &w) in got.iter().zip(want) {
        let e = (g as i32 - w as i32).unsigned_abs();
        if e == 0 {
            d.exact += 1;
        }
        d.max_err = d.max_err.max(e);
    }
    d
}

/// Splits a little-endian 16-bit planar Y′CbCr file into its three planes.
pub fn reference_planes(raw: &[u8], width: usize, height: usize, chroma_w: usize) -> [Vec<u16>; 3] {
    let samples: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let luma = width * height;
    let chroma = chroma_w * height;
    assert!(samples.len() >= luma + 2 * chroma, "reference too short");
    [
        samples[..luma].to_vec(),
        samples[luma..luma + chroma].to_vec(),
        samples[luma + chroma..luma + 2 * chroma].to_vec(),
    ]
}

/// Compares a decoded picture against reference planes; returns per-plane diffs
/// and the total.
pub fn compare(pic: &Picture, reference: &[Vec<u16>; 3]) -> ([Diff; 3], Diff) {
    let planes = [
        diff_plane(&pic.y, &reference[0]),
        diff_plane(&pic.cb, &reference[1]),
        diff_plane(&pic.cr, &reference[2]),
    ];
    let mut total = Diff::default();
    for p in planes {
        total.add(p);
    }
    (planes, total)
}

/// Reads one big-endian u32 at `at`.
fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

fn be64(b: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(b[at..at + 8].try_into().unwrap())
}

/// Iterates the child atoms of `buf`, returning (type, payload range).
fn atoms(buf: &[u8], base: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= buf.len() {
        let mut size = be32(buf, at) as usize;
        let kind: [u8; 4] = buf[at + 4..at + 8].try_into().unwrap();
        let mut header = 8;
        if size == 1 {
            size = be64(buf, at + 8) as usize;
            header = 16;
        } else if size == 0 {
            size = buf.len() - at;
        }
        if size < header || at + size > buf.len() {
            break;
        }
        out.push((kind, base + at + header, base + at + size));
        at += size;
    }
    out
}

fn child(file: &[u8], range: (usize, usize), kind: &[u8; 4]) -> Option<(usize, usize)> {
    atoms(&file[range.0..range.1], range.0)
        .into_iter()
        .find(|(k, _, _)| k == kind)
        .map(|(_, s, e)| (s, e))
}

/// Returns the first video sample of a QuickTime file (the first track whose
/// handler is `vide`).
pub fn first_mov_frame(path: &Path) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).expect("open mov");
    let len = f.metadata().unwrap().len();
    // Locate the moov atom by walking top-level headers without reading mdat.
    let mut at = 0u64;
    let mut moov = None;
    while at + 8 <= len {
        let mut h = [0u8; 16];
        f.seek(SeekFrom::Start(at)).unwrap();
        f.read_exact(&mut h[..8]).unwrap();
        let mut size = u32::from_be_bytes(h[..4].try_into().unwrap()) as u64;
        let mut header = 8;
        if size == 1 {
            f.read_exact(&mut h[8..16]).unwrap();
            size = u64::from_be_bytes(h[8..16].try_into().unwrap());
            header = 16;
        } else if size == 0 {
            size = len - at;
        }
        if &h[4..8] == b"moov" {
            moov = Some((at + header, at + size));
            break;
        }
        if size < header {
            break;
        }
        at += size;
    }
    let (ms, me) = moov.expect("moov atom");
    let mut buf = vec![0u8; (me - ms) as usize];
    f.seek(SeekFrom::Start(ms)).unwrap();
    f.read_exact(&mut buf).unwrap();

    for (kind, ts, te) in atoms(&buf, 0) {
        if &kind != b"trak" {
            continue;
        }
        let Some(mdia) = child(&buf, (ts, te), b"mdia") else {
            continue;
        };
        let Some(hdlr) = child(&buf, mdia, b"hdlr") else {
            continue;
        };
        if &buf[hdlr.0 + 8..hdlr.0 + 12] != b"vide" {
            continue;
        }
        let minf = child(&buf, mdia, b"minf").expect("minf");
        let stbl = child(&buf, minf, b"stbl").expect("stbl");
        let stsz = child(&buf, stbl, b"stsz").expect("stsz");
        let fixed = be32(&buf, stsz.0 + 4);
        let size = if fixed != 0 {
            fixed
        } else {
            be32(&buf, stsz.0 + 12)
        } as usize;
        // The first sample is the first one in the first chunk, so stsc is not
        // needed beyond knowing chunk one starts the track.
        let offset = if let Some(stco) = child(&buf, stbl, b"stco") {
            be32(&buf, stco.0 + 8) as u64
        } else {
            let co64 = child(&buf, stbl, b"co64").expect("stco or co64");
            be64(&buf, co64.0 + 8)
        };
        let mut frame = vec![0u8; size];
        f.seek(SeekFrom::Start(offset)).unwrap();
        f.read_exact(&mut frame).unwrap();
        return frame;
    }
    panic!("no video track in {}", path.display());
}

/// Returns the first ProRes frame in an MXF file by finding the first `icpf`
/// marker (the frame size is the four bytes before it).
pub fn first_mxf_frame(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).expect("open mxf");
    let mut head = vec![0u8; 64 << 20];
    let n = f.read(&mut head).unwrap();
    head.truncate(n);
    let at = head
        .windows(4)
        .position(|w| w == b"icpf")
        .expect("icpf marker");
    let start = at - 4;
    let size = be32(&head, start) as usize;
    let mut frame = head[start..(start + size).min(head.len())].to_vec();
    if frame.len() < size {
        let mut rest = vec![0u8; size - frame.len()];
        f.read_exact(&mut rest).unwrap();
        frame.extend_from_slice(&rest);
    }
    frame
}

/// The corpus root, when `LUNA_CORPUS` is set.
pub fn corpus() -> Option<PathBuf> {
    std::env::var_os("LUNA_CORPUS").map(PathBuf::from)
}

/// The ffmpeg binary used as a black-box oracle (`LUNA_FFMPEG` or `ffmpeg` on
/// PATH), when it runs.
pub fn ffmpeg() -> Option<PathBuf> {
    let bin = std::env::var_os("LUNA_FFMPEG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));
    Command::new(&bin)
        .arg("-version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| bin)
}

/// Decodes the first frame of `input` with the oracle into 16-bit planar raw
/// samples of `pix_fmt`, cached in the temp dir under `tag`.
pub fn oracle_first_frame(ffmpeg: &Path, input: &Path, pix_fmt: &str, tag: &str) -> Vec<u8> {
    let dir = std::env::temp_dir().join("luna-prores-oracle");
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join(format!("{tag}.{pix_fmt}.yuv"));
    if !out.exists() {
        let status = Command::new(ffmpeg)
            .args(["-v", "error", "-y", "-i"])
            .arg(input)
            .args(["-frames:v", "1", "-pix_fmt", pix_fmt, "-f", "rawvideo"])
            .arg(&out)
            .status()
            .expect("run ffmpeg");
        assert!(status.success(), "ffmpeg failed on {}", input.display());
    }
    std::fs::read(&out).unwrap()
}

/// Debug aid: signed error histogram and a few (got, want, index) samples.
pub fn mismatch_report(got: &[u16], want: &[u16]) -> String {
    let mut hist = std::collections::BTreeMap::new();
    let mut samples = Vec::new();
    for (i, (&g, &w)) in got.iter().zip(want).enumerate() {
        let d = g as i32 - w as i32;
        if d != 0 {
            *hist.entry(d).or_insert(0usize) += 1;
            if samples.len() < 12 {
                samples.push((g, w, i));
            }
        }
    }
    format!("{hist:?} {samples:?}")
}

/// Mean absolute difference between a box-downscaled `full` plane and `small`.
pub fn box_error(full: &[u16], w: usize, h: usize, small: &[u16], ds: usize) -> f64 {
    let sw = w.div_ceil(ds);
    let sh = h.div_ceil(ds);
    let mut total = 0f64;
    for sy in 0..sh {
        for sx in 0..sw {
            let (mut sum, mut n) = (0u64, 0u64);
            for y in sy * ds..((sy + 1) * ds).min(h) {
                for x in sx * ds..((sx + 1) * ds).min(w) {
                    sum += full[y * w + x] as u64;
                    n += 1;
                }
            }
            let mean = sum as f64 / n as f64;
            total += (mean - small[sy * sw + sx] as f64).abs();
        }
    }
    total / (sw * sh) as f64
}
