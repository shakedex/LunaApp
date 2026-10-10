//! The one display rendering every camera shares: scene-linear Rec.709 →
//! display-referred Rec.709 for a BT.1886 (gamma 2.4) display.
//!
//! Tone curve: our own fit in the shape of the Michaelis–Menten "daniele"
//! tonescale that ACES 2.0 uses (a hyperbolic shoulder raised to a contrast
//! power, then a quadratic toe):
//!
//! ```text
//!   f(x) = M · (x / (x + S))^G          shoulder + contrast
//!   y    = f² / (f + T)                 toe, y is display-linear
//! ```
//!
//! `G` (contrast) and `T` (toe) are the ACES 2.0 SDR defaults (1.15, 0.04). `M`
//! makes the curve's asymptote 1.02, so display white is reached around eight
//! stops over grey and everything below rolls off gently instead of clipping.
//! `S` is solved so 18% grey lands at 0.42 BT.1886 signal (code 107 of 255),
//! close to where camera makers' own Rec.709 looks put grey. The curve is
//! applied per channel, which desaturates bright colours towards white the way
//! film and camera LUTs do. Out-of-gamut colours (negative after the gamut
//! matrix) are first pulled towards their own luminance until they fit.

/// Contrast power (ACES 2.0 SDR default).
const G: f64 = 1.15;
/// Toe strength (ACES 2.0 SDR default `t_1`).
const T: f64 = 0.04;
/// Shoulder scale: `M² / (M + T) = 1.02`, the curve's asymptote.
const M: f64 = 1.058_543_526_076_099_1;
/// Shoulder knee, solved so `signal(0.18) = 0.42`.
const S: f64 = 0.768_604_473_386_746_3;

/// BT.1886 display gamma.
const GAMMA: f64 = 2.4;

/// Rec.709 luminance weights.
pub const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// Scene-linear (18% grey = 0.18) → display-linear light, 0..1.
pub fn tone(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let f = M * (x / (x + S)).powf(G);
    (f * f / (f + T)).min(1.0)
}

/// Display-linear light → BT.1886 signal (inverse EOTF, black at 0).
pub fn encode_display(l: f64) -> f64 {
    l.clamp(0.0, 1.0).powf(1.0 / GAMMA)
}

/// BT.1886 signal → display-linear light.
pub fn decode_display(v: f64) -> f64 {
    v.clamp(0.0, 1.0).powf(GAMMA)
}

/// Scene-linear → display signal 0..1: the whole rendering for one channel.
pub fn signal(x: f64) -> f64 {
    encode_display(tone(x))
}

/// Pulls a linear Rec.709 colour with negative components towards its own
/// luminance until the most negative component reaches zero; luminance is
/// unchanged. A non-positive luminance gives black.
#[inline]
pub fn gamut_clip(rgb: [f32; 3]) -> [f32; 3] {
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    if min >= 0.0 {
        return rgb;
    }
    let y = LUMA[0] * rgb[0] + LUMA[1] * rgb[1] + LUMA[2] * rgb[2];
    if y <= 0.0 {
        return [0.0; 3];
    }
    let k = y / (y - min);
    rgb.map(|c| (y + k * (c - y)).max(0.0))
}

/// Table for `signal` over scene-linear input, indexed by `x / (x + 1)` so a
/// fixed number of entries covers deep shadows to far highlights.
pub struct SceneToDisplay {
    table: Vec<f32>,
}

const SCENE_ENTRIES: usize = 16_384;

impl SceneToDisplay {
    pub fn new() -> Self {
        let n = SCENE_ENTRIES;
        let table = (0..=n)
            .map(|i| {
                let u = i as f64 / n as f64;
                let x = if i == n { f64::INFINITY } else { u / (1.0 - u) };
                (if x.is_finite() { signal(x) } else { 1.0 }) as f32
            })
            .collect();
        Self { table }
    }

    /// Scene-linear → display signal 0..1.
    #[inline]
    pub fn signal(&self, x: f32) -> f32 {
        if x.is_nan() || x <= 0.0 {
            return 0.0;
        }
        let u = x / (x + 1.0);
        let pos = u * SCENE_ENTRIES as f32;
        let i = (pos as usize).min(SCENE_ENTRIES - 1);
        let frac = pos - i as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * frac
    }
}

/// Display signal 0..1 → 8-bit code, rounded.
#[inline]
pub fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighteen_percent_grey_lands_at_042_signal() {
        assert!((signal(0.18) - 0.42).abs() < 1e-6, "{}", signal(0.18));
    }

    #[test]
    fn asymptote_is_102_and_black_is_black() {
        assert!((M * M / (M + T) - 1.02).abs() < 1e-12);
        assert_eq!(tone(0.0), 0.0);
        assert_eq!(tone(-1.0), 0.0);
    }

    #[test]
    fn curve_is_monotonic_with_a_gentle_shoulder() {
        let mut prev = -1.0;
        for i in -120..=100 {
            let x = 0.18 * 2f64.powf(i as f64 / 10.0);
            let v = signal(x);
            assert!(v >= prev, "not monotonic at {x}");
            prev = v;
        }
        // One stop over grey is clearly brighter but far from white;
        // six stops over is near white but not clipped.
        let plus1 = signal(0.36);
        assert!((0.52..0.62).contains(&plus1), "{plus1}");
        let plus6 = signal(0.18 * 64.0);
        assert!((0.95..1.0).contains(&plus6), "{plus6}");
        // Four stops under is dark but not crushed.
        let minus4 = signal(0.18 / 16.0);
        assert!((0.03..0.12).contains(&minus4), "{minus4}");
    }

    #[test]
    fn gamut_clip_keeps_in_gamut_colours_and_luminance() {
        assert_eq!(gamut_clip([0.2, 0.5, 0.1]), [0.2, 0.5, 0.1]);
        let c = [-0.2, 0.6, 0.3];
        let out = gamut_clip(c);
        let y = |p: [f32; 3]| LUMA[0] * p[0] + LUMA[1] * p[1] + LUMA[2] * p[2];
        assert!(out.iter().all(|&v| v >= 0.0));
        assert!((y(out) - y(c)).abs() < 1e-6);
        assert!(out[0].abs() < 1e-6, "most negative channel lands on zero");
        assert_eq!(gamut_clip([-0.5, -0.1, 0.2]), [0.0; 3]);
    }

    #[test]
    fn table_matches_the_direct_curve_within_half_a_code() {
        let lut = SceneToDisplay::new();
        for i in -160..=120 {
            let x = 0.18 * 2f64.powf(i as f64 / 10.0);
            let a = lut.signal(x as f32) as f64 * 255.0;
            let b = signal(x) * 255.0;
            assert!((a - b).abs() < 0.5, "{x}: {a} vs {b}");
        }
        assert_eq!(lut.signal(0.0), 0.0);
        assert_eq!(lut.signal(f32::NAN), 0.0);
        assert!(lut.signal(1e30) <= 1.0);
    }

    #[test]
    fn display_round_trip() {
        for v in [0.0, 0.1, 0.42, 1.0] {
            assert!((encode_display(decode_display(v)) - v).abs() < 1e-12);
        }
    }
}
