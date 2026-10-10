//! Test helpers: the ffmpeg oracle, Annex B access-unit splitting, plane
//! comparison.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use luna_h264::Picture;

/// The ffmpeg CLI, if installed (`LUNA_FFMPEG` overrides the name).
pub fn ffmpeg() -> Option<String> {
    let exe = std::env::var("LUNA_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    Command::new(&exe)
        .arg("-version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| exe)
}

pub fn run(cmd: &mut Command) {
    let out = cmd.output().expect("run ffmpeg");
    assert!(
        out.status.success(),
        "{cmd:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Reference planes decoded by ffmpeg from `input` as `pix_fmt`.
pub fn reference(ffmpeg: &str, input: &Path, pix_fmt: &str, out: &Path) -> Vec<u16> {
    run(Command::new(ffmpeg)
        .args(["-v", "error", "-y", "-i"])
        .arg(input)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", pix_fmt])
        .arg(out));
    let raw = std::fs::read(out).expect("read reference");
    if pix_fmt.ends_with("le") {
        raw.as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect()
    } else {
        raw.iter().map(|&b| b as u16).collect()
    }
}

/// Number of mismatching samples per plane (Y, Cb, Cr), plus the first
/// mismatch for diagnostics.
pub fn compare(pic: &Picture, want: &[u16]) -> ([usize; 3], Option<String>) {
    let (w, h) = (pic.width as usize, pic.height as usize);
    let (cw, ch) = (pic.chroma_width() as usize, pic.chroma_height() as usize);
    assert_eq!(want.len(), w * h + 2 * cw * ch, "reference size");
    let planes: [(&[u16], usize, usize, usize); 3] = [
        (&pic.y, 0, w, h),
        (&pic.cb, w * h, cw, ch),
        (&pic.cr, w * h + cw * ch, cw, ch),
    ];
    let mut bad = [0; 3];
    let mut first = None;
    for (k, (got, off, pw, ph)) in planes.into_iter().enumerate() {
        assert_eq!(got.len(), pw * ph, "plane {k} size");
        for i in 0..pw * ph {
            if got[i] != want[off + i] {
                bad[k] += 1;
                if first.is_none() {
                    first = Some(format!(
                        "plane {k} ({}, {}): got {} want {}",
                        i % pw,
                        i / pw,
                        got[i],
                        want[off + i]
                    ));
                }
            }
        }
    }
    (bad, first)
}

/// Splits an Annex B stream into access units (a new one starts at an AUD,
/// or at an SPS/PPS/slice with first_mb_in_slice = 0 after a slice).
pub fn access_units(data: &[u8]) -> Vec<Vec<u8>> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            let s = if i > 0 && data[i - 1] == 0 { i - 1 } else { i };
            starts.push((s, i + 3));
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut aus: Vec<Vec<u8>> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut have_slice = false;
    for (k, &(s, p)) in starts.iter().enumerate() {
        let e = starts.get(k + 1).map_or(data.len(), |n| n.0);
        let kind = data[p] & 0x1f;
        let first_mb_zero = data.get(p + 1).is_some_and(|b| b & 0x80 != 0);
        let new_au = have_slice
            && (kind == 9 || kind == 7 || kind == 8 || ((kind == 1 || kind == 5) && first_mb_zero));
        if new_au {
            aus.push(std::mem::take(&mut cur));
            have_slice = false;
        }
        if kind == 1 || kind == 5 {
            have_slice = true;
        }
        cur.extend_from_slice(&data[s..e]);
    }
    if !cur.is_empty() {
        aus.push(cur);
    }
    aus
}

pub fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("luna-h264");
    std::fs::create_dir_all(&d).expect("tmp dir");
    d.join(name)
}
