//! Camera files from the local corpus (`LUNA_CORPUS`; skipped when unset):
//! intra access units fetched through luna-formats' frame index, decoded
//! and compared bit-exactly with the ffmpeg oracle decoding the same bytes.

mod common;

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

use luna_formats::{ClipFiles, Container, FrameIndex};
use luna_h264::{DecoderConfig, FrameKind, access_unit_kind, decode_access_unit};

fn corpus(rel: &str) -> Option<PathBuf> {
    let root = std::env::var_os("LUNA_CORPUS")?;
    let p = Path::new(&root).join(rel);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("corpus file missing: {}", p.display());
        None
    }
}

fn read_range(path: &Path, offset: u64, len: u64) -> Vec<u8> {
    let mut f = std::fs::File::open(path).expect("open");
    f.seek(SeekFrom::Start(offset)).expect("seek");
    let mut b = vec![0u8; len as usize];
    f.read_exact(&mut b).expect("read");
    b
}

/// Annex B bytes of an access unit (AVCC samples get the avcC parameter
/// sets and start codes) for the oracle.
fn annex_b(au: &[u8], avcc: Option<&[u8]>) -> Vec<u8> {
    let Some(rec) = avcc else {
        return au.to_vec();
    };
    let mut out = Vec::new();
    let n_sps = (rec[5] & 0x1f) as usize;
    let mut i = 6;
    let mut sets = Vec::new();
    for _ in 0..n_sps {
        let l = u16::from_be_bytes([rec[i], rec[i + 1]]) as usize;
        sets.push(&rec[i + 2..i + 2 + l]);
        i += 2 + l;
    }
    let n_pps = rec[i] as usize;
    i += 1;
    for _ in 0..n_pps {
        let l = u16::from_be_bytes([rec[i], rec[i + 1]]) as usize;
        sets.push(&rec[i + 2..i + 2 + l]);
        i += 2 + l;
    }
    let len_size = (rec[4] & 3) as usize + 1;
    for s in sets {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(s);
    }
    let mut j = 0;
    while j + len_size <= au.len() {
        let l = au[j..j + len_size]
            .iter()
            .fold(0usize, |a, &b| (a << 8) | b as usize);
        j += len_size;
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&au[j..j + l]);
        j += l;
    }
    out
}

fn check_clip(rel: &str, container: Container, max_intra: usize) {
    let Some(path) = corpus(rel) else {
        return;
    };
    let clip = ClipFiles {
        container,
        main: path.clone(),
        segments: vec![],
        sidecars: vec![],
    };
    let probe = luna_formats::probe(&clip).expect("probe");
    let FrameIndex::Ranges(ranges) = &probe.frames else {
        panic!("{rel}: no frame index");
    };
    let cfg = match &probe.codec_config {
        Some(rec) => DecoderConfig::from_avcc(rec).expect("avcC"),
        None => DecoderConfig::annex_b(),
    };
    let ffmpeg = common::ffmpeg();
    // The intra access units at or before the first, a later, the middle
    // and the last frame.
    let n = ranges.len();
    let picks = [0, 14.min(n - 1), n / 2, n - 1, 1, 2, 3];
    let mut done = 0;
    let mut seen = std::collections::HashSet::new();
    for &i in &picks {
        // Walk back to the nearest intra access unit at or before i.
        let mut k = i;
        let au = loop {
            let r = ranges[k];
            let au = read_range(&path, r.offset, r.len);
            if access_unit_kind(&au, &cfg) != FrameKind::Other {
                break Some(au);
            }
            if k == 0 {
                break None;
            }
            k -= 1;
        };
        let Some(au) = au else { continue };
        if !seen.insert(k) {
            continue;
        }
        let t = Instant::now();
        let pic = decode_access_unit(&au, &cfg).unwrap_or_else(|e| panic!("{rel} frame {k}: {e}"));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "{rel} frame {k} ({:?}): {}x{} {}-bit chroma {} colour {:?} in {ms:.0} ms",
            access_unit_kind(&au, &cfg),
            pic.width,
            pic.height,
            pic.bit_depth,
            pic.chroma_format,
            pic.colour
        );
        if let Some(ff) = &ffmpeg {
            let tag = format!("corpus_{}_{k}", rel.replace(['/', '\\', '.'], "_"));
            let src = common::tmp(&format!("{tag}.h264"));
            std::fs::write(&src, annex_b(&au, probe.codec_config.as_deref())).expect("write");
            let fmt = match (pic.chroma_format, pic.bit_depth) {
                (2, 8) => "yuv422p",
                (2, _) => "yuv422p10le",
                (_, 8) => "yuv420p",
                _ => "yuv420p10le",
            };
            let want = common::reference(ff, &src, fmt, &common::tmp(&format!("{tag}.yuv")));
            let (bad, first) = common::compare(&pic, &want);
            assert_eq!(bad, [0; 3], "{rel} frame {k}: first mismatch {first:?}");
            eprintln!("  bit-exact vs ffmpeg");
        }
        if done == 0 {
            damage(&au, &cfg);
        }
        done += 1;
        if done >= max_intra {
            break;
        }
    }
    assert!(done > 0, "{rel}: no intra frame decoded");
}

#[test]
fn sony_xavc_intra_4k() {
    check_clip(
        "S_cam/S003/Filmplusgear-FX6-testfile-1-254_0110.MXF",
        Container::Mxf,
        4,
    );
}

#[test]
fn canon_xf_avc_long_gop() {
    check_clip(
        "S_cam/s004/s_0004C062A250107_2240217S_CANON.MXF",
        Container::Mxf,
        4,
    );
}

#[test]
fn lumix_mov_long_gop() {
    check_clip("S_cam/S002/lumix_s5.mov", Container::QuickTime, 3);
}

/// Bit flips and truncations of a real access unit never panic.
fn damage(au: &[u8], cfg: &DecoderConfig) {
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for i in 0..24 {
        let mut d = au.to_vec();
        if i % 3 == 0 {
            d.truncate(next() as usize % d.len());
        } else {
            for _ in 0..1 + next() % 8 {
                let k = next() as usize % d.len();
                d[k] ^= 1 << (next() % 8);
            }
        }
        let _ = decode_access_unit(&d, cfg);
    }
}
