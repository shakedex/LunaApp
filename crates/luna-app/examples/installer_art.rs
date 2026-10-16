//! Renders the Windows installer's wizard panel from Luna's own sky and
//! Moon, at every size Inno Setup picks from by DPI:
//! `cargo run -p luna-app --example installer_art -- <dir>`.
//! Writes `wizard-<w>x<h>.ppm`; `packaging/windows/art/make.py` turns them
//! into the PNGs the installer uses.

use std::path::PathBuf;

use luna_app::backdrop::{BackdropRenderer, Sky, place_moon};
use luna_app::moon::{MoonFrame, MoonRenderer};

/// Inno's wizard image area at 100% DPI.
const BASE: (f32, f32) = (164.0, 314.0);
/// 100% to 250%, the scales Inno's own images come in.
const SCALES: [f32; 7] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5];

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut moon = MoonRenderer::new();
    let mut sky = BackdropRenderer::new();
    for scale in SCALES {
        let (w, h) = (
            (BASE.0 * scale).round() as u32,
            (BASE.1 * scale).round() as u32,
        );
        // The Moon as Luna shows it while waiting for a folder: a thin
        // crescent in its dial, a little above the middle.
        let moon_box = 132.0 * scale;
        let centre = [w as f32 / 2.0, h as f32 * 0.42];
        let disc = moon.render(
            moon_box.round() as u32,
            2,
            MoonFrame {
                lit: 0.08,
                spin: 0.0,
                progress: 0.0,
            },
        );
        let mut pixels = sky.render(
            Sky {
                width: w,
                height: h,
                centre,
                dial: moon_box * 0.42,
                scale,
            },
            0.0,
        );
        place_moon(&mut pixels, &disc, centre);
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for p in pixels.as_slice() {
            out.extend([p.r, p.g, p.b]);
        }
        let name = format!("wizard-{w}x{h}.ppm");
        std::fs::write(dir.join(&name), out).unwrap();
        eprintln!("wrote {name}");
    }
}
