//! Scaling and inverse transforms (8.5.6 – 8.5.13).
//!
//! Scaled coefficients are clamped to ±2^22 (conforming streams stay within
//! ±2^(7+BitDepth), BitDepth ≤ 14) so damaged input cannot overflow the
//! transforms.

const LIM: i64 = 1 << 22;

#[inline]
fn mul_shl(c: i32, ls: i32, sh: i32) -> i32 {
    ((c as i64 * ls as i64) << sh).clamp(-LIM, LIM) as i32
}

#[inline]
fn mul_rnd_shr(c: i32, ls: i32, sh: i32) -> i32 {
    ((c as i64 * ls as i64 + (1i64 << (sh - 1))) >> sh).clamp(-LIM, LIM) as i32
}

/// Inverse 4×4 transform of `d` (row-major, already scaled), in place,
/// including the final (x + 32) >> 6.
#[inline]
pub(crate) fn idct4(d: &mut [i32; 16]) {
    for i in 0..4 {
        let r = &mut d[i * 4..i * 4 + 4];
        let e0 = r[0] + r[2];
        let e1 = r[0] - r[2];
        let e2 = (r[1] >> 1) - r[3];
        let e3 = r[1] + (r[3] >> 1);
        r[0] = e0 + e3;
        r[1] = e1 + e2;
        r[2] = e1 - e2;
        r[3] = e0 - e3;
    }
    for j in 0..4 {
        let (f0, f1, f2, f3) = (d[j], d[4 + j], d[8 + j], d[12 + j]);
        let g0 = f0 + f2;
        let g1 = f0 - f2;
        let g2 = (f1 >> 1) - f3;
        let g3 = f1 + (f3 >> 1);
        d[j] = (g0 + g3 + 32) >> 6;
        d[4 + j] = (g1 + g2 + 32) >> 6;
        d[8 + j] = (g1 - g2 + 32) >> 6;
        d[12 + j] = (g0 - g3 + 32) >> 6;
    }
}

#[inline]
fn idct8_1d(s: [i32; 8]) -> [i32; 8] {
    let a0 = s[0] + s[4];
    let a4 = s[0] - s[4];
    let a2 = (s[2] >> 1) - s[6];
    let a6 = s[2] + (s[6] >> 1);
    let b0 = a0 + a6;
    let b2 = a4 + a2;
    let b4 = a4 - a2;
    let b6 = a0 - a6;
    let a1 = -s[3] + s[5] - s[7] - (s[7] >> 1);
    let a3 = s[1] + s[7] - s[3] - (s[3] >> 1);
    let a5 = -s[1] + s[7] + s[5] + (s[5] >> 1);
    let a7 = s[3] + s[5] + s[1] + (s[1] >> 1);
    let b1 = a1 + (a7 >> 2);
    let b7 = a7 - (a1 >> 2);
    let b3 = a3 + (a5 >> 2);
    let b5 = (a3 >> 2) - a5;
    [
        b0 + b7,
        b2 + b5,
        b4 + b3,
        b6 + b1,
        b6 - b1,
        b4 - b3,
        b2 - b5,
        b0 - b7,
    ]
}

/// Inverse 8×8 transform, in place, including (x + 32) >> 6.
pub(crate) fn idct8(d: &mut [i32; 64]) {
    for i in 0..8 {
        let row: [i32; 8] = d[i * 8..i * 8 + 8].try_into().unwrap_or([0; 8]);
        d[i * 8..i * 8 + 8].copy_from_slice(&idct8_1d(row));
    }
    for j in 0..8 {
        let col = std::array::from_fn(|i| d[i * 8 + j]);
        let out = idct8_1d(col);
        for i in 0..8 {
            d[i * 8 + j] = (out[i] + 32) >> 6;
        }
    }
}

/// Scales 4×4 coefficients (8.5.12.1); `skip_dc` leaves d[0] (already set
/// from a DC transform) untouched. `ls` is LevelScale4x4(qP % 6, ·).
#[inline]
pub(crate) fn scale4(c: &mut [i32; 16], ls: &[i32; 16], qp: i32, skip_dc: bool) {
    let start = skip_dc as usize;
    if qp >= 24 {
        let sh = qp / 6 - 4;
        for k in start..16 {
            c[k] = mul_shl(c[k], ls[k], sh);
        }
    } else {
        let sh = 4 - qp / 6;
        for k in start..16 {
            c[k] = mul_rnd_shr(c[k], ls[k], sh);
        }
    }
}

