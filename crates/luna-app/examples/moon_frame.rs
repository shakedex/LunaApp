//! Writes engraved Moon frames as PPM, for comparison with the prototype:
//! `cargo run -p luna-app --example moon_frame -- <dir> [size]`.

use std::path::PathBuf;

use luna_app::moon::{MoonFrame, MoonRenderer};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let size: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(480);
    std::fs::create_dir_all(&dir).unwrap();
    let mut moon = MoonRenderer::new();
    for (lit, progress) in [(0.08, 0.0), (0.35, 0.3), (0.65, 0.62), (1.0, 1.0)] {
        let start = std::time::Instant::now();
        let img = moon.render(
            size,
            2,
            MoonFrame {
                lit,
                spin: 0.0,
                progress,
            },
        );
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut out = format!("P6\n{size} {size}\n255\n").into_bytes();
        for p in img.as_slice() {
            out.extend([p.r, p.g, p.b]);
        }
        let name = format!("rust-{:03}.ppm", (lit * 100.0) as u32);
        std::fs::write(dir.join(&name), out).unwrap();
        eprintln!("{name}: {ms:.1} ms");
    }
}
