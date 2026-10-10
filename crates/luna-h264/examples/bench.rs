//! Timing: decode a one-picture Annex B file repeatedly, single-threaded and
//! with slice threads.
//!
//! `cargo run --release -p luna-h264 --example bench -- frame.h264 [runs]`

use std::time::Instant;

use luna_h264::{DecoderConfig, decode_access_unit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: bench <frame.h264> [runs]");
        std::process::exit(2);
    };
    let runs: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
    let data = std::fs::read(path).expect("read");
    for (label, cfg) in [
        ("1 thread", DecoderConfig::annex_b().with_max_threads(1)),
        ("all cores", DecoderConfig::annex_b()),
    ] {
        let mut best = f64::MAX;
        let mut total = 0.0;
        for _ in 0..runs {
            let t = Instant::now();
            let pic = decode_access_unit(&data, &cfg).expect("decode");
            std::hint::black_box(&pic);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            best = best.min(ms);
            total += ms;
        }
        println!(
            "{label}: best {best:.1} ms, mean {:.1} ms over {runs} runs",
            total / runs as f64
        );
    }
}
