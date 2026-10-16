//! Decodes one frame and writes a quick-and-dirty PNG preview, for eyeballing.
//!
//! ```text
//! cargo run -p luna-decode --release --example dump -- <kind> <file> [frame] [out.png] [min_width]
//! ```
//! `kind`: xocn, crm, braw, r3d, nev, prores, h264, hevc. Vendor containers also print their metadata.

#[path = "../tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use luna_decode::{Decoded, decode_frames, vendor_probe};
use luna_formats::{Codec, Container, FrameIndex, ProResProfile};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: dump <kind> <file> [frame] [out.png] [min_width]");
        std::process::exit(2);
    }
    let (kind, path) = (args[0].as_str(), Path::new(&args[1]));
    let frame: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let out = args
        .get(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dump.png"));
    let min_width: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(640);

    let (container, probe) = match kind {
        "xocn" => (
            Container::Mxf,
            common::probe(Codec::XOcn, FrameIndex::DecoderOwned, Default::default()),
        ),
        "crm" | "braw" | "r3d" | "nev" => {
            let container = match kind {
                "crm" => Container::Crm,
                "braw" => Container::Braw,
                "nev" => Container::Nev,
                _ => Container::R3d,
            };
            let t = Instant::now();
            let p = vendor_probe(&common::files(container, path))
                .expect("vendor container")
                .unwrap_or_else(|e| panic!("probe failed: {e}"));
            println!("probe in {:?}:\n{:#?}", t.elapsed(), p);
            (container, p)
        }
        "prores" => {
            let container = if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mxf"))
            {
                Container::Mxf
            } else {
                Container::QuickTime
            };
            let ranges = if container == Container::Mxf {
                common::mxf_picture_ranges(path)
            } else {
                common::mov_video_ranges(path)
            }
            .expect("index");
            println!("{} frames", ranges.len());
            (
                container,
                common::probe(
                    Codec::ProRes(ProResProfile::Hq),
                    FrameIndex::Ranges(ranges),
                    Default::default(),
                ),
            )
        }
        "h264" | "hevc" => (
            Container::QuickTime,
            common::probe(
                if kind == "h264" {
                    Codec::H264
                } else {
                    Codec::Hevc
                },
                FrameIndex::DecoderOwned,
                Default::default(),
            ),
        ),
        other => panic!("unknown kind {other}"),
    };

    let files = common::files(container, path);
    let t = Instant::now();
    let r = decode_frames(&files, &probe, &[frame], min_width)
        .pop()
        .unwrap();
    let took = t.elapsed();
    match r {
        Ok(f) => {
            let what = match &f.image {
                Decoded::Mosaic {
                    width,
                    height,
                    cfa,
                    black,
                    white,
                    wb,
                    ..
                } => {
                    format!("mosaic {width}x{height} {cfa:?} black {black} white {white} wb {wb:?}")
                }
                Decoded::Rgb16 {
                    width,
                    height,
                    transfer,
                    gamut,
                    ..
                } => format!("rgb16 {width}x{height} {transfer:?}/{gamut:?}"),
                Decoded::Display8 { width, height, .. } => format!("display8 {width}x{height}"),
                Decoded::Ycbcr {
                    width,
                    height,
                    chroma_width,
                    chroma_height,
                    bits,
                    matrix,
                    full_range,
                    ..
                } => format!(
                    "ycbcr {width}x{height} chroma {chroma_width}x{chroma_height} {bits}-bit {matrix:?} full={full_range}"
                ),
            };
            println!("frame {frame}: {what} in {took:?}, hint {:?}", f.hint);
            let (w, h, rgb) = common::preview_rgb(&f.image);
            common::write_png(&out, w, h, &rgb).expect("write png");
            println!("wrote {}", out.display());
        }
        Err(e) => println!("frame {frame}: error in {took:?}: {e:?}"),
    }
}
