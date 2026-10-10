//! 8×8 inverse DCT.
//!
//! RDD 36 defines the inverse transform mathematically (the orthonormal 2-D
//! DCT-III) rather than as a particular integer algorithm, so this is a
//! direct separable evaluation in single precision: accurate to far better
//! than one output code, which is what the specification asks for.

use std::sync::OnceLock;

/// `BASIS[u][x] = c(u)/2 · cos((2x+1)uπ/16)`, `c(0) = 1/√2`, else 1.
fn basis() -> &'static [[f32; 8]; 8] {
    static BASIS: OnceLock<[[f32; 8]; 8]> = OnceLock::new();
    BASIS.get_or_init(|| {
        let mut t = [[0f32; 8]; 8];
        for (u, row) in t.iter_mut().enumerate() {
            let c = if u == 0 {
                std::f64::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            for (x, v) in row.iter_mut().enumerate() {
                let angle = ((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0;
                *v = (c / 2.0 * angle.cos()) as f32;
            }
        }
        t
    })
}

/// Inverse-transforms one block of dequantised coefficients (raster order)
/// into spatial values (raster order).
#[inline]
pub(crate) fn idct8x8(coefs: &[f32; 64], out: &mut [f32; 64]) {
    let t = basis();
    // Rows: tmp[v][x] = Σ_u F[v][u] · T[u][x].
    let mut tmp = [[0f32; 8]; 8];
    let mut live_rows = 0u8;
    for v in 0..8 {
        let row = &coefs[v * 8..v * 8 + 8];
        let mut acc = [0f32; 8];
        let mut any = false;
        for u in 0..8 {
            let f = row[u];
            if f != 0.0 {
                any = true;
                for x in 0..8 {
                    acc[x] += f * t[u][x];
                }
            }
        }
        if any {
            live_rows |= 1 << v;
            tmp[v] = acc;
        }
    }
    // Columns: out[y][x] = Σ_v T[v][y] · tmp[v][x].
    let mut res = [[0f32; 8]; 8];
    for v in 0..8 {
        if live_rows & (1 << v) == 0 {
            continue;
        }
        let r = &tmp[v];
        for (y, out_row) in res.iter_mut().enumerate() {
            let w = t[v][y];
            for x in 0..8 {
                out_row[x] += w * r[x];
            }
        }
    }
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
