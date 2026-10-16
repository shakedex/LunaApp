//! Luna's moon: a line engraving of the real Moon, drawn live.
//!
//! The surface is NASA's lunar albedo (CGI Moon Kit, NASA's Scientific
//! Visualization Studio), baked by `tools/moon/bake_albedo.py`. Lines run
//! along the Moon's latitude and thicken with the light on the surface; a
//! second set cross-hatches the shade near the terminator; the night side is
//! a sparse stipple of earthshine; an orbit dial of 72 ticks lights up with
//! progress. Turning the Moon about its axis only shifts longitude, so the
//! geometry is worked out once per size and each frame is a map lookup and a
//! little arithmetic per sample. `tools/moon/engrave_proto.py` is the same
//! model, for look development.

use std::f32::consts::{PI, TAU};

use slint::{Rgb8Pixel, SharedPixelBuffer};

const ALBEDO: &[u8] = include_bytes!("../assets/moon-albedo-1024x512.u16");
const MAP_W: usize = 1024;
const MAP_H: usize = 512;

/// The Moon's radius as a share of the image width.
const RADIUS: f32 = 0.36;
/// The orbit dial's radius, in Moon radii.
const RING: f32 = 1.17;
const TICKS: f32 = 72.0;
/// Pixels between engraved lines at the centre, before supersampling.
const LINE_PERIOD: f32 = 5.2;
const CROSS_STRETCH: f32 = 1.15;

const BG: [f32; 3] = rgb(0x0c, 0x0f, 0x14);
const MOONLIGHT: [f32; 3] = rgb(0xec, 0xef, 0xf4);
const LUNA_BLUE: [f32; 3] = rgb(0x79, 0xca, 0xea);
const DIM_BLUE: [f32; 3] = rgb(0x2a, 0x4a, 0x5c);

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// One frame of the Moon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonFrame {
    /// Lit fraction of the disc: 0 new, 1 full.
    pub lit: f32,
    /// Rotation about the Moon's axis, in radians.
    pub spin: f32,
    /// Share of the orbit dial lit, 0..=1, clockwise from the top.
    pub progress: f32,
}

/// A sample on the disc.
struct Disc {
    index: u32,
    /// Map column without spin, and map row, in texels.
    u: f32,
    v: f32,
    /// View-space normal: x right, y up, z toward the viewer.
    n: [f32; 3],
    /// Distance to the nearest line, as a share of the line period.
    main: f32,
    cross: f32,
    /// Stipple: the dot's coverage here, and the cell's random draw.
    dot: f32,
    draw: f32,
    limb: f32,
}

/// A sample just outside the disc: limb, halo and the orbit dial.
struct Around {
    index: u32,
    /// Unit direction from the centre.
    dir: [f32; 2],
    halo: f32,
    limb: f32,
    hair: f32,
    tick: f32,
    /// Position round the dial, 0 at the top, clockwise.
    turn: f32,
}

pub struct MoonRenderer {
    albedo: Vec<f32>,
    size: u32,
    /// Samples per pixel along each axis.
    ss: u32,
    disc: Vec<Disc>,
    around: Vec<Around>,
    samples: Vec<[f32; 3]>,
}

