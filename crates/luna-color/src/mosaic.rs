//! Raw sensor mosaics → linear camera RGB at thumbnail scale.
//!
//! No demosaic in the usual sense: each 2×2 CFA cell becomes one RGB
//! super-pixel (R, mean of the two greens, B), and `kx × ky` super-pixels are
//! box-averaged while the mosaic is read, in integers. For thumbnails ≥6×
//! smaller than the sensor this is alias-free enough and touches each sample
//! once. Odd trailing rows/columns are dropped.

use luna_decode::Cfa;
use luna_formats::Gamut;

use crate::scale::Image;

/// Positions of R and B in a 2×2 cell, numbered p00, p01, p10, p11.
fn red_blue(cfa: Cfa) -> (usize, usize) {
    match cfa {
        Cfa::Rggb => (0, 3),
        Cfa::Grbg => (1, 2),
        Cfa::Gbrg => (2, 1),
        Cfa::Bggr => (3, 0),
    }
}

/// Number of super-pixel columns and rows a `width × height` mosaic has.
pub fn cells(width: usize, height: usize) -> (usize, usize) {
    (width / 2, height / 2)
}

/// Box-averages `kx × ky` CFA cells into one RGB pixel of raw sample means
/// (black not yet subtracted). `height` must not exceed `data.len() / width`.
pub fn reduce(width: usize, height: usize, cfa: Cfa, data: &[u16], kx: usize, ky: usize) -> Image {
    match red_blue(cfa) {
        (0, 3) => reduce_with::<0, 3>(width, height, data, kx, ky),
        (1, 2) => reduce_with::<1, 2>(width, height, data, kx, ky),
        (2, 1) => reduce_with::<2, 1>(width, height, data, kx, ky),
        _ => reduce_with::<3, 0>(width, height, data, kx, ky),
    }
}

fn reduce_with<const R: usize, const B: usize>(
    width: usize,
    height: usize,
    data: &[u16],
    kx: usize,
    ky: usize,
) -> Image {
    let (cw, ch) = cells(width, height);
    let (ow, oh) = (cw / kx, ch / ky);
    let mut out = Vec::with_capacity(ow * oh * 3);
    let mut acc = vec![[0u64; 3]; ow];
    let n = (kx * ky) as f32;
    for oy in 0..oh {
        acc.iter_mut().for_each(|a| *a = [0; 3]);
        for cy in oy * ky..(oy + 1) * ky {
            let row0 = &data[2 * cy * width..(2 * cy + 1) * width];
            let row1 = &data[(2 * cy + 1) * width..(2 * cy + 2) * width];
            for (ox, a) in acc.iter_mut().enumerate() {
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                let x0 = 2 * ox * kx;
                for x in (x0..x0 + 2 * kx).step_by(2) {
                    let p = [
                        row0[x] as u32,
                        row0[x + 1] as u32,
                        row1[x] as u32,
                        row1[x + 1] as u32,
                    ];
                    let (pr, pb) = (p[R], p[B]);
                    r += pr;
                    b += pb;
                    g += p[0] + p[1] + p[2] + p[3] - pr - pb;
                }
                a[0] += r as u64;
                a[1] += g as u64;
                a[2] += b as u64;
            }
        }
        for a in &acc {
            out.push(a[0] as f32 / n);
            out.push(a[1] as f32 / (2.0 * n));
            out.push(a[2] as f32 / n);
        }
    }
    Image {
        width: ow,
        height: oh,
        data: out,
    }
}

/// How many stops above 18% grey the sensor clips (raw white) by default, per
/// camera family, so raw thumbnails sit at the exposure the camera's own
/// Rec.709 would show. The mosaic carries no exposure metadata, so these are
/// nominal figures at each camera's base EI:
/// - ALEXA (AWG3): ARRI quotes 7.8 stops over 18% grey at EI 800.
/// - ALEXA 35 (AWG4): ARRI quotes 10 stops over at EI 800.
/// - everything else (Sony X-OCN, Canon CRM, unknown): 6 stops, a typical
///   base-ISO highlight latitude for current cinema sensors.
pub fn clip_stops_above_grey(gamut: Option<Gamut>) -> f32 {
    match gamut {
        Some(Gamut::ArriWideGamut3) => 7.8,
        Some(Gamut::ArriWideGamut4) => 10.0,
        _ => 6.0,
    }
}

