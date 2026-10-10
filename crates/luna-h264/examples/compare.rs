//! Development tool: decode a one-picture Annex B file and compare it with
//! raw planar reference output (`ffmpeg -f rawvideo`).
//!
//! `cargo run --release -p luna-h264 --example compare -- in.h264 ref.yuv`

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: compare <in.h264> [ref.yuv]");
        std::process::exit(2);
    }
    let data = std::fs::read(&args[1]).expect("read input");
    let cfg = luna_h264::DecoderConfig::annex_b();
    let t = Instant::now();
    let pic = match luna_h264::decode_access_unit(&data, &cfg) {
        Ok(p) => p,
        Err(e) => {
            println!("decode failed: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "{}x{} chroma {} {}-bit in {:.1} ms",
        pic.width,
        pic.height,
        pic.chroma_format,
        pic.bit_depth,
        t.elapsed().as_secs_f64() * 1e3
    );
    let Some(refp) = args.get(2) else { return };
    let raw = std::fs::read(refp).expect("read reference");
    let wide = pic.bit_depth > 8;
    let samples: Vec<u16> = if wide {
        raw.as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect()
    } else {
        raw.iter().map(|&b| b as u16).collect()
    };
    let (w, h) = (pic.width as usize, pic.height as usize);
    let (cw, ch) = (pic.chroma_width() as usize, pic.chroma_height() as usize);
    let planes = [
        ("Y", &pic.y, 0usize, w, h, 16usize, 16usize),
        (
            "Cb",
            &pic.cb,
            w * h,
            cw,
            ch,
            8,
            if pic.chroma_format == 1 { 8 } else { 16 },
        ),
        (
            "Cr",
            &pic.cr,
            w * h + cw * ch,
            cw,
            ch,
            8,
            if pic.chroma_format == 1 { 8 } else { 16 },
        ),
    ];
    let mut all_ok = true;
    for (name, got, off, pw, ph, mbw, mbh) in planes {
        let want = &samples[off..off + pw * ph];
        let mut bad = 0usize;
        let mut first = None;
        for y in 0..ph {
            for x in 0..pw {
                if got[y * pw + x] != want[y * pw + x] {
                    bad += 1;
                    if first.is_none() {
                        first = Some((x, y));
                    }
                }
            }
        }
        if let Some((x, y)) = first {
            all_ok = false;
            let mb = (y / mbh) * pw.div_ceil(mbw) + x / mbw;
            println!(
                "{name}: {bad} mismatches; first at ({x},{y}) MB {mb} got {} want {}",
                got[y * pw + x],
                want[y * pw + x]
            );
        } else {
            println!("{name}: exact");
        }
    }
    if !all_ok {
        std::process::exit(1);
    }
}
