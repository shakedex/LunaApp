//! Planar Y′CbCr → R′G′B′ signal at thumbnail scale.
//!
//! Each plane is box-reduced by a whole-number factor first (so a 4K frame is
//! touched once, in integers), chroma is then bilinearly brought to the
//! reduced luma size (centre-sited; MPEG-2 left siting is a sub-pixel shift at
//! this scale), and the matrix is applied on the small picture.
//!
//! Output is the R′G′B′ *signal*: 0 at reference black and 1 at reference
//! white, i.e. legal range expanded for limited-range files.

use luna_decode::YcbcrMatrix;

use crate::scale::{Image, reduction};

/// (Kr, Kb) luma coefficients: ITU-R BT.601, BT.709 and BT.2020.
fn kr_kb(m: YcbcrMatrix) -> (f32, f32) {
    match m {
        YcbcrMatrix::Bt601 => (0.299, 0.114),
        YcbcrMatrix::Bt709 => (0.2126, 0.0722),
        YcbcrMatrix::Bt2020 => (0.2627, 0.0593),
    }
}

/// Code-to-signal scaling for one sample depth and range (ITU-R BT.709 /
/// BT.2100 quantisation; H.273 for full range).
#[derive(Debug, Clone, Copy)]
pub struct Levels {
    y_off: f32,
    y_scale: f32,
    c_off: f32,
    c_scale: f32,
}

impl Levels {
    pub fn new(bits: u8, full_range: bool) -> Self {
        let bits = bits.clamp(8, 16) as i32;
        let max = ((1u32 << bits) - 1) as f32;
        let unit = 2f32.powi(bits - 8);
        if full_range {
            Levels {
                y_off: 0.0,
                y_scale: 1.0 / max,
                c_off: 2f32.powi(bits - 1),
                c_scale: 1.0 / max,
            }
        } else {
            Levels {
                y_off: 16.0 * unit,
                y_scale: 1.0 / (219.0 * unit),
                c_off: 128.0 * unit,
                c_scale: 1.0 / (224.0 * unit),
            }
        }
    }
}

/// One Y′CbCr sample (as codes, possibly fractional after averaging) → R′G′B′
/// signal.
#[inline]
pub fn to_rgb(y: f32, cb: f32, cr: f32, l: &Levels, k: (f32, f32)) -> [f32; 3] {
    let (kr, kb) = k;
    let y = (y - l.y_off) * l.y_scale;
    let cb = (cb - l.c_off) * l.c_scale;
    let cr = (cr - l.c_off) * l.c_scale;
    let r = y + 2.0 * (1.0 - kr) * cr;
    let b = y + 2.0 * (1.0 - kb) * cb;
    let g = (y - kr * r - kb * b) / (1.0 - kr - kb);
    [r, g, b]
}

/// Signal → full-range code fraction (`cv / (2^bits − 1)`), the domain camera
/// log curves are written in. Identity for full-range files.
pub fn signal_to_code(bits: u8, full_range: bool) -> impl Fn(f32) -> f32 {
    let bits = bits.clamp(8, 16) as i32;
    let max = ((1u32 << bits) - 1) as f32;
    let unit = 2f32.powi(bits - 8);
    let (off, scale) = if full_range {
        (0.0, 1.0)
    } else {
        (16.0 * unit / max, 219.0 * unit / max)
    };
    move |s| off + scale * s
}

/// Box-averages a plane by `kx × ky` into `f32` means; trailing partial
/// blocks are dropped.
pub fn reduce_plane(
    plane: &[u16],
    width: usize,
    height: usize,
    kx: usize,
    ky: usize,
) -> (Vec<f32>, usize, usize) {
    let (ow, oh) = ((width / kx).max(1), (height / ky).max(1));
    let (kx, ky) = (kx.min(width), ky.min(height));
    let mut out = Vec::with_capacity(ow * oh);
    let mut acc = vec![0u64; ow];
    let n = (kx * ky) as f32;
    for oy in 0..oh {
        acc.iter_mut().for_each(|a| *a = 0);
        for y in oy * ky..(oy + 1) * ky {
            let row = &plane[y * width..(y + 1) * width];
            for (ox, a) in acc.iter_mut().enumerate() {
                let s: u32 = row[ox * kx..(ox + 1) * kx].iter().map(|&v| v as u32).sum();
                *a += s as u64;
            }
        }
        out.extend(acc.iter().map(|&a| a as f32 / n));
    }
    (out, ow, oh)
}

