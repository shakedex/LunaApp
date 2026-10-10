//! The platform decoder on a synthetic H.264 4:2:0 clip made with ffmpeg at
//! test time (ffmpeg is only a fixture generator here). Skipped without ffmpeg.

mod common;

use std::path::PathBuf;
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

#[cfg(windows)]
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
    let clip = common::files(Container::QuickTime, &path);
    let mut p = common::probe(Codec::H264, FrameIndex::DecoderOwned, Default::default());
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
            (320, 180, 160, 90)
        );
        assert_eq!(bits, 8);
        assert_eq!(matrix, YcbcrMatrix::Bt709);
        assert!(!full_range);
        lumas.push(y.iter().map(|&v| v as f64).sum::<f64>() / y.len() as f64);
    }
    // geq writes Y = N*5, which x264 clips to the 16..235 video range
    // (checked against ffmpeg's own decode of the same file: 16, 124, 235).
    let expect = |n: f64| (n * 5.0).clamp(16.0, 235.0);
    for (got, n) in lumas.iter().zip([0.0, 25.0, 49.0, 49.0]) {
        assert!(
            (got - expect(n)).abs() < 3.0,
            "frame {n}: luma {got}, expected {}",
            expect(n)
        );
    }
}