impl Default for MoonRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MoonRenderer {
    pub fn new() -> Self {
        let albedo = ALBEDO
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| u16::from_le_bytes(b) as f32 / 65535.0)
            .collect();
        Self {
            albedo,
            size: 0,
            ss: 0,
            disc: Vec::new(),
            around: Vec::new(),
            samples: Vec::new(),
        }
    }

    /// The Moon as a `size`×`size` image on Luna's background, with
    /// `supersample`² samples per pixel (2 on ordinary screens, 1 on dense
    /// ones, whose pixels are already small).
    pub fn render(
        &mut self,
        size: u32,
        supersample: u32,
        frame: MoonFrame,
    ) -> SharedPixelBuffer<Rgb8Pixel> {
        let size = size.max(8);
        let ss = supersample.clamp(1, 4);
        if size != self.size || ss != self.ss {
            self.prepare(size, ss);
        }
        let n = (size * ss) as usize;
        self.samples.clear();
        self.samples.resize(n * n, BG);

        let lit = frame.lit.clamp(0.0, 1.0);
        let phi = (1.0 - 2.0 * lit).clamp(-1.0, 1.0).acos();
        let sun = [phi.sin(), 0.0, -phi.cos()];
        let shift = frame.spin / TAU * MAP_W as f32;
        let per_px = LINE_PERIOD * ss as f32;

        for d in &self.disc {
            let albedo = self.sample(d.u + shift, d.v);
            let mu0 = d.n[0] * sun[0] + d.n[1] * sun[1] + d.n[2] * sun[2];
            // Lommel-Seeliger, the Moon's own reflectance: flat-bright when full.
            let reflect = if mu0 > 0.0 {
                2.0 * mu0 / (mu0 + d.n[2] + 1e-6)
            } else {
                0.0
            };
            let light = (albedo * reflect * 0.62).clamp(0.0, 1.0);
            let main = line(d.main, per_px, light.powf(0.85) * 0.78);
            let shade = ((0.09 - light) / 0.09).clamp(0.0, 1.0) * f32::from(mu0 > 0.0);
            let cross = line(d.cross, per_px * CROSS_STRETCH, shade * 0.3)
                * (light * 10.0).clamp(0.0, 1.0)
                * 0.8;
            let night = mu0 <= 0.0 && d.draw < 0.015 + 0.06 * albedo;
            let stipple = if night { d.dot } else { 0.0 };

            let s = &mut self.samples[d.index as usize];
            add(s, MOONLIGHT, (main + cross).clamp(0.0, 1.0));
            add(s, LUNA_BLUE, stipple * 0.35);
            add(s, DIM_BLUE, d.limb * 0.8);
        }

        let glow = [
            (MOONLIGHT[0] + LUNA_BLUE[0]) * 0.5,
            (MOONLIGHT[1] + LUNA_BLUE[1]) * 0.5,
            (MOONLIGHT[2] + LUNA_BLUE[2]) * 0.5,
        ];
        let progress = frame.progress.clamp(0.0, 1.0);
        for a in &self.around {
            let toward = (a.dir[0] * sun[0] + a.dir[1] * sun[1]) * 0.5 + 0.5;
            let lit_side = toward.clamp(0.0, 1.0).powi(2);
            let done = a.turn <= progress;
            let s = &mut self.samples[a.index as usize];
            add(s, DIM_BLUE, a.limb * 0.8);
            add(s, glow, a.halo * lit_side * lit.sqrt() * 0.10);
            let unlit_tick = if done { 0.0 } else { a.tick * 0.55 };
            add(s, DIM_BLUE, a.hair * 0.45 + unlit_tick);
            if done {
                add(s, LUNA_BLUE, a.tick);
            }
        }

        self.resolve(size)
    }

    /// Works out every sample's fixed geometry for one image size.
    fn prepare(&mut self, size: u32, ss: u32) {
        let n = (size * ss) as usize;
        let r = n as f32 * RADIUS;
        let c = n as f32 / 2.0;
        let (sl, cl) = (-16f32).to_radians().sin_cos();
        let (sa, ca) = 58f32.to_radians().sin_cos();
        let period = LINE_PERIOD * ss as f32;
        let cell = 4.0 * ss as f32;
        let limb_width = 0.7 * ss as f32;

        self.size = size;
        self.ss = ss;
        self.disc.clear();
        self.around.clear();
        for y in 0..n {
            for x in 0..n {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let vx = (px - c) / r;
                let vy = (c - py) / r;
                let rr = (vx * vx + vy * vy).sqrt();
                let index = (y * n + x) as u32;
                let limb = (1.0 - (rr - 1.0).abs() * r / limb_width).clamp(0.0, 1.0);
                if rr < 1.0 {
                    let vz = (1.0 - rr * rr).max(0.0).sqrt();
                    // The Moon leans a little; lines follow its latitude.
                    let sx = cl * vx - sl * vy;
                    let sy = sl * vx + cl * vy;
                    let lon = sx.atan2(vz);
                    let lat = sy.clamp(-1.0, 1.0).asin();
                    let u = ((lon / TAU) + 0.5) * MAP_W as f32 - 0.5;
                    let v = (0.5 - lat / PI) * MAP_H as f32 - 0.5;
                    let main = (sy * r + vz * 0.10 * r) / period;
                    let cross_axis = ca * sx + sa * sy;
                    let cross = (cross_axis * r + vz * 0.08 * r) / (period * CROSS_STRETCH);

                    let ix = (px / cell).floor() as i64;
                    let iy = (py / cell).floor() as i64;
                    let jx = hash01(ix, iy);
                    let jy = hash01(iy + 91, ix + 17);
                    let dx = px - (ix as f32 + 0.2 + 0.6 * jx) * cell;
                    let dy = py - (iy as f32 + 0.2 + 0.6 * jy) * cell;
                    let dot = (1.2 * ss as f32 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);

                    self.disc.push(Disc {
                        index,
                        u,
                        v,
                        n: [vx, vy, vz],
                        main: distance_to_line(main),
                        cross: distance_to_line(cross),
                        dot,
                        draw: hash01(ix + 5, iy + 3),
                        limb,
                    });
                } else if rr < RING + 0.08 || limb > 0.0 {
                    let turn = (vx.atan2(vy) / TAU).rem_euclid(1.0);
                    let tick_gap =
                        (turn * TICKS - (turn * TICKS).round()).abs() * (TAU * RING * r / TICKS);
                    let in_band = rr > RING - 0.035 && rr < RING + 0.035;
                    let tick = if in_band {
                        (ss as f32 - tick_gap).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let hair = (1.0 - (rr - RING).abs() * r / (0.6 * ss as f32)).clamp(0.0, 1.0);
                    self.around.push(Around {
                        index,
                        dir: [vx / rr, vy / rr],
                        halo: (-(rr - 1.0) * 9.0).exp(),
                        limb,
                        hair,
                        tick,
                        turn,
                    });
                }
            }
        }
    }

    /// Bilinear albedo at a map position, wrapping in longitude.
    fn sample(&self, u: f32, v: f32) -> f32 {
        let x0f = u.floor();
        let y0f = v.floor().clamp(0.0, (MAP_H - 2) as f32);
        let fx = u - x0f;
        let fy = (v - y0f).clamp(0.0, 1.0);
        let x0 = (x0f as i64).rem_euclid(MAP_W as i64) as usize;
        let x1 = (x0 + 1) % MAP_W;
        let y0 = y0f as usize;
        let row0 = &self.albedo[y0 * MAP_W..];
        let row1 = &self.albedo[(y0 + 1) * MAP_W..];
        let a = row0[x0] * (1.0 - fx) + row0[x1] * fx;
        let b = row1[x0] * (1.0 - fx) + row1[x1] * fx;
        a * (1.0 - fy) + b * fy
    }

    /// Averages the samples into pixels.
    fn resolve(&self, size: u32) -> SharedPixelBuffer<Rgb8Pixel> {
        let ss = self.ss as usize;
        let n = size as usize * ss;
        let mut out = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
        let pixels = out.make_mut_slice();
        let scale = 255.0 / (ss * ss) as f32;
        for py in 0..size as usize {
            for px in 0..size as usize {
                let mut acc = [0.0f32; 3];
                for sy in 0..ss {
                    let row = (py * ss + sy) * n + px * ss;
                    for s in &self.samples[row..row + ss] {
                        for k in 0..3 {
                            acc[k] += s[k].clamp(0.0, 1.0);
                        }
                    }
                }
                pixels[py * size as usize + px] = Rgb8Pixel {
                    r: (acc[0] * scale).round() as u8,
                    g: (acc[1] * scale).round() as u8,
                    b: (acc[2] * scale).round() as u8,
                };
            }
        }
        out
    }
}