/// Bilinear resample of a single plane, centre-aligned.
fn bilinear(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    let map = |o: usize, s: usize, d: usize| {
        let p = ((o as f32 + 0.5) * s as f32 / d as f32 - 0.5).clamp(0.0, (s - 1) as f32);
        let i = (p as usize).min(s.saturating_sub(2));
        let i1 = (i + 1).min(s - 1);
        (i, i1, p - i as f32)
    };
    let xs: Vec<_> = (0..dw).map(|x| map(x, sw, dw)).collect();
    let mut out = Vec::with_capacity(dw * dh);
    for y in 0..dh {
        let (y0, y1, fy) = map(y, sh, dh);
        let (r0, r1) = (&src[y0 * sw..(y0 + 1) * sw], &src[y1 * sw..(y1 + 1) * sw]);
        for &(x0, x1, fx) in &xs {
            let top = r0[x0] + (r0[x1] - r0[x0]) * fx;
            let bot = r1[x0] + (r1[x1] - r1[x0]) * fx;
            out.push(top + (bot - top) * fy);
        }
    }
    out
}

/// A Y′CbCr frame's planes and coding, borrowed.
pub struct Planes<'a> {
    pub width: usize,
    pub height: usize,
    pub chroma_width: usize,
    pub chroma_height: usize,
    pub bits: u8,
    pub y: &'a [u16],
    pub cb: &'a [u16],
    pub cr: &'a [u16],
    pub matrix: YcbcrMatrix,
    pub full_range: bool,
}

