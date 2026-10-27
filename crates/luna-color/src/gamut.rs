//! Colour primaries and the 3×3 matrices between them.
//!
//! Matrices are built from each maker's published CIE xy primaries and white
//! point (no LUTs, no copied matrices), the SMPTE RP 177 way: normalised
//! primary matrix per space, then XYZ → Rec.709. When the white points differ
//! the XYZ step goes through a Bradford chromatic adaptation.

use luna_formats::Gamut;

pub type Mat3 = [[f64; 3]; 3];

/// CIE 1931 xy chromaticities of R, G, B and white.
#[derive(Debug, Clone, Copy)]
pub struct Primaries {
    pub r: (f64, f64),
    pub g: (f64, f64),
    pub b: (f64, f64),
    pub w: (f64, f64),
}

/// CIE D65 as written in ITU-R BT.709 and most camera papers.
const D65: (f64, f64) = (0.3127, 0.3290);

/// Published primaries for each gamut, with the document each comes from.
pub fn primaries(g: Gamut) -> Primaries {
    let p = |r, g, b, w| Primaries { r, g, b, w };
    match g {
        // ITU-R BT.709-6, table 1, items 1.3–1.4.
        Gamut::Rec709 => p((0.64, 0.33), (0.30, 0.60), (0.15, 0.06), D65),
        // ITU-R BT.2020-2, table 3.
        Gamut::Rec2020 => p((0.708, 0.292), (0.170, 0.797), (0.131, 0.046), D65),
        // ARRI, "ALEXA Log C Curve – Usage in VFX" (2017), ARRI Wide Gamut 3 primaries.
        Gamut::ArriWideGamut3 => p((0.6840, 0.3130), (0.2210, 0.8480), (0.0861, -0.1020), D65),
        // ARRI, "ARRI LogC4 Logarithmic Color Space – Specification" (2022), AWG4 primaries.
        Gamut::ArriWideGamut4 => p((0.7347, 0.2653), (0.1424, 0.8576), (0.0991, -0.0308), D65),
        // Sony, "Technical Summary for S-Gamut3.Cine/S-Log3 and S-Gamut3/S-Log3" (2014).
        Gamut::SGamut3 => p((0.730, 0.280), (0.140, 0.855), (0.100, -0.050), D65),
        // Same Sony technical summary, S-Gamut3.Cine primaries.
        Gamut::SGamut3Cine => p((0.766, 0.275), (0.225, 0.800), (0.089, -0.087), D65),
        // Canon, "Cinema Gamut" definition in the Canon Log gamma curves / Cinema EOS
        // colour white paper (Cinema Gamut primaries).
        Gamut::CinemaGamut => p((0.740, 0.270), (0.170, 1.140), (0.080, -0.100), D65),
        // Panasonic, "V-Log/V-Gamut Reference Manual" (2014), V-Gamut primaries.
        Gamut::VGamut => p((0.730, 0.280), (0.165, 0.840), (0.100, -0.030), D65),
        // DJI, "White Paper on D-Log and D-Gamut of DJI Cinema Color System" (2017).
        Gamut::DGamut => p((0.71, 0.31), (0.21, 0.88), (0.09, -0.08), D65),
        // Blackmagic Design, "Blackmagic Generation 5 Color Science" (2021),
        // Blackmagic Wide Gamut Gen 4/5 primaries; its white is a hair off D65.
        Gamut::BmdWideGamutGen5 => p(
            (0.717_722_3, 0.317_118_7),
            (0.228_041, 0.861_569),
            (0.100_584_1, -0.082_045_2),
            (0.312_717_1, 0.329_031_2),
        ),
        // RED, "White Paper on REDWideGamutRGB and Log3G10" (2017).
        Gamut::RedWideGamutRgb => p(
            (0.780_308, 0.304_253),
            (0.121_595, 1.493_994),
            (0.095_612, -0.084_589),
            D65,
        ),
    }
}

fn xy_to_xyz((x, y): (f64, f64)) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

pub fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

pub fn apply(m: &Mat3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn invert(m: &Mat3) -> Mat3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    let k = 1.0 / det;
    [
        [
            (e * i - f * h) * k,
            (c * h - b * i) * k,
            (b * f - c * e) * k,
        ],
        [
            (f * g - d * i) * k,
            (a * i - c * g) * k,
            (c * d - a * f) * k,
        ],
        [
            (d * h - e * g) * k,
            (b * g - a * h) * k,
            (a * e - b * d) * k,
        ],
    ]
}

/// Normalised primary matrix: linear RGB → CIE XYZ (SMPTE RP 177).
pub fn rgb_to_xyz(p: &Primaries) -> Mat3 {
    let (r, g, b) = (xy_to_xyz(p.r), xy_to_xyz(p.g), xy_to_xyz(p.b));
    let m = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let s = apply(&invert(&m), xy_to_xyz(p.w));
    let mut out = m;
    for row in out.iter_mut() {
        for (j, v) in row.iter_mut().enumerate() {
            *v *= s[j];
        }
    }
    out
}

