//! Cropping and binning raw Bayer mosaics at thumbnail scale.

/// A rectangle in photosites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// The largest power-of-two binning factor (1, 2, 4 or 8) that keeps a mosaic
/// of `width` photosites at least `2 × min_width` wide (a picture pixel per
/// 2×2 photosites).
pub(crate) fn bin_factor(width: usize, min_width: u32) -> usize {
    let want = 2 * min_width as usize;
    [8, 4, 2]
        .into_iter()
        .find(|&f| width / f >= want)
        .unwrap_or(1)
}

/// Crops `rect` out of a `w × h` mosaic and bins it by `f`: each output 2×2
/// cell averages the same-colour photosites of a `2f × 2f` input block, so the
/// CFA layout is kept. The crop origin is rounded up to even photosites (also
/// keeping the CFA), and the result is a whole number of 2×2 cells.
pub(crate) fn crop_bin(
    data: &[u16],
    w: usize,
    h: usize,
    rect: Rect,
    f: usize,
) -> (Vec<u16>, usize, usize) {
    let f = f.max(1);
    let x0 = (rect.x + 1) & !1;
    let y0 = (rect.y + 1) & !1;
    let x1 = (rect.x + rect.width).min(w);
    let y1 = (rect.y + rect.height).min(h);
    let cw = x1.saturating_sub(x0);
    let ch = y1.saturating_sub(y0);
    let cells_x = cw / (2 * f);
    let cells_y = ch / (2 * f);
    let (ow, oh) = (cells_x * 2, cells_y * 2);
    if ow == 0 || oh == 0 || data.len() < w * h {
        return (Vec::new(), 0, 0);
    }
    let mut out = vec![0u16; ow * oh];
    let area = (f * f) as u32;
    for oy in 0..oh {
        let (cy, dy) = (oy / 2, oy % 2);
        let row_out = &mut out[oy * ow..(oy + 1) * ow];
        for (ox, o) in row_out.iter_mut().enumerate() {
            let (cx, dx) = (ox / 2, ox % 2);
            let mut sum = 0u32;
            for j in 0..f {
                let y = y0 + cy * 2 * f + 2 * j + dy;
                let row = &data[y * w..y * w + w];
                for i in 0..f {
                    sum += row[x0 + cx * 2 * f + 2 * i + dx] as u32;
                }
            }
            *o = ((sum + area / 2) / area) as u16;
        }
    }
    (out, ow, oh)
}

/// Lays an RGB picture out as an RGGB mosaic twice its size (R G / G B per
/// pixel), so it travels as a raw frame that still needs white balance.
pub(crate) fn rgb_to_rggb(rgb: &[u16], w: usize, h: usize) -> Vec<u16> {
    let ow = 2 * w;
    let mut out = vec![0u16; ow * 2 * h];
    for y in 0..h {
        let src = &rgb[y * w * 3..(y + 1) * w * 3];
        let (top, bottom) = out[2 * y * ow..(2 * y + 2) * ow].split_at_mut(ow);
        for x in 0..w {
            let (r, g, b) = (src[3 * x], src[3 * x + 1], src[3 * x + 2]);
            top[2 * x] = r;
            top[2 * x + 1] = g;
            bottom[2 * x] = g;
            bottom[2 * x + 1] = b;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_factor_keeps_twice_the_thumbnail_width() {
        assert_eq!(bin_factor(8192, 640), 4); // 2048 >= 1280
        assert_eq!(bin_factor(2048, 640), 1); // 1024 < 1280
        assert_eq!(bin_factor(2560, 640), 2);
        assert_eq!(bin_factor(12000, 640), 8);
        assert_eq!(bin_factor(100, 640), 1);
        assert_eq!(bin_factor(4096, 0), 8);
    }

    #[test]
    fn crop_only_keeps_cfa_by_rounding_origin_up() {
        // 6x4 mosaic, values = index
        let data: Vec<u16> = (0..24).collect();
        let full = Rect {
            x: 1,
            y: 1,
            width: 5,
            height: 3,
        };
        let (out, w, h) = crop_bin(&data, 6, 4, full, 1);
        // origin rounds to (2, 2); 4x2 left
        assert_eq!((w, h), (4, 2));
        assert_eq!(out, vec![14, 15, 16, 17, 20, 21, 22, 23]);
    }

    #[test]
    fn binning_averages_same_colour_sites() {
        // 4x4 RGGB, f=2 -> one 2x2 cell
        #[rustfmt::skip]
        let data: Vec<u16> = vec![
            10, 20, 30, 40,
            50, 60, 70, 80,
            12, 22, 32, 42,
            52, 62, 72, 82,
        ];
        let all = Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        let (out, w, h) = crop_bin(&data, 4, 4, all, 2);
        assert_eq!((w, h), (2, 2));
        // R: 10,30,12,32 -> 21; G1: 20,40,22,42 -> 31; G2: 50,70,52,72 -> 61; B -> 71
        assert_eq!(out, vec![21, 31, 61, 71]);
    }

    #[test]
    fn degenerate_rects_give_empty_mosaics() {
        let data = vec![0u16; 16];
        let r = Rect {
            x: 3,
            y: 3,
            width: 1,
            height: 1,
        };
        assert_eq!(crop_bin(&data, 4, 4, r, 1).1, 0);
        // data shorter than claimed
        let all = Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        assert_eq!(crop_bin(&data, 8, 8, all, 1).1, 0);
    }

    #[test]
    fn rgb_becomes_rggb() {
        let rgb = vec![1, 2, 3, 4, 5, 6];
        let m = rgb_to_rggb(&rgb, 2, 1);
        assert_eq!(m, vec![1, 2, 4, 5, 2, 3, 5, 6]);
    }
}