/// Distance from a line-space position to the nearest line, 0..=0.5.
fn distance_to_line(p: f32) -> f32 {
    (p - p.round()).abs()
}

/// An anti-aliased line of `width` (share of the period) at `dist`; a line
/// thinner than a pixel fades instead of staying half-strength.
fn line(dist: f32, per_px: f32, width: f32) -> f32 {
    let edge = ((width * 0.5 - dist) * per_px + 0.5).clamp(0.0, 1.0);
    edge * (width * per_px).clamp(0.0, 1.0)
}

fn add(s: &mut [f32; 3], color: [f32; 3], amount: f32) {
    for k in 0..3 {
        s[k] += (color[k] - BG[k]) * amount;
    }
}

/// A stable pseudo-random value in 0..=1 for a grid cell.
fn hash01(ix: i64, iy: i64) -> f32 {
    let h = (ix
        .wrapping_mul(374_761_393)
        .wrapping_add(iy.wrapping_mul(668_265_263))) as u64
        & 0xFFFF_FFFF;
    let h = ((h ^ (h >> 13)).wrapping_mul(1_274_126_177)) & 0xFFFF_FFFF;
    (h & 0xFFFF) as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brightness(img: &SharedPixelBuffer<Rgb8Pixel>, x: u32, y: u32, half: u32) -> f32 {
        let w = img.width();
        let px = img.as_slice();
        let mut sum = 0.0;
        let mut count = 0.0;
        for yy in y - half..y + half {
            for xx in x - half..x + half {
                let p = px[(yy * w + xx) as usize];
                sum += (p.r as f32 + p.g as f32 + p.b as f32) / 3.0;
                count += 1.0;
            }
        }
        sum / count
    }

    fn frame(lit: f32, progress: f32) -> MoonFrame {
        MoonFrame {
            lit,
            spin: 0.0,
            progress,
        }
    }

    #[test]
    fn a_new_moon_is_dark_and_a_full_one_is_lit() {
        let mut m = MoonRenderer::new();
        let new = m.render(200, 2, frame(0.0, 0.0));
        let full = m.render(200, 2, frame(1.0, 0.0));
        let (dark, bright) = (
            brightness(&new, 100, 100, 20),
            brightness(&full, 100, 100, 20),
        );
        assert!(dark < 20.0, "new moon centre {dark}");
        assert!(bright > dark + 25.0, "full {bright} vs new {dark}");
    }

    #[test]
    fn a_waxing_moon_is_lit_from_the_right() {
        let mut m = MoonRenderer::new();
        let half = m.render(200, 2, frame(0.5, 0.0));
        let (right, left) = (
            brightness(&half, 125, 100, 10),
            brightness(&half, 75, 100, 10),
        );
        assert!(right > left + 15.0, "right {right} vs left {left}");
    }

    #[test]
    fn the_dial_lights_with_progress() {
        let mut m = MoonRenderer::new();
        // The dial's right-hand side is a quarter turn round from the top.
        let ring_x = 100 + (200.0 * RADIUS * RING) as u32;
        let before = m.render(200, 2, frame(0.3, 0.1));
        let after = m.render(200, 2, frame(0.3, 0.4));
        let blue = |img: &SharedPixelBuffer<Rgb8Pixel>| {
            let w = img.width();
            (96..104)
                .flat_map(|y| (ring_x - 3..ring_x + 3).map(move |x| (x, y)))
                .map(|(x, y)| img.as_slice()[(y * w + x) as usize].b as u32)
                .max()
                .unwrap_or(0)
        };
        assert!(blue(&after) > blue(&before) + 40);
    }

    #[test]
    fn turning_moves_the_surface() {
        let mut m = MoonRenderer::new();
        let a = m.render(160, 2, frame(1.0, 0.0));
        let b = m.render(
            160,
            2,
            MoonFrame {
                spin: 0.8,
                ..frame(1.0, 0.0)
            },
        );
        assert_ne!(a.as_slice(), b.as_slice());
    }
}
