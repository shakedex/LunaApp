//! The platform decoder (Media Foundation on Windows, AVFoundation on macOS)
//! on synthetic clips made with ffmpeg at test time (ffmpeg is only a fixture
//! generator here). Skipped without ffmpeg.
#![cfg(any(windows, target_os = "macos"))]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use luna_decode::{Decoded, YcbcrMatrix, decode_frames};
use luna_formats::{Codec, Container, FrameIndex, Rational};

fn fixture(name: &str, args: &[&str]) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("luna-decode-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let out = dir.join(name);
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .arg(&out)
        .status();
    match status {
        Ok(s) if s.success() => Some(out),
        _ => {
            eprintln!("skipping: ffmpeg not available");
            None
        }
    }
}

#[test]
fn h264_420_decodes_the_frame_asked_for() {
    // 50 frames at 25 fps, B-frames on; each frame's grey level encodes its
    // number, so the mean luma shows which frame came back.
    let Some(path) = fixture(
        "h264_420.mp4",
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=25:d=2,geq=lum='N*5':cb=128:cr=128",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "12",
            "-bf",
            "2",
            "-colorspace",
            "bt709",
            "-color_range",
            "tv",
        ],
    ) else {
        return;
    };
    // geq writes Y = N*5, which x264 clips to the 16..235 video range
    // (checked against ffmpeg's own decode of the same file: 16, 124, 235).
    let expect = |n: f64| (n * 5.0).clamp(16.0, 235.0);
    decodes_the_frame_asked_for(&path, Codec::H264, (160, 90, 8), expect);
}

#[cfg(target_os = "macos")]
#[test]
fn hevc_422_10bit_decodes_the_frame_asked_for() {
    // As above, in 10-bit 4:2:2 with Y = 64 + N*16, inside the video range.
    let Some(path) = fixture(
        "hevc_422_10.mov",
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=25:d=2,format=yuv422p10le,geq=lum='64+N*16':cb=512:cr=512",
            "-c:v",
            "libx265",
            "-x265-params",
            "log-level=error:keyint=12:bframes=2",
            "-pix_fmt",
            "yuv422p10le",
            "-tag:v",
            "hvc1",
            "-colorspace",
            "bt709",
            "-color_range",
            "tv",
        ],
    ) else {
        return;
    };
    decodes_the_frame_asked_for(&path, Codec::Hevc, (160, 180, 10), |n| 64.0 + n * 16.0);
}

/// Frames 0, 25, 49 and 500 (clamped to 49) of a 50-frame 25 fps 320×180
/// clip whose frames are flat grey levels that encode their number, so the
/// mean luma shows which frame came back.
fn decodes_the_frame_asked_for(
    path: &Path,
    codec: Codec,
    (cw, ch, depth): (u32, u32, u8),
    expect: impl Fn(f64) -> f64,
) {
    let clip = common::files(Container::QuickTime, path);
    let mut p = common::probe(codec, FrameIndex::DecoderOwned, Default::default());
    p.meta.frame_rate = Some(Rational { num: 25, den: 1 });
    p.meta.frame_count = Some(50);
    let out = decode_frames(&clip, &p, &[0, 25, 49, 500], 160);
    assert_eq!(out.len(), 4);
    let mut lumas = Vec::new();
    for r in out {
        let f = r.unwrap();
        let Decoded::Ycbcr {
            width,
            height,
            chroma_width,
            chroma_height,
            bits,
            ref y,
            matrix,
            full_range,
            ..
        } = f.image
        else {
            panic!("expected ycbcr")
        };
        assert_eq!(
            (width, height, chroma_width, chroma_height),
            (320, 180, cw, ch)
        );
        assert_eq!(bits, depth);
        assert_eq!(matrix, YcbcrMatrix::Bt709);
        assert!(!full_range);
        lumas.push(y.iter().map(|&v| v as f64).sum::<f64>() / y.len() as f64);
    }
    for (got, n) in lumas.iter().zip([0.0, 25.0, 49.0, 49.0]) {
        assert!(
            (got - expect(n)).abs() < 3.0,
            "frame {n}: luma {got}, expected {}",
            expect(n)
        );
    }
}
