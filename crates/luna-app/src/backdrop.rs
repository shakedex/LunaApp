//! The sky behind the Moon: a star chart's graticule on a large celestial
//! sphere, with stars fixed to it, turning slowly about its tilted pole.
//!
//! Like the Moon, turning only shifts longitude, so each pixel's latitude,
//! longitude and line widths are worked out once per window size and a frame
//! is a little arithmetic per pixel. Lines and stars fade out near the Moon so
//! the dial stays the focus.

use slint::{Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer};

const BG: [f32; 3] = rgb(0x0c, 0x0f, 0x14);
const MOONLIGHT: [f32; 3] = rgb(0xe4, 0xe8, 0xef);
const LUNA_BLUE: [f32; 3] = rgb(0x79, 0xca, 0xea);

/// Degrees between graticule lines.
const STEP: f32 = 15.0;
/// Strength of the graticule and of the stars.
const GRID: f32 = 0.075;
const STAR: f32 = 0.55;
const STARS: usize = 900;
/// The sphere leans with the Moon and its pole tips toward the viewer.
const LEAN: f32 = -16.0;
const TILT: f32 = 22.0;

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// Where the sky is drawn, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sky {
    pub width: u32,
    pub height: u32,
    /// The Moon's centre and the orbit dial's radius.
    pub centre: [f32; 2],
    pub dial: f32,
    /// Physical pixels per logical pixel.
    pub scale: f32,
}

/// A star on the sphere, and how bright and large it is.
struct Star {
    lat: f32,
    lon: f32,
    bright: f32,
    size: f32,
}

/// A pixel of the sphere that's clear of the Moon.
struct Point {
    index: u32,
    /// Longitude in graticule steps, and pixels per step east-west.
    lon: f32,
    px_per_step: f32,
    /// The fixed latitude lines here, and how far from the Moon (0 near,
    /// 1 clear).
    parallel: f32,
    calm: f32,
}

pub struct BackdropRenderer {
    sky: Option<Sky>,
    /// The sphere's radius, in pixels.
    radius: f32,
    points: Vec<Point>,
    stars: Vec<Star>,
    /// The plain background, and the last sky drawn with its turn.
    blank: SharedPixelBuffer<Rgb8Pixel>,
    last: Option<(f32, SharedPixelBuffer<Rgb8Pixel>)>,
}

impl Default for BackdropRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl BackdropRenderer {
    pub fn new() -> Self {
        let stars = (0..STARS as i64)
            .map(|i| {
                // Uniform on the sphere.
                let z = hash01(i, 1) * 2.0 - 1.0;
                let pick = hash01(i, 4);
                Star {
                    lat: z.asin().to_degrees(),
                    lon: hash01(i, 2) * 360.0 - 180.0,
                    bright: 0.35 + 0.65 * hash01(i, 3),
                    size: if pick < 0.5 {
                        0.6
                    } else if pick < 0.8 {
                        0.8
                    } else if pick < 0.95 {
                        1.1
                    } else {
                        1.6
                    },
                }
            })
            .collect();
        Self {
            sky: None,
            radius: 1.0,
            points: Vec::new(),
            stars,
            blank: SharedPixelBuffer::new(1, 1),
            last: None,
        }
    }

    /// The sky with the sphere turned by `turn` degrees. The sky turns
    /// slowly, so it's redrawn only once it has moved about half a pixel.
    pub fn render(&mut self, sky: Sky, turn: f32) -> SharedPixelBuffer<Rgb8Pixel> {
        let sky = Sky {
            width: sky.width.max(1),
            height: sky.height.max(1),
            ..sky
        };
        if self.sky != Some(sky) {
            self.prepare(sky);
        }
        let half_pixel = (0.5 / self.radius).to_degrees();
        if let Some((drawn, image)) = &self.last
            && (turn - drawn).abs() < half_pixel
        {
            return image.clone();
        }

        let width = 0.8 * sky.scale;
        let mut out = self.blank.clone();
        let pixels = out.make_mut_slice();
        let turn_steps = turn / STEP;
        for p in &self.points {
            let f = p.lon - turn_steps;
            let d = (f - f.round()).abs() * p.px_per_step;
            let meridian = (width / 2.0 + 0.5 - d).clamp(0.0, 1.0);
            let line = meridian.max(p.parallel) * GRID * p.calm;
            if line > 0.002 {
                add(&mut pixels[p.index as usize], LUNA_BLUE, line);
            }
        }
        self.draw_stars(pixels, sky, turn);
        self.last = Some((turn, out.clone()));
        out
    }