/// Turns raw sample means into scene-linear camera RGB (18% grey = 0.18):
/// subtract black, normalise by the clip range, white-balance (multipliers
/// scaled so the smallest is 1, then clipped at 1 so blown highlights go white,
/// not magenta), then scale so sensor clip sits `clip_stops` over grey.
/// Missing `wb` means 1,1,1: the picture is shown as the sensor saw it.
pub fn linearise(img: &mut Image, black: f32, white: f32, wb: Option<[f32; 3]>, clip_stops: f32) {
    let range = (white - black).max(1e-6);
    let wb = wb
        .filter(|w| w.iter().all(|v| v.is_finite() && *v > 0.0))
        .unwrap_or([1.0; 3]);
    let min = wb[0].min(wb[1]).min(wb[2]);
    let exposure = 0.18 * clip_stops.exp2();
    let k = wb.map(|w| w / min / range);
    for px in img.data.chunks_exact_mut(3) {
        for c in 0..3 {
            px[c] = ((px[c] - black) * k[c]).clamp(0.0, 1.0) * exposure;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mosaic whose every cell holds the given R, G, B in `cfa` layout.
    fn flat(width: usize, height: usize, cfa: Cfa, rgb: [u16; 3]) -> Vec<u16> {
        let (r, b) = red_blue(cfa);
        let mut cell = [rgb[1]; 4];
        cell[r] = rgb[0];
        cell[b] = rgb[2];
        let mut data = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                data[y * width + x] = cell[(y % 2) * 2 + x % 2];
            }
        }
        data
    }

    #[test]
    fn every_cfa_layout_bins_to_the_right_channels() {
        for cfa in [Cfa::Rggb, Cfa::Grbg, Cfa::Gbrg, Cfa::Bggr] {
            let data = flat(8, 4, cfa, [1000, 2000, 3000]);
            let img = reduce(8, 4, cfa, &data, 1, 1);
            assert_eq!((img.width, img.height), (4, 2));
            for px in img.data.chunks(3) {
                assert_eq!(px, [1000.0, 2000.0, 3000.0], "{cfa:?}");
            }
        }
    }

    #[test]
    fn rggb_cell_positions_are_literal() {
        // R G / G B with distinct greens: G is their mean.
        let data = vec![10, 20, 30, 40];
        let img = reduce(2, 2, Cfa::Rggb, &data, 1, 1);
        assert_eq!(img.data, vec![10.0, 25.0, 40.0]);
        let img = reduce(2, 2, Cfa::Bggr, &data, 1, 1);
        assert_eq!(img.data, vec![40.0, 25.0, 10.0]);
    }

    #[test]
    fn box_reduction_averages_cells_and_drops_remainders() {
        // 6×2 RGGB = 3 cells wide; kx = 2 keeps one output column of cells 0–1.
        let mut data = vec![0u16; 12];
        for cx in 0..3 {
            let v = (cx as u16 + 1) * 100;
            data[2 * cx] = v; // R
            data[6 + 2 * cx + 1] = v * 2; // B
        }
        let img = reduce(6, 2, Cfa::Rggb, &data, 2, 1);
        assert_eq!((img.width, img.height), (1, 1));
        assert_eq!(img.data, vec![150.0, 0.0, 300.0]);
    }

    #[test]
    fn odd_sizes_drop_the_trailing_row_and_column() {
        let data = flat(5, 3, Cfa::Rggb, [1, 2, 3]);
        let img = reduce(5, 3, Cfa::Rggb, &data, 1, 1);
        assert_eq!((img.width, img.height), (2, 1));
    }

    #[test]
    fn linearise_levels_wb_and_exposure() {
        let mut img = Image {
            width: 1,
            height: 1,
            data: vec![
                256.0 + 0.1 * 1000.0,
                256.0 + 0.2 * 1000.0,
                256.0 + 0.05 * 1000.0,
            ],
        };
        linearise(&mut img, 256.0, 1256.0, Some([2.0, 1.0, 4.0]), 6.0);
        let e = 0.18 * 64.0;
        let want = [0.2 * e, 0.2 * e, 0.2 * e];
        for (a, b) in img.data.iter().zip(want) {
            assert!((a - b).abs() < 1e-4, "{:?}", img.data);
        }
    }

    #[test]
    fn blown_highlights_stay_white() {
        let mut img = Image {
            width: 1,
            height: 1,
            data: vec![4095.0; 3],
        };
        linearise(&mut img, 0.0, 4095.0, Some([2.2, 1.0, 1.6]), 6.0);
        assert!(img.data.iter().all(|&v| (v - img.data[0]).abs() < 1e-6));
    }

    #[test]
    fn missing_or_bad_wb_is_unity() {
        let mut a = Image {
            width: 1,
            height: 1,
            data: vec![100.0, 200.0, 300.0],
        };
        let mut b = a.clone();
        linearise(&mut a, 0.0, 1000.0, None, 6.0);
        linearise(&mut b, 0.0, 1000.0, Some([0.0, f32::NAN, 1.0]), 6.0);
        assert_eq!(a, b);
        assert!((a.data[1] / a.data[0] - 2.0).abs() < 1e-6);
    }
}