impl Planes<'_> {
    /// True when every plane holds at least the samples its size promises.
    pub fn is_complete(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.chroma_width > 0
            && self.chroma_height > 0
            && self.y.len() >= self.width * self.height
            && self.cb.len() >= self.chroma_width * self.chroma_height
            && self.cr.len() >= self.chroma_width * self.chroma_height
    }

    /// Reduces towards `tw × th` and converts to R′G′B′ signal.
    pub fn reduce_to_rgb(&self, tw: usize, th: usize) -> Image {
        let (kx, ky) = (reduction(self.width, tw), reduction(self.height, th));
        let (y, rw, rh) = reduce_plane(self.y, self.width, self.height, kx, ky);
        let (ckx, cky) = (
            reduction(self.chroma_width, rw),
            reduction(self.chroma_height, rh),
        );
        let (cb, cw, ch) = reduce_plane(self.cb, self.chroma_width, self.chroma_height, ckx, cky);
        let (cr, _, _) = reduce_plane(self.cr, self.chroma_width, self.chroma_height, ckx, cky);
        let cb = bilinear(&cb, cw, ch, rw, rh);
        let cr = bilinear(&cr, cw, ch, rw, rh);
        let levels = Levels::new(self.bits, self.full_range);
        let k = kr_kb(self.matrix);
        let mut data = Vec::with_capacity(rw * rh * 3);
        for i in 0..rw * rh {
            data.extend_from_slice(&to_rgb(y[i], cb[i], cr[i], &levels, k));
        }
        Image {
            width: rw,
            height: rh,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgb(got: [f32; 3], want: [f32; 3]) {
        for (a, b) in got.iter().zip(want) {
            assert!((a - b).abs() < 0.003, "{got:?} vs {want:?}");
        }
    }

    /// Exact (unrounded) codes for an R′G′B′ signal, the way a camera would code it.
    fn code(rgb: [f32; 3], m: YcbcrMatrix, bits: u8, full: bool) -> [f32; 3] {
        let (kr, kb) = kr_kb(m);
        let y = kr * rgb[0] + (1.0 - kr - kb) * rgb[1] + kb * rgb[2];
        let cb = (rgb[2] - y) / (2.0 * (1.0 - kb));
        let cr = (rgb[0] - y) / (2.0 * (1.0 - kr));
        let l = Levels::new(bits, full);
        [
            y / l.y_scale + l.y_off,
            cb / l.c_scale + l.c_off,
            cr / l.c_scale + l.c_off,
        ]
    }

    const BARS: [[f32; 3]; 8] = [
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
    ];

    #[test]
    fn bt709_10_bit_limited_known_codes() {
        let l = Levels::new(10, false);
        let k = kr_kb(YcbcrMatrix::Bt709);
        assert_rgb(to_rgb(940.0, 512.0, 512.0, &l, k), [1.0, 1.0, 1.0]);
        assert_rgb(to_rgb(64.0, 512.0, 512.0, &l, k), [0.0, 0.0, 0.0]);
        // 100% red in BT.709 10-bit: Y 250, Cb 409, Cr 960 (rounded codes).
        assert_rgb(to_rgb(250.0, 409.0, 960.0, &l, k), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn colour_bars_decode_for_every_matrix_depth_and_range() {
        for m in [YcbcrMatrix::Bt601, YcbcrMatrix::Bt709, YcbcrMatrix::Bt2020] {
            for bits in [8, 10, 12, 16] {
                for full in [false, true] {
                    let l = Levels::new(bits, full);
                    for bar in BARS {
                        let [y, cb, cr] = code(bar, m, bits, full);
                        let rounded = [y.round(), cb.round(), cr.round()];
                        let got = to_rgb(rounded[0], rounded[1], rounded[2], &l, kr_kb(m));
                        let tol = if bits == 8 { 0.01 } else { 0.003 };
                        for (a, b) in got.iter().zip(bar) {
                            assert!((a - b).abs() < tol, "{m:?} {bits} {full} {bar:?}: {got:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn matrices_differ_on_red() {
        // BT.601 red decoded as BT.709 is visibly wrong: the matrix matters.
        let l = Levels::new(10, false);
        let red601 = code([1.0, 0.0, 0.0], YcbcrMatrix::Bt601, 10, false);
        let as709 = to_rgb(
            red601[0],
            red601[1],
            red601[2],
            &l,
            kr_kb(YcbcrMatrix::Bt709),
        );
        assert!(as709[1].abs() > 0.05, "{as709:?}");
    }

    #[test]
    fn signal_to_code_maps_legal_range_into_code_fractions() {
        let f = signal_to_code(10, false);
        assert!((f(0.0) - 64.0 / 1023.0).abs() < 1e-6);
        assert!((f(1.0) - 940.0 / 1023.0).abs() < 1e-6);
        let g = signal_to_code(10, true);
        assert_eq!(g(0.5), 0.5);
    }

    #[test]
    fn reduce_plane_box_averages() {
        let plane: Vec<u16> = (0..16).collect();
        let (out, w, h) = reduce_plane(&plane, 4, 4, 2, 2);
        assert_eq!((w, h), (2, 2));
        assert_eq!(out, vec![2.5, 4.5, 10.5, 12.5]);
    }

    #[test]
    fn subsampled_chroma_is_upsampled_to_luma() {
        // 4:2:0, 4×2 luma, 2×1 chroma: left half red, right half blue.
        let red = code([1.0, 0.0, 0.0], YcbcrMatrix::Bt709, 10, false);
        let blue = code([0.0, 0.0, 1.0], YcbcrMatrix::Bt709, 10, false);
        let y = [red[0] as u16, red[0] as u16, blue[0] as u16, blue[0] as u16].repeat(2);
        let cb = vec![red[1] as u16, blue[1] as u16];
        let cr = vec![red[2] as u16, blue[2] as u16];
        let p = Planes {
            width: 4,
            height: 2,
            chroma_width: 2,
            chroma_height: 1,
            bits: 10,
            y: &y,
            cb: &cb,
            cr: &cr,
            matrix: YcbcrMatrix::Bt709,
            full_range: false,
        };
        let img = p.reduce_to_rgb(4, 2);
        assert_eq!((img.width, img.height), (4, 2));
        let px = |x: usize| [img.data[x * 3], img.data[x * 3 + 1], img.data[x * 3 + 2]];
        assert!(px(0)[0] > 0.9 && px(0)[2] < 0.1, "{:?}", px(0));
        assert!(px(3)[2] > 0.9 && px(3)[0] < 0.1, "{:?}", px(3));
    }

    #[test]
    fn large_frames_are_reduced_before_conversion() {
        let (w, h) = (64, 32);
        let y = vec![940u16; w * h];
        let c = vec![512u16; w * h / 4];
        let p = Planes {
            width: w,
            height: h,
            chroma_width: w / 2,
            chroma_height: h / 2,
            bits: 10,
            y: &y,
            cb: &c,
            cr: &c,
            matrix: YcbcrMatrix::Bt709,
            full_range: false,
        };
        assert!(p.is_complete());
        let img = p.reduce_to_rgb(16, 8);
        assert_eq!((img.width, img.height), (16, 8));
        assert!(img.data.iter().all(|&v| (v - 1.0).abs() < 1e-6));
    }
}