    fn prepare(&mut self, sky: Sky) {
        let (w, h) = (sky.width as usize, sky.height as usize);
        self.sky = Some(sky);
        self.radius = 0.86 * w.max(h) as f32;
        let (sl, cl) = LEAN.to_radians().sin_cos();
        let (st, ct) = TILT.to_radians().sin_cos();
        let r = self.radius;
        let mut lat = vec![f32::NAN; w * h];
        let mut lon = vec![f32::NAN; w * h];
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 + 0.5 - sky.centre[0];
                let dy = y as f32 + 0.5 - sky.centre[1];
                let u = (dx * cl + dy * sl) / r;
                let v = (-dx * sl + dy * cl) / r;
                let rr = u * u + v * v;
                if rr >= 1.0 {
                    continue;
                }
                let z = (1.0 - rr).sqrt();
                let yy = v * ct - z * st;
                let zz = v * st + z * ct;
                lat[y * w + x] = (-yy).clamp(-1.0, 1.0).asin().to_degrees();
                lon[y * w + x] = u.atan2(zz).to_degrees();
            }
        }

        // Line widths need each field's change per pixel.
        let width = 0.8 * sky.scale;
        let slope = |field: &[f32], i: usize, x: usize, y: usize, wrap: bool| {
            let diff = |a: f32, b: f32| {
                let d = a - b;
                if wrap {
                    (d + 180.0).rem_euclid(360.0) - 180.0
                } else {
                    d
                }
            };
            let here = field[i];
            let gx = if x + 1 < w {
                diff(field[i + 1], here)
            } else {
                diff(here, field[i - 1])
            };
            let gy = if y + 1 < h {
                diff(field[i + w], here)
            } else {
                diff(here, field[i - w])
            };
            let g = (gx * gx + gy * gy).sqrt();
            if g.is_finite() { g } else { 0.0 }
        };
        let inner = sky.dial + 14.0 * sky.scale;
        let outer = sky.dial + 90.0 * sky.scale;
        self.points.clear();
        self.last = None;
        self.blank = SharedPixelBuffer::new(sky.width, sky.height);
        let bg = Rgb8Pixel {
            r: (BG[0] * 255.0).round() as u8,
            g: (BG[1] * 255.0).round() as u8,
            b: (BG[2] * 255.0).round() as u8,
        };
        self.blank.make_mut_slice().fill(bg);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if lat[i].is_nan() {
                    continue;
                }
                let dx = x as f32 + 0.5 - sky.centre[0];
                let dy = y as f32 + 0.5 - sky.centre[1];
                let from_moon = (dx * dx + dy * dy).sqrt();
                let calm = ((from_moon - inner) / (outer - inner)).clamp(0.0, 1.0);
                if calm <= 0.0 {
                    continue;
                }
                let lon_per_px = slope(&lon, i, x, y, true);
                let lat_per_px = slope(&lat, i, x, y, false);
                let parallel = if lat_per_px > 0.0 {
                    let f = lat[i] / STEP;
                    let d = (f - f.round()).abs() * STEP / lat_per_px;
                    (width / 2.0 + 0.5 - d).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.points.push(Point {
                    index: i as u32,
                    lon: lon[i] / STEP,
                    px_per_step: if lon_per_px > 0.0 {
                        STEP / lon_per_px
                    } else {
                        0.0
                    },
                    parallel,
                    calm,
                });
            }
        }
    }

    fn draw_stars(&self, pixels: &mut [Rgb8Pixel], sky: Sky, turn: f32) {
        let (w, h) = (sky.width as i64, sky.height as i64);
        let (sl, cl) = LEAN.to_radians().sin_cos();
        let (st, ct) = TILT.to_radians().sin_cos();
        let inner = sky.dial + 4.0 * sky.scale;
        let outer = sky.dial + 40.0 * sky.scale;
        for star in &self.stars {
            let (slat, clat) = star.lat.to_radians().sin_cos();
            let (slon, clon) = (star.lon + turn).to_radians().sin_cos();
            let (u, yy, zz) = (clat * slon, -slat, clat * clon);
            let v = yy * ct + zz * st;
            let z = -yy * st + zz * ct;
            if z <= 0.0 {
                continue;
            }
            let sx = sky.centre[0] + self.radius * (u * cl - v * sl);
            let sy = sky.centre[1] + self.radius * (u * sl + v * cl);
            let from_moon = ((sx - sky.centre[0]).powi(2) + (sy - sky.centre[1]).powi(2)).sqrt();
            let calm = ((from_moon - inner) / (outer - inner)).clamp(0.0, 1.0);
            if calm <= 0.0 {
                continue;
            }
            let size = star.size * sky.scale;
            let reach = size.ceil() as i64 + 1;
            let (cx, cy) = (sx.floor() as i64, sy.floor() as i64);
            for py in (cy - reach).max(0)..(cy + reach + 1).min(h) {
                for px in (cx - reach).max(0)..(cx + reach + 1).min(w) {
                    let d =
                        ((px as f32 + 0.5 - sx).powi(2) + (py as f32 + 0.5 - sy).powi(2)).sqrt();
                    let cover = (size + 0.5 - d).clamp(0.0, 1.0);
                    if cover > 0.0 {
                        let p = &mut pixels[(py * w + px) as usize];
                        add(p, MOONLIGHT, cover * star.bright * STAR * calm);
                    }
                }
            }
        }
    }
}