/// Bradford chromatic adaptation from white `src` to white `dst` (XYZ → XYZ).
pub fn bradford(src: (f64, f64), dst: (f64, f64)) -> Mat3 {
    const B: Mat3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let s = apply(&B, xy_to_xyz(src));
    let d = apply(&B, xy_to_xyz(dst));
    let scale = [
        [d[0] / s[0], 0.0, 0.0],
        [0.0, d[1] / s[1], 0.0],
        [0.0, 0.0, d[2] / s[2]],
    ];
    mul(&invert(&B), &mul(&scale, &B))
}

fn same_white(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

/// XYZ (at white `w`) → linear Rec.709, adapting to D65 when `w` differs.
pub fn xyz_to_rec709(w: (f64, f64)) -> Mat3 {
    let rec = primaries(Gamut::Rec709);
    let to_rgb = invert(&rgb_to_xyz(&rec));
    if same_white(w, rec.w) {
        to_rgb
    } else {
        mul(&to_rgb, &bradford(w, rec.w))
    }
}

/// XYZ (at white `w`) → linear RGB in `g`, adapting to `g`'s white when `w` differs.
pub fn xyz_to(g: Gamut, w: (f64, f64)) -> Mat3 {
    let p = primaries(g);
    let to_rgb = invert(&rgb_to_xyz(&p));
    if same_white(w, p.w) {
        to_rgb
    } else {
        mul(&to_rgb, &bradford(w, p.w))
    }
}

/// Linear RGB in `from` → linear RGB in `to`, adapting white points.
pub fn convert(from: Gamut, to: Gamut) -> Mat3 {
    let p = primaries(from);
    mul(&xyz_to(to, p.w), &rgb_to_xyz(&p))
}

/// Linear RGB in `g` → linear Rec.709 (D65).
pub fn to_rec709(g: Gamut) -> Mat3 {
    let p = primaries(g);
    mul(&xyz_to_rec709(p.w), &rgb_to_xyz(&p))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Gamut; 11] = [
        Gamut::Rec709,
        Gamut::Rec2020,
        Gamut::ArriWideGamut3,
        Gamut::ArriWideGamut4,
        Gamut::SGamut3,
        Gamut::SGamut3Cine,
        Gamut::CinemaGamut,
        Gamut::VGamut,
        Gamut::DGamut,
        Gamut::BmdWideGamutGen5,
        Gamut::RedWideGamutRgb,
    ];

    fn assert_mat(a: &Mat3, b: &Mat3, tol: f64) {
        for i in 0..3 {
            for j in 0..3 {
                assert!((a[i][j] - b[i][j]).abs() <= tol, "{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn rec709_npm_matches_bt709() {
        let m = rgb_to_xyz(&primaries(Gamut::Rec709));
        let want = [
            [0.4124, 0.3576, 0.1805],
            [0.2126, 0.7152, 0.0722],
            [0.0193, 0.1192, 0.9505],
        ];
        assert_mat(&m, &want, 5e-4);
    }

    #[test]
    fn rec709_to_itself_is_identity() {
        let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_mat(&to_rec709(Gamut::Rec709), &id, 1e-12);
    }

    #[test]
    fn rec2020_to_rec709_matches_bt2087() {
        // ITU-R BT.2087 gives the inverse direction; this is the widely quoted
        // Rec.2020 → Rec.709 matrix derived from the same primaries.
        let want = [
            [1.6605, -0.5876, -0.0728],
            [-0.1246, 1.1329, -0.0083],
            [-0.0182, -0.1006, 1.1187],
        ];
        assert_mat(&to_rec709(Gamut::Rec2020), &want, 5e-4);
    }

    #[test]
    fn every_source_white_maps_to_rec709_white() {
        for g in ALL {
            let m = to_rec709(g);
            let w = apply(&m, [1.0, 1.0, 1.0]);
            for c in w {
                assert!((c - 1.0).abs() < 1e-9, "{g:?}: white → {w:?}");
            }
            // Wide gamuts pull saturated primaries outward: the diagonal dominates.
            for (i, row) in m.iter().enumerate() {
                assert!(row[i] >= 0.9, "{g:?}: diagonal {row:?}");
            }
        }
    }

    #[test]
    fn bradford_is_identity_for_the_same_white_and_maps_white_to_white() {
        let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_mat(&bradford(D65, D65), &id, 1e-12);
        let d50 = (0.3457, 0.3585);
        let out = apply(&bradford(d50, D65), xy_to_xyz(d50));
        let want = xy_to_xyz(D65);
        for i in 0..3 {
            assert!((out[i] - want[i]).abs() < 1e-9);
        }
    }
}
