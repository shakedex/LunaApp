//! 8×8 inverse DCT.
//!
//! RDD 36 defines the inverse transform mathematically (the orthonormal 2-D
//! DCT-III) rather than as a particular integer algorithm, so this is a
//! separable evaluation in single precision through an even/odd split of the
//! 8-point transform: accurate to far better than one output code, which is
//! what the specification asks for.

type Rows = [[f32; 8]; 8];

/// `cos(kπ/16) / 2`: the 1-D transform's ½ normalisation folded in.
const H1: f32 = 0.490_392_64;
const H2: f32 = 0.461_939_77;
const H3: f32 = 0.415_734_8;
const H4: f32 = 0.353_553_38;
const H5: f32 = 0.277_785_12;
const H6: f32 = 0.191_341_72;
const H7: f32 = 0.097_545_16;

/// 1-D inverse transform down each column: row `k` of `x` holds frequency
/// `k` for all eight columns, which the compiler processes as SIMD lanes.
///
/// `out[n] = Σ_k c(k)/2 · X[k] · cos((2n+1)kπ/16)`, evaluated as an even
/// part (k = 0, 2, 4, 6) and an odd part (k = 1, 3, 5, 7) whose sum and
/// difference give the mirrored outputs `n` and `7 − n`.
#[inline(always)]
fn columns(x: &Rows) -> Rows {
    let mut out = [[0f32; 8]; 8];
    for l in 0..8 {
        let a0 = (x[0][l] + x[4][l]) * H4;
        let a1 = (x[0][l] - x[4][l]) * H4;
        let b0 = x[2][l] * H2 + x[6][l] * H6;
        let b1 = x[2][l] * H6 - x[6][l] * H2;
        let even = [a0 + b0, a1 + b1, a1 - b1, a0 - b0];

        let (x1, x3, x5, x7) = (x[1][l], x[3][l], x[5][l], x[7][l]);
        let odd = [
            x1 * H1 + x3 * H3 + x5 * H5 + x7 * H7,
            x1 * H3 - x3 * H7 - x5 * H1 - x7 * H5,
            x1 * H5 - x3 * H1 + x5 * H7 + x7 * H3,
            x1 * H7 - x3 * H5 + x5 * H3 - x7 * H1,
        ];
        for n in 0..4 {
            out[n][l] = even[n] + odd[n];
            out[7 - n][l] = even[n] - odd[n];
        }
    }
    out
}

fn transpose(m: &Rows) -> Rows {
    std::array::from_fn(|r| std::array::from_fn(|c| m[c][r]))
}

/// Inverse-transforms one block of dequantised coefficients (raster order)
/// into spatial values (raster order).
#[inline]
pub(crate) fn idct8x8(coefs: &[f32; 64], out: &mut [f32; 64]) {
    // Transposed input: the first pass then transforms rows (Fᵀ → (F·T)ᵀ),
    // the transpose turns that into F·T, and the second pass gives Tᵀ·F·T.
    let input: Rows = std::array::from_fn(|r| std::array::from_fn(|c| coefs[c * 8 + r]));
    let res = columns(&transpose(&columns(&input)));
    for (y, row) in res.iter().enumerate() {
        out[y * 8..y * 8 + 8].copy_from_slice(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Direct (non-separable) double-precision definition for comparison.
    fn reference(coefs: &[f32; 64]) -> [f64; 64] {
        let mut out = [0f64; 64];
        for y in 0..8 {
            for x in 0..8 {
                let mut s = 0f64;
                for v in 0..8 {
                    for u in 0..8 {
                        let cu = if u == 0 { 0.5f64.sqrt() } else { 1.0 };
                        let cv = if v == 0 { 0.5f64.sqrt() } else { 1.0 };
                        s += cu
                            * cv
                            * coefs[v * 8 + u] as f64
                            * (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).cos()
                            * (((2 * y + 1) * v) as f64 * std::f64::consts::PI / 16.0).cos();
                    }
                }
                out[y * 8 + x] = s / 4.0;
            }
        }
        out
    }

    #[test]
    fn dc_only_block_is_flat_at_one_eighth() {
        let mut c = [0f32; 64];
        c[0] = 800.0;
        let mut out = [0f32; 64];
        idct8x8(&c, &mut out);
        for v in out {
            assert!((v - 100.0).abs() < 1e-3, "{v}");
        }
    }

    #[test]
    fn matches_the_definition() {
        let mut seed = 0x1234_5678u32;
        for _ in 0..50 {
            let mut c = [0f32; 64];
            for v in c.iter_mut() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                if seed.is_multiple_of(3) {
                    *v = (seed % 8001) as f32 - 4000.0;
                }
            }
            let mut out = [0f32; 64];
            idct8x8(&c, &mut out);
            let want = reference(&c);
            for i in 0..64 {
                assert!(
                    (out[i] as f64 - want[i]).abs() < 0.02,
                    "{} vs {}",
                    out[i],
                    want[i]
                );
            }
        }
    }
}