/// Scales 8×8 coefficients (8.5.13.1).
pub(crate) fn scale8(c: &mut [i32; 64], ls: &[i32; 64], qp: i32) {
    if qp >= 36 {
        let sh = qp / 6 - 6;
        for k in 0..64 {
            c[k] = mul_shl(c[k], ls[k], sh);
        }
    } else {
        let sh = 6 - qp / 6;
        for k in 0..64 {
            c[k] = mul_rnd_shr(c[k], ls[k], sh);
        }
    }
}

/// Intra_16x16 luma DC: inverse Hadamard and scaling (8.5.10). `c` is the
/// 4×4 matrix of DC levels (row-major, after the inverse zig-zag).
pub(crate) fn luma_dc(c: &mut [i32; 16], ls0: i32, qp: i32) {
    hadamard4(c);
    if qp >= 36 {
        let sh = qp / 6 - 6;
        for v in c.iter_mut() {
            *v = mul_shl(*v, ls0, sh);
        }
    } else {
        let sh = 6 - qp / 6;
        for v in c.iter_mut() {
            *v = mul_rnd_shr(*v, ls0, sh);
        }
    }
}

fn hadamard4(c: &mut [i32; 16]) {
    for i in 0..4 {
        let r = &mut c[i * 4..i * 4 + 4];
        let (a, b, cc, d) = (r[0], r[1], r[2], r[3]);
        r[0] = a + b + cc + d;
        r[1] = a + b - cc - d;
        r[2] = a - b - cc + d;
        r[3] = a - b + cc - d;
    }
    for j in 0..4 {
        let (a, b, cc, d) = (c[j], c[4 + j], c[8 + j], c[12 + j]);
        c[j] = a + b + cc + d;
        c[4 + j] = a + b - cc - d;
        c[8 + j] = a - b - cc + d;
        c[12 + j] = a - b + cc - d;
    }
}

/// 4:2:0 chroma DC (8.5.11.2): `c` is [c0 c1; c2 c3]. Returns dcC by
/// chroma4x4BlkIdx.
pub(crate) fn chroma_dc420(c: &[i32], ls0: i32, qp: i32) -> [i32; 8] {
    let f = [
        c[0] + c[1] + c[2] + c[3],
        c[0] - c[1] + c[2] - c[3],
        c[0] + c[1] - c[2] - c[3],
        c[0] - c[1] - c[2] + c[3],
    ];
    let mut out = [0; 8];
    for k in 0..4 {
        out[k] = (((f[k] as i64 * ls0 as i64) << (qp / 6)) >> 5).clamp(-LIM, LIM) as i32;
    }
    out
}

/// 4:2:2 chroma DC (8.5.11.2): the 8 levels in coded order map to a 4×2
/// matrix [[c0 c2] [c1 c5] [c3 c6] [c4 c7]]; `qp_dc` is QP′c + 3.
pub(crate) fn chroma_dc422(c: &[i32], ls0: i32, qp_dc: i32) -> [i32; 8] {
    let m = [[c[0], c[2]], [c[1], c[5]], [c[3], c[6]], [c[4], c[7]]];
    // f = A(4x4) · m · [[1 1] [1 −1]]
    let mut g = [[0i32; 2]; 4];
    for (i, gi) in g.iter_mut().enumerate() {
        let (a, b) = (m[i][0], m[i][1]);
        *gi = [a + b, a - b];
    }
    let mut f = [[0i32; 2]; 4];
    for j in 0..2 {
        let (a, b, cc, d) = (g[0][j], g[1][j], g[2][j], g[3][j]);
        f[0][j] = a + b + cc + d;
        f[1][j] = a + b - cc - d;
        f[2][j] = a - b - cc + d;
        f[3][j] = a - b + cc - d;
    }
    let mut out = [0; 8];
    for i in 0..4 {
        for j in 0..2 {
            let v = f[i][j];
            out[i * 2 + j] = if qp_dc >= 36 {
                mul_shl(v, ls0, qp_dc / 6 - 6)
            } else {
                mul_rnd_shr(v, ls0, 6 - qp_dc / 6)
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_only_block_is_flat() {
        let mut d = [0; 16];
        d[0] = 64 * 5;
        idct4(&mut d);
        assert!(d.iter().all(|&v| v == 5));
        let mut d = [0; 64];
        d[0] = 64 * 7;
        idct8(&mut d);
        assert!(d.iter().all(|&v| v == 7));
    }

    #[test]
    fn hadamard_of_dc() {
        let mut c = [0; 16];
        c[0] = 1;
        hadamard4(&mut c);
        assert!(c.iter().all(|&v| v == 1));
    }
}
