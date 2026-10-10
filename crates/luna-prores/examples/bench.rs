//! Times decoding of one raw coded frame (an `icpf` frame as stored in a MOV
//! sample), e.g. made with
//! `ffmpeg -i in.mov -frames:v 1 -c:v copy -f rawvideo frame.prores`.
//!
//! `cargo run --release -p luna-prores --example bench -- frame.prores`

use std::time::{Duration, Instant};

fn best(runs: usize, mut f: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: bench <frame.prores>");
    let frame = std::fs::read(&path).expect("read frame");
    let pic = luna_prores::decode(&frame, 1).expect("decode");
    println!(
        "{path}: {}x{} chroma {}",
        pic.width, pic.height, pic.chroma_format
    );
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let samples = pic.y.len() + pic.cb.len() + pic.cr.len();
    let touch = best(10, || {
        let mut v = vec![0u16; samples];
        for (i, s) in v.iter_mut().enumerate() {
            *s = i as u16;
        }
        std::hint::black_box(&v);
    });
    println!("allocate + write output-sized planes: {touch:?}");
    for (ds, t) in [
        (1u8, 1usize),
        (1, threads),
        (2, 1),
        (4, 1),
        (8, 1),
        (8, threads),
    ] {
        let d = best(10, || {
            luna_prores::decode_with_threads(&frame, ds, t).unwrap();
        });
        println!("downscale {ds} threads {t:2}: {d:?}");
    }
}
