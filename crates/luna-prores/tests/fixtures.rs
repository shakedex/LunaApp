//! Small generated ProRes frames (ffmpeg `prores_ks`, 200×120 or 200×122) with
//! the oracle's decoded planes next to them. These cover the 4:2:2 profiles,
//! interlacing, custom quantisation matrices, every slice width and alpha,
//! none of which the real-camera corpus has.

mod common;

use common::{compare, reference_planes};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Decodes a fixture and compares it with the oracle's planes.
fn check(name: &str, width: u32, height: u32, chroma_format: u8) {
    let frame = fixture(&format!("{name}.prores"));
    let pic = luna_prores::decode(&frame, 1).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!((pic.width, pic.height), (width, height), "{name}: size");
    assert_eq!(pic.chroma_format, chroma_format, "{name}: chroma format");
    let bits = if chroma_format == 2 { 10 } else { 12 };
    assert_eq!(pic.bits, bits, "{name}: bits");
    let chroma_w = if chroma_format == 2 {
        width.div_ceil(2)
    } else {
        width
    } as usize;
    let reference = reference_planes(
        &fixture(&format!("{name}.yuv")),
        width as usize,
        height as usize,
        chroma_w,
    );
    let (planes, total) = compare(&pic, &reference);
    eprintln!("{name}: {planes:?}");
    // Every coefficient must be right (a wrong one shows as a multi-code
    // error); the remaining one-code differences are the oracle's fixed-point
    // inverse DCT, see tests/corpus.rs.
    assert!(total.max_err <= 1, "{name}: {planes:?}");
    let min_exact = if bits == 10 { 0.97 } else { 0.85 };
    assert!(
        total.exact_ratio() >= min_exact,
        "{name}: {:.3}% exact {planes:?}",
        total.exact_ratio() * 100.0
    );
}

#[test]
fn proxy_422() {
    check("proxy", 200, 120, 2);
}

#[test]
fn lt_422() {
    check("lt", 200, 120, 2);
}

#[test]
fn standard_422_two_mb_slices() {
    check("std", 200, 120, 2);
}

#[test]
fn hq_422_four_mb_slices() {
    check("hq", 200, 120, 2);
}

#[test]
fn hq_422_flat_matrix_one_mb_slices() {
    check("hq_flat", 200, 120, 2);
}

#[test]
fn interlaced_top_field_first() {
    check("interlaced_tff", 200, 120, 2);
}

#[test]
fn interlaced_bottom_field_first_odd_height() {
    check("interlaced_bff", 200, 122, 2);
}

#[test]
fn p4444() {
    check("p4444", 200, 120, 3);
}

#[test]
fn p4444_with_alpha() {
    check("p4444_alpha", 200, 120, 3);
}

#[test]
fn p4444xq_eight_mb_slices() {
    check("p4444xq", 200, 120, 3);
}

#[test]
fn colour_description_is_reported() {
    let pic = luna_prores::decode(&fixture("hq.prores"), 1).unwrap();
    assert_eq!(
        (
            pic.color_primaries,
            pic.transfer_characteristic,
            pic.matrix_coefficients
        ),
        (2, 2, 2)
    );
}