/// Lays the Moon's premultiplied image over the sky, centred on `centre`:
/// the disc covers the sky, its halo and dial add light to it.
pub fn place_moon(
    sky: &mut SharedPixelBuffer<Rgb8Pixel>,
    moon: &SharedPixelBuffer<Rgba8Pixel>,
    centre: [f32; 2],
) {
    let (sw, sh) = (sky.width() as i64, sky.height() as i64);
    let (mw, mh) = (moon.width() as i64, moon.height() as i64);
    let left = (centre[0] - mw as f32 / 2.0).round() as i64;
    let top = (centre[1] - mh as f32 / 2.0).round() as i64;
    let src = moon.as_slice();
    let dst = sky.make_mut_slice();
    for y in 0..mh {
        let sy = top + y;
        if !(0..sh).contains(&sy) {
            continue;
        }
        for x in 0..mw {
            let sx = left + x;
            if !(0..sw).contains(&sx) {
                continue;
            }
            let m = src[(y * mw + x) as usize];
            let d = &mut dst[(sy * sw + sx) as usize];
            let keep = 255 - m.a as u32;
            let over = |c: u8, under: u8| (c as u32 + under as u32 * keep / 255).min(255) as u8;
            *d = Rgb8Pixel {
                r: over(m.r, d.r),
                g: over(m.g, d.g),
                b: over(m.b, d.b),
            };
        }
    }
}

/// Adds `amount` of `colour`'s light over the background to a pixel.
fn add(p: &mut Rgb8Pixel, colour: [f32; 3], amount: f32) {
    let lift = |c: u8, k: usize| {
        (c as f32 + (colour[k] - BG[k]) * amount * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    *p = Rgb8Pixel {
        r: lift(p.r, 0),
        g: lift(p.g, 1),
        b: lift(p.b, 2),
    };
}

/// A stable pseudo-random value in 0..=1.
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

    fn sky() -> Sky {
        Sky {
            width: 320,
            height: 240,
            centre: [160.0, 100.0],
            dial: 40.0,
            scale: 1.0,
        }
    }

    #[test]
    fn the_sky_is_quiet_round_the_moon() {
        let mut b = BackdropRenderer::new();
        let img = b.render(sky(), 0.0);
        let px = img.as_slice();
        let bg = Rgb8Pixel {
            r: 0x0c,
            g: 0x0f,
            b: 0x14,
        };
        for y in 70..130 {
            for x in 130..190 {
                assert_eq!(px[y * 320 + x], bg);
            }
        }
        assert!(px.iter().any(|p| *p != bg), "the sky has lines and stars");
    }

    #[test]
    fn the_sky_turns() {
        let mut b = BackdropRenderer::new();
        let a = b.render(sky(), 0.0);
        let c = b.render(sky(), 2.0);
        assert_ne!(a.as_slice(), c.as_slice());
    }
}
