//! Thumbnail geometry and area (box) resampling.
//!
//! Big pictures are first reduced by whole-number box factors while they are
//! read (see each input path), then [`area_resample`] does the exact
//! fractional box filter down (or, for anamorphic desqueeze, up) to the final
//! size on the much smaller intermediate.

/// Interleaved RGB, `f32`, in whatever domain the caller is working in.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

/// Output size for a `width × height` picture: desqueezed by `pixel_aspect`
/// (num/den widens), then scaled down to at most `max_width` (never up).
pub fn fit(
    width: u32,
    height: u32,
    max_width: u32,
    pixel_aspect: Option<(u32, u32)>,
) -> (u32, u32) {
    let (num, den) = match pixel_aspect {
        Some((n, d)) if n > 0 && d > 0 => (n as f64, d as f64),
        _ => (1.0, 1.0),
    };
    let dw = width.max(1) as f64 * num / den;
    let dh = height.max(1) as f64;
    let max_w = max_width.max(1) as f64;
    let (w, h) = if dw > max_w {
        (max_w, dh * max_w / dw)
    } else {
        (dw, dh)
    };
    ((w.round() as u32).max(1), (h.round() as u32).max(1))
}

/// The whole-number box factor to reduce `src` samples towards `dst` before
/// the exact resample: as large as possible without going below `dst`.
pub fn reduction(src: usize, dst: usize) -> usize {
    (src / dst.max(1)).max(1)
}

/// For each output index, the source indices it covers and their weights
/// (summing to 1). Works for both down- and up-scaling.
fn weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|o| {
            let (a, b) = (o as f64 * scale, (o + 1) as f64 * scale);
            let first = a.floor() as usize;
            let last = (b.ceil() as usize).min(src).max(first + 1);
            let w: Vec<f32> = (first..last)
                .map(|i| {
                    let lo = a.max(i as f64);
                    let hi = b.min((i + 1) as f64);
                    ((hi - lo).max(0.0) / scale) as f32
                })
                .collect();
            let sum: f32 = w.iter().sum();
            (first, w.into_iter().map(|v| v / sum).collect())
        })
        .collect()
}

/// Exact box (area-average) resample to `width × height`.
pub fn area_resample(src: &Image, width: usize, height: usize) -> Image {
    if src.width == width && src.height == height {
        return src.clone();
    }
    let wx = weights(src.width, width);
    let wy = weights(src.height, height);

    // Horizontal pass: src.height rows of `width`.
    let mut tmp = vec![0f32; width * src.height * 3];
    for y in 0..src.height {
        let row = &src.data[y * src.width * 3..(y + 1) * src.width * 3];
        let out = &mut tmp[y * width * 3..(y + 1) * width * 3];
        for (o, (first, w)) in wx.iter().enumerate() {
            let mut acc = [0f32; 3];
            for (k, &wk) in w.iter().enumerate() {
                let p = (first + k) * 3;
                acc[0] += row[p] * wk;
                acc[1] += row[p + 1] * wk;
                acc[2] += row[p + 2] * wk;
            }
            out[o * 3..o * 3 + 3].copy_from_slice(&acc);
        }
    }

    // Vertical pass.
    let stride = width * 3;
    let mut data = vec![0f32; stride * height];
    for (o, (first, w)) in wy.iter().enumerate() {
        let out = &mut data[o * stride..(o + 1) * stride];
        for (k, &wk) in w.iter().enumerate() {
            let row = &tmp[(first + k) * stride..(first + k + 1) * stride];
            for (d, s) in out.iter_mut().zip(row) {
                *d += s * wk;
            }
        }
    }
    Image {
        width,
        height,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_to_max_width_keeping_aspect() {
        assert_eq!(fit(3840, 2160, 640, None), (640, 360));
        assert_eq!(fit(8192, 4320, 640, None), (640, 338));
        assert_eq!(fit(4096, 2160, 640, Some((1, 1))), (640, 338));
    }

    #[test]
    fn never_upscales() {
        assert_eq!(fit(320, 240, 640, None), (320, 240));
        assert_eq!(fit(640, 480, 640, None), (640, 480));
    }

    #[test]
    fn anamorphic_is_desqueezed_before_fitting() {
        // 2× squeeze, 4:3 sensor area → 2.66:1 picture.
        assert_eq!(fit(2880, 2160, 640, Some((2, 1))), (640, 240));
        // 1.3× squeeze.
        assert_eq!(fit(3840, 2160, 640, Some((13, 10))), (640, 277));
        // Small anamorphic picture: widened, still not past max width.
        assert_eq!(fit(200, 100, 640, Some((2, 1))), (400, 100));
        // Nonsense aspect is ignored.
        assert_eq!(fit(1920, 1080, 640, Some((0, 1))), (640, 360));
    }

    #[test]
    fn degenerate_sizes_stay_at_least_one_pixel() {
        assert_eq!(fit(10000, 1, 100, None), (100, 1));
        assert_eq!(fit(0, 0, 0, None), (1, 1));
    }

    #[test]
    fn reduction_never_goes_below_the_target() {
        assert_eq!(reduction(4096, 640), 6);
        assert_eq!(reduction(640, 640), 1);
        assert_eq!(reduction(100, 640), 1);
    }

    fn image(width: usize, height: usize, f: impl Fn(usize, usize) -> [f32; 3]) -> Image {
        let mut data = Vec::new();
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&f(x, y));
            }
        }
        Image {
            width,
            height,
            data,
        }
    }

    #[test]
    fn area_resample_keeps_a_flat_field_flat() {
        let src = image(7, 5, |_, _| [0.25, 0.5, 0.75]);
        let out = area_resample(&src, 3, 2);
        assert_eq!((out.width, out.height), (3, 2));
        for px in out.data.chunks(3) {
            for (a, b) in px.iter().zip([0.25, 0.5, 0.75]) {
                assert!((a - b).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn area_resample_averages_and_upsamples() {
        let src = image(4, 1, |x, _| [x as f32; 3]);
        let half = area_resample(&src, 2, 1);
        assert_eq!(half.data, vec![0.5, 0.5, 0.5, 2.5, 2.5, 2.5]);
        // 3 → 2 splits the middle pixel between both outputs.
        let src = image(3, 1, |x, _| [x as f32 * 3.0; 3]);
        let out = area_resample(&src, 2, 1);
        assert!((out.data[0] - 1.0).abs() < 1e-6, "{:?}", out.data);
        assert!((out.data[3] - 5.0).abs() < 1e-6, "{:?}", out.data);
        // Upscaling 1 → 2 repeats the pixel.
        let src = image(1, 1, |_, _| [0.3, 0.6, 0.9]);
        let up = area_resample(&src, 2, 1);
        assert_eq!(up.data, vec![0.3, 0.6, 0.9, 0.3, 0.6, 0.9]);
    }
}
