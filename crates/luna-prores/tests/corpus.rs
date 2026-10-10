//! Real camera ProRes against the ffmpeg oracle. Runs only when `LUNA_CORPUS`
//! points at the test corpus and an ffmpeg binary is available; otherwise each
//! test returns early.
//!
//! The oracle's 12-bit inverse DCT is a fixed-point approximation that can
//! sit up to ~0.8 of a code away from the mathematical transform RDD 36
//! defines; this decoder evaluates the transform accurately, so a few percent
//! of samples differ by exactly one code. Anything larger would mean a
//! bitstream-level mistake, so the bound asserted is max error ≤ 1.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use common::{
    box_error, compare, first_mov_frame, first_mxf_frame, oracle_first_frame, reference_planes,
};

/// Minimum share of samples identical to the oracle at 12 bits.
const MIN_EXACT_12BIT: f64 = 0.80;

fn setup() -> Option<(PathBuf, PathBuf)> {
    let Some(root) = common::corpus() else {
        eprintln!("LUNA_CORPUS not set; skipping");
        return None;
    };
    let Some(ffmpeg) = common::ffmpeg() else {
        eprintln!("ffmpeg not available; skipping");
        return None;
    };
    Some((root, ffmpeg))
}

fn check(rel: &str, tag: &str, width: u32, height: u32) {
    let Some((root, ffmpeg)) = setup() else {
        return;
    };
    let path = root.join(rel);
    let frame = if rel.ends_with(".mxf") {
        first_mxf_frame(&path)
    } else {
        first_mov_frame(&path)
    };
    let start = Instant::now();
    let pic = luna_prores::decode(&frame, 1).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let elapsed = start.elapsed();
    assert_eq!((pic.width, pic.height), (width, height), "{rel}");
    assert_eq!(pic.chroma_format, 3);
    assert_eq!(pic.bits, 12);

    // The oracle's native format; alpha (if any) follows the three planes.
    let raw = oracle_first_frame(&ffmpeg, &path, oracle_format(&path), tag);
    let reference = reference_planes(&raw, width as usize, height as usize, width as usize);
    let (planes, total) = compare(&pic, &reference);
    eprintln!(
        "{rel}: decode {elapsed:?}, exact {:.4}% max {} ({planes:?})",
        total.exact_ratio() * 100.0,
        total.max_err
    );
    assert!(total.max_err <= 1, "{rel}: max error {}", total.max_err);
    assert!(
        total.exact_ratio() >= MIN_EXACT_12BIT,
        "{rel}: only {:.4}% exact",
        total.exact_ratio() * 100.0
    );

    // Reduced decodes agree with a box-filtered full decode.
    for ds in [2u8, 4, 8] {
        let small = luna_prores::decode(&frame, ds).unwrap();
        assert_eq!(small.width, width.div_ceil(ds as u32));
        assert_eq!(small.height, height.div_ceil(ds as u32));
        let err = box_error(
            &pic.y,
            width as usize,
            height as usize,
            &small.y,
            ds as usize,
        );
        assert!(err < 8.0, "{rel}: downscale {ds} mean error {err}");
    }
}

/// The oracle pixel format that keeps samples untouched for this file.
fn oracle_format(path: &Path) -> &'static str {
    let out = std::process::Command::new(common::ffmpeg().unwrap().with_file_name("ffprobe"))
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=pix_fmt",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if out.starts_with("yuva") {
        "yuva444p12le"
    } else {
        "yuv444p12le"
    }
}

#[test]
fn c004_4444_with_alpha_1080p() {
    check("C_CAM/C004/Slog3 -proress 444.mov", "c004", 1920, 1080);
}

#[test]
fn m001c001_4444xq_1080p() {
    check(
        "M_CAM/M001/M001C001_161207_R00H.mov",
        "m001c001",
        1920,
        1080,
    );
}

#[test]
fn m001c005_4444xq_uhd() {
    check(
        "M_CAM/M001/M001C005_161207_R00H.mov",
        "m001c005",
        3840,
        2160,
    );
}

#[test]
fn m001c007_4444xq_2048x858() {
    check("M_CAM/M001/M001C007_161207_R00H.mov", "m001c007", 2048, 858);
}

#[test]
fn ronin_4444_4096x1712() {
    check(
        "RONIN-4D/A001C0004_210816_0000_4K_ProRes4444_25FPS.mov",
        "ronin",
        4096,
        1712,
    );
}

#[test]
fn a_cam_mxf_4444_uhd() {
    check(
        "A_CAM/A001/A0001C027_251020_135413_p1I7H.mxf",
        "acam",
        3840,
        2160,
    );
}

#[test]
fn f_cam_mxf_4444_uhd() {
    check("F_CAM/F003/F003C004_190925_MN99.mxf", "fcam", 3840, 2160);
}
