//! Camera transfer functions: code value ↔ scene-linear light.
//!
//! Every log curve here is written from the maker's own published formula, not
//! from LUT files. Code values are full-range fractions of the file's code
//! range (`cv / (2^bits − 1)`, so 10-bit code 420 is `420 / 1023`), which is how
//! ARRI, Sony, Panasonic, DJI, Blackmagic and RED state their curves. Canon
//! states its curves in IRE (`(cv − 64) / 876` at 10 bits); we convert at the
//! boundary. Scene-linear is scaled so an 18% grey card is `0.18`.
//!
//! Maths is in `f64`; the renderer samples these into tables, so speed here
//! does not matter.

use luna_formats::{Gamut, Transfer};

/// Code value (full-range fraction) → scene-linear, 18% grey = 0.18.
///
/// `Rec709` uses the inverse BT.709 OETF and `Linear` is the identity; the
/// renderer never routes display-referred `Rec709` pictures through here, but
/// the mapping is defined so every transfer has one.
pub fn decode(t: Transfer, code: f64) -> f64 {
    match t {
        Transfer::LogC3 => logc3::decode(code),
        Transfer::LogC4 => logc4::decode(code),
        Transfer::SLog3 => slog3::decode(code),
        Transfer::CanonLog2 => canon_log2::decode(code),
        Transfer::CanonLog3 => canon_log3::decode(code),
        Transfer::VLog => vlog::decode(code),
        Transfer::DLog => dlog::decode(code),
        Transfer::BmdFilmGen5 => bmd_gen5::decode(code),
        Transfer::Log3G10 => log3g10::decode(code),
        Transfer::Rec709 => rec709_inverse_oetf(code),
        Transfer::Hlg => hlg_inverse_oetf(code) * HLG_TO_SCENE,
        Transfer::Pq => pq_eotf_nits(code) * PQ_NITS_TO_SCENE,
        Transfer::Linear => code,
    }
}

/// Scene-linear (18% grey = 0.18) → code value; the inverse of [`decode`].
pub fn encode(t: Transfer, x: f64) -> f64 {
    match t {
        Transfer::LogC3 => logc3::encode(x),
        Transfer::LogC4 => logc4::encode(x),
        Transfer::SLog3 => slog3::encode(x),
        Transfer::CanonLog2 => canon_log2::encode(x),
        Transfer::CanonLog3 => canon_log3::encode(x),
        Transfer::VLog => vlog::encode(x),
        Transfer::DLog => dlog::encode(x),
        Transfer::BmdFilmGen5 => bmd_gen5::encode(x),
        Transfer::Log3G10 => log3g10::encode(x),
        Transfer::Rec709 => rec709_oetf(x),
        Transfer::Hlg => hlg_oetf(x / HLG_TO_SCENE),
        Transfer::Pq => pq_inverse_eotf(x / PQ_NITS_TO_SCENE),
        Transfer::Linear => x,
    }
}

/// The gamut a camera log curve is recorded in; `None` for display and
/// linear transfers.
pub fn native_gamut(t: Transfer) -> Option<Gamut> {
    t.native_gamut()
}

/// The log curve a camera gamut is recorded with (Canon: Canon Log 2, the
/// curve Canon develops its raw to); `None` for display gamuts.
pub fn native_log(g: Gamut) -> Option<Transfer> {
    Some(match g {
        Gamut::ArriWideGamut3 => Transfer::LogC3,
        Gamut::ArriWideGamut4 => Transfer::LogC4,
        Gamut::SGamut3 | Gamut::SGamut3Cine => Transfer::SLog3,
        Gamut::CinemaGamut => Transfer::CanonLog2,
        Gamut::VGamut => Transfer::VLog,
        Gamut::DGamut => Transfer::DLog,
        Gamut::BmdWideGamutGen5 => Transfer::BmdFilmGen5,
        Gamut::RedWideGamutRgb => Transfer::Log3G10,
        Gamut::Rec709 | Gamut::Rec2020 => return None,
    })
}

/// The scene-linear value a code value stands for; the renderer's entry point.
pub fn to_scene_linear(t: Transfer, code: f64) -> f64 {
    decode(t, code)
}

/// ARRI LogC3 at EI 800 (the reference EI, used whatever EI was shot).
/// Source: ARRI, "ALEXA Log C Curve – Usage in VFX" (2017), table of EI 800
/// parameters for "LogC (scene linear)".
mod logc3 {
    const CUT: f64 = 0.010591;
    const A: f64 = 5.555556;
    const B: f64 = 0.052272;
    const C: f64 = 0.247190;
    const D: f64 = 0.385537;
    const E: f64 = 5.367655;
    const F: f64 = 0.092809;

    pub fn encode(x: f64) -> f64 {
        if x > CUT {
            C * (A * x + B).log10() + D
        } else {
            E * x + F
        }
    }

    pub fn decode(t: f64) -> f64 {
        if t > E * CUT + F {
            (10f64.powf((t - D) / C) - B) / A
        } else {
            (t - F) / E
        }
    }
}

/// ARRI LogC3 at the exposure index the clip was shot at: scene-linear
/// (18% grey = 0.18) → code value.
///
/// Source: ARRI, "ALEXA Log C Curve – Usage in VFX" (2017), table of
/// parameters for conversion between Log C values and exposure values, EI 160
/// to 1600. Checked against ARRI Reference Tool's LogC3 renders of an ALEXA
/// Mini LF clip: within 0.6 of 65535 codes at every EI in the table.
///
/// Other EIs use the table's parameters fitted as quadratics in
/// `log2(EI / 800)` (every parameter is smooth in it; residuals ≤ 2.4e-5).
/// ARRI publishes no formula above EI 1600, where the camera adds a highlight
/// roll-off; the fit matches ARRI Reference Tool at EI 2000, 2560 and 3200
/// within 3 codes of 65535 up to Log C 0.8, and runs up to 0.4 % high above
/// 0.85, where the roll-off is.
pub fn encode_logc3_at(ei: u32, x: f64) -> f64 {
    let [cut, a, b, c, d, e, f] = logc3_ei::params(ei);
    if x > cut {
        c * (a * x + b).log10() + d
    } else {
        e * x + f
    }
}

mod logc3_ei {
    /// EI → cut, a, b, c, d, e, f, as ARRI prints them.
    #[rustfmt::skip]
    pub(super) const TABLE: [(u32, [f64; 7]); 11] = [
        (160, [0.005561, 5.555556, 0.080216, 0.269036, 0.381991, 5.842037, 0.092778]),
        (200, [0.006208, 5.555556, 0.076621, 0.266007, 0.382478, 5.776265, 0.092782]),
        (250, [0.006871, 5.555556, 0.072941, 0.262978, 0.382966, 5.710494, 0.092786]),
        (320, [0.007622, 5.555556, 0.068768, 0.259627, 0.383508, 5.637732, 0.092791]),
        (400, [0.008318, 5.555556, 0.064901, 0.256598, 0.383999, 5.571960, 0.092795]),
        (500, [0.009031, 5.555556, 0.060939, 0.253569, 0.384493, 5.506188, 0.092800]),
        (640, [0.009840, 5.555556, 0.056443, 0.250219, 0.385040, 5.433426, 0.092805]),
        (800, [0.010591, 5.555556, 0.052272, 0.247190, 0.385537, 5.367655, 0.092809]),
        (1000, [0.011361, 5.555556, 0.047996, 0.244161, 0.386036, 5.301883, 0.092814]),
        (1280, [0.012235, 5.555556, 0.043137, 0.240810, 0.386590, 5.229121, 0.092819]),
        (1600, [0.013047, 5.555556, 0.038625, 0.237781, 0.387093, 5.163350, 0.092824]),
    ];

    /// Each parameter as `q·x² + l·x + k` with `x = log2(EI / 800)`, least
    /// squares over [`TABLE`] (`a` is constant).
    pub(super) const FIT: [[f64; 3]; 7] = [
        [8.503813826e-05, 2.363413979e-03, 1.059428719e-02],
        [0.0, 0.0, 5.555556],
        [-4.732560796e-04, -1.313173140e-02, 5.225394378e-02],
        [4.723145827e-08, -9.408526274e-03, 2.471896946e-01],
        [8.423566092e-06, 1.546691320e-03, 3.855374431e-01],
        [1.125101059e-07, -2.043050951e-01, 5.367654715e+00],
        [3.477833976e-07, 1.432503570e-05, 9.280926783e-02],
    ];

    pub fn params(ei: u32) -> [f64; 7] {
        if let Some((_, p)) = TABLE.iter().find(|(e, _)| *e == ei) {
            return *p;
        }
        // ARRI cameras offer EI 160 to 3200.
        let x = (ei.clamp(160, 3200) as f64 / 800.0).log2();
        FIT.map(|[q, l, k]| (q * x + l) * x + k)
    }
}

/// ARRI LogC4 (ALEXA 35).
/// Source: ARRI, "ARRI LogC4 Logarithmic Color Space – Specification" (2022),
/// encoding and decoding functions.
mod logc4 {
    const A: f64 = (262_144.0 - 16.0) / 117.45;
    const B: f64 = (1023.0 - 95.0) / 1023.0;
    const C: f64 = 95.0 / 1023.0;

    fn s() -> f64 {
        (7.0 * std::f64::consts::LN_2 * 2f64.powf(7.0 - 14.0 * C / B)) / (A * B)
    }

    fn t() -> f64 {
        (2f64.powf(14.0 * (-C / B) + 6.0) - 64.0) / A
    }

    pub fn encode(x: f64) -> f64 {
        if x >= t() {
            ((A * x + 64.0).log2() - 6.0) / 14.0 * B + C
        } else {
            (x - t()) / s()
        }
    }

    pub fn decode(e: f64) -> f64 {
        if e >= 0.0 {
            (2f64.powf(14.0 * (e - C) / B + 6.0) - 64.0) / A
        } else {
            e * s() + t()
        }
    }
}

/// Sony S-Log3.
/// Source: Sony, "Technical Summary for S-Gamut3.Cine/S-Log3 and
/// S-Gamut3/S-Log3" (2014), S-Log3 formulae (10-bit code values / 1023).
mod slog3 {
    const TOE: f64 = 171.210_294_692_9;

    pub fn encode(x: f64) -> f64 {
        if x >= 0.011_25 {
            (420.0 + ((x + 0.01) / (0.18 + 0.01)).log10() * 261.5) / 1023.0
        } else {
            (x * (TOE - 95.0) / 0.011_25 + 95.0) / 1023.0
        }
    }

    pub fn decode(y: f64) -> f64 {
        if y >= TOE / 1023.0 {
            10f64.powf((y * 1023.0 - 420.0) / 261.5) * (0.18 + 0.01) - 0.01
        } else {
            (y * 1023.0 - 95.0) * 0.011_25 / (TOE - 95.0)
        }
    }
}

/// Canon states its curves in IRE (legal range, 10-bit 64 → 0, 940 → 1) with
/// linear input as reflectance / 0.9. These convert at the boundary.
fn code_to_ire(code: f64) -> f64 {
    (code * 1023.0 - 64.0) / 876.0
}

fn ire_to_code(ire: f64) -> f64 {
    (ire * 876.0 + 64.0) / 1023.0
}

/// Canon Log 2.
/// Source: Canon, "Canon Log Gamma Curves – Description of the Canon Log,
/// Canon Log 2 and Canon Log 3" white paper (2018), Canon Log 2 formulae.
mod canon_log2 {
    use super::{code_to_ire, ire_to_code};

    const A: f64 = 0.241_360_77;
    const B: f64 = 87.099_375_46;
    const C: f64 = 0.092_864_125;

    pub fn encode(lin: f64) -> f64 {
        let x = lin / 0.9;
        let ire = if x < 0.0 {
            -A * (-x * B + 1.0).log10() + C
        } else {
            A * (x * B + 1.0).log10() + C
        };
        ire_to_code(ire)
    }

    pub fn decode(code: f64) -> f64 {
        let y = code_to_ire(code);
        let x = if y < C {
            -(10f64.powf((C - y) / A) - 1.0) / B
        } else {
            (10f64.powf((y - C) / A) - 1.0) / B
        };
        x * 0.9
    }
}

/// Canon Log 3.
/// Source: Canon, "Canon Log Gamma Curves – Description of the Canon Log,
/// Canon Log 2 and Canon Log 3" white paper (2018), Canon Log 3 formulae.
mod canon_log3 {
    use super::{code_to_ire, ire_to_code};

    const A: f64 = 0.367_268_45;
    const B: f64 = 14.983_25;
    const SLOPE: f64 = 1.975_479_8;
    const MID: f64 = 0.125_122_19;
    const HI: f64 = 0.122_405_37;
    const LO: f64 = 0.127_839_01;

    pub fn encode(lin: f64) -> f64 {
        let x = lin / 0.9;
        let ire = if x < -0.014 {
            -A * (-x * B + 1.0).log10() + LO
        } else if x <= 0.014 {
            SLOPE * x + MID
        } else {
            A * (x * B + 1.0).log10() + HI
        };
        ire_to_code(ire)
    }

    pub fn decode(code: f64) -> f64 {
        let y = code_to_ire(code);
        let x = if y < 0.097_465_473 {
            -(10f64.powf((LO - y) / A) - 1.0) / B
        } else if y <= 0.152_778_91 {
            (y - MID) / SLOPE
        } else {
            (10f64.powf((y - HI) / A) - 1.0) / B
        };
        x * 0.9
    }
}

/// Panasonic V-Log.
/// Source: Panasonic, "V-Log/V-Gamut Reference Manual" (2014), V-Log formulae.
mod vlog {
    const CUT1: f64 = 0.01;
    const CUT2: f64 = 0.181;
    const B: f64 = 0.008_73;
    const C: f64 = 0.241_514;
    const D: f64 = 0.598_206;

    pub fn encode(x: f64) -> f64 {
        if x < CUT1 {
            5.6 * x + 0.125
        } else {
            C * (x + B).log10() + D
        }
    }

    pub fn decode(y: f64) -> f64 {
        if y < CUT2 {
            (y - 0.125) / 5.6
        } else {
            10f64.powf((y - D) / C) - B
        }
    }
}

/// DJI D-Log.
/// Source: DJI, "White Paper on D-Log and D-Gamut of DJI Cinema Color System"
/// (2017), D-Log curve formulae.
mod dlog {
    pub fn encode(x: f64) -> f64 {
        if x <= 0.0078 {
            6.025 * x + 0.0929
        } else {
            (x * 0.9892 + 0.0108).log10() * 0.256_663 + 0.584_555
        }
    }

    pub fn decode(y: f64) -> f64 {
        if y <= 0.14 {
            (y - 0.0929) / 6.025
        } else {
            // The paper prints 10^(3.89616·y − 2.27752), which is
            // (y − 0.584555) / 0.256663 rounded; the exact form round-trips.
            (10f64.powf((y - 0.584_555) / 0.256_663) - 0.0108) / 0.9892
        }
    }
}

/// Blackmagic Film Generation 5.
/// Source: Blackmagic Design, "Blackmagic Generation 5 Color Science"
/// white paper (2021), Blackmagic Film Generation 5 OETF.
mod bmd_gen5 {
    const A: f64 = 0.086_928_760_654_912_24;
    const B: f64 = 0.005_494_072_432_257_808;
    const C: f64 = 0.530_013_339_229_193_9;
    const D: f64 = 8.283_605_932_402_494;
    const E: f64 = 0.092_465_753_424_657_53;
    const LIN_CUT: f64 = 0.005;

    pub fn encode(x: f64) -> f64 {
        if x < LIN_CUT {
            D * x + E
        } else {
            A * (x + B).ln() + C
        }
    }

    pub fn decode(y: f64) -> f64 {
        if y < D * LIN_CUT + E {
            (y - E) / D
        } else {
            ((y - C) / A).exp() - B
        }
    }
}

/// RED Log3G10 (the current version, with the 0.01 black offset).
/// Source: RED, "White Paper on REDWideGamutRGB and Log3G10" (2017),
/// Log3G10 equations.
mod log3g10 {
    const A: f64 = 0.224_282;
    const B: f64 = 155.975_327;
    const C: f64 = 0.01;
    const G: f64 = 15.1927;

    pub fn encode(x: f64) -> f64 {
        let x = x + C;
        if x < 0.0 {
            x * G
        } else {
            A * (x * B + 1.0).log10()
        }
    }

    pub fn decode(y: f64) -> f64 {
        if y < 0.0 {
            y / G - C
        } else {
            (10f64.powf(y / A) - 1.0) / B - C
        }
    }
}

/// ITU-R BT.709 OETF (scene linear → signal), for completeness.
fn rec709_oetf(x: f64) -> f64 {
    if x < 0.018 {
        4.5 * x
    } else {
        1.099 * x.powf(0.45) - 0.099
    }
}

fn rec709_inverse_oetf(v: f64) -> f64 {
    if v < 0.081 {
        v / 4.5
    } else {
        ((v + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

/// HLG scene-light scale: ITU-R BT.2408 puts an 18% grey card at 38% HLG
/// signal, so scene-linear = HLG scene light × (0.18 / E(0.38)).
const HLG_TO_SCENE: f64 = 0.18 / (0.38 * 0.38 / 3.0);

const HLG_A: f64 = 0.178_832_77;
const HLG_B: f64 = 0.284_668_92;
const HLG_C: f64 = 0.559_910_73;

/// ITU-R BT.2100 HLG inverse OETF: signal E′ → normalised scene light E (0..1).
pub fn hlg_inverse_oetf(e: f64) -> f64 {
    if e <= 0.5 {
        e * e / 3.0
    } else {
        (((e - HLG_C) / HLG_A).exp() + HLG_B) / 12.0
    }
}

fn hlg_oetf(e: f64) -> f64 {
    if e <= 1.0 / 12.0 {
        (3.0 * e.max(0.0)).sqrt()
    } else {
        HLG_A * (12.0 * e - HLG_B).ln() + HLG_C
    }
}

/// PQ to scene: ITU-R BT.2408 puts an 18% grey card at 26 cd/m², so PQ light
/// is treated as scene light scaled by 0.18 / 26. Simple, and it lines HDR grey
/// up with every other camera's grey before the shared SDR rendering.
const PQ_NITS_TO_SCENE: f64 = 0.18 / 26.0;

const PQ_M1: f64 = 2610.0 / 16384.0;
const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f64 = 3424.0 / 4096.0;
const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

/// SMPTE ST 2084 EOTF: signal → absolute luminance in cd/m².
pub fn pq_eotf_nits(e: f64) -> f64 {
    let p = e.clamp(0.0, 1.0).powf(1.0 / PQ_M2);
    let num = (p - PQ_C1).max(0.0);
    let den = PQ_C2 - PQ_C3 * p;
    10_000.0 * (num / den).powf(1.0 / PQ_M1)
}

fn pq_inverse_eotf(nits: f64) -> f64 {
    let y = (nits / 10_000.0).max(0.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * y) / (1.0 + PQ_C3 * y)).powf(PQ_M2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOGS: [Transfer; 9] = [
        Transfer::LogC3,
        Transfer::LogC4,
        Transfer::SLog3,
        Transfer::CanonLog2,
        Transfer::CanonLog3,
        Transfer::VLog,
        Transfer::DLog,
        Transfer::BmdFilmGen5,
        Transfer::Log3G10,
    ];

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn eighteen_percent_grey_matches_the_white_papers() {
        // Code values as full-range fractions of the file's code range (cv / 1023).
        let expected = [
            (Transfer::LogC3, 0.3910, 0.0005), // ARRI: 18% grey ≈ 400/1023 at EI 800
            (Transfer::LogC4, 0.2784, 0.0005), // ARRI LogC4 spec: 18% grey ≈ 0.278
            (Transfer::SLog3, 420.0 / 1023.0, 1e-6), // Sony: 18% grey at 10-bit 420
            (
                Transfer::CanonLog2,
                (64.0 + 0.39825 * 876.0) / 1023.0,
                0.0005,
            ), // 39.8% IRE
            (
                Transfer::CanonLog3,
                (64.0 + 0.34339 * 876.0) / 1023.0,
                0.0005,
            ), // 34.3% IRE
            (Transfer::VLog, 0.4233, 0.0005),  // Panasonic: 18% grey ≈ 433/1023
            (Transfer::DLog, 0.3988, 0.0005),  // DJI D-Log: 18% grey ≈ 40%
            (Transfer::BmdFilmGen5, 0.3836, 0.0005), // Blackmagic Gen 5: 18% grey ≈ 38.4%
            (Transfer::Log3G10, 1.0 / 3.0, 0.0002), // RED: 18% grey at 1/3
        ];
        for (t, code, tol) in expected {
            let got = encode(t, 0.18);
            assert!(close(got, code, tol), "{t:?}: {got} vs {code}");
        }
    }

    #[test]
    fn log_curves_round_trip() {
        for t in LOGS {
            for stops in -8..=8 {
                let x = 0.18 * 2f64.powi(stops);
                let back = decode(t, encode(t, x));
                assert!(close(back, x, x * 1e-9 + 1e-12), "{t:?} at {stops}: {back}");
            }
            // Linear toe segments too.
            for x in [0.0, 0.001, 0.004] {
                let back = decode(t, encode(t, x));
                assert!(close(back, x, 1e-9), "{t:?} toe at {x}: {back}");
            }
        }
    }

    #[test]
    fn log_curves_are_monotonic() {
        for t in LOGS {
            let mut prev = f64::NEG_INFINITY;
            for i in 0..=1023 {
                let v = decode(t, i as f64 / 1023.0);
                assert!(v > prev, "{t:?} not increasing at code {i}");
                prev = v;
            }
        }
    }

    #[test]
    fn logc3_black_and_known_points() {
        // EI 800 parameters: linear 0 encodes to f = 0.092809.
        assert!(close(encode(Transfer::LogC3, 0.0), 0.092809, 1e-6));
    }

    const ARRI_EIS: [u32; 14] = [
        160, 200, 250, 320, 400, 500, 640, 800, 1000, 1280, 1600, 2000, 2560, 3200,
    ];

    #[test]
    fn logc3_at_ei_800_is_the_reference_curve() {
        for x in [0.0, 0.001, 0.01, 0.18, 1.0, 10.0] {
            assert_eq!(encode_logc3_at(800, x), encode(Transfer::LogC3, x), "{x}");
        }
    }

    #[test]
    fn logc3_at_every_ei_keeps_grey_and_black() {
        // ARRI: every EI maps 18% grey to Log C 0.391 (400/1023), and black to
        // 0.0928.
        for ei in ARRI_EIS {
            assert!(
                close(encode_logc3_at(ei, 0.18), 400.0 / 1023.0, 1e-3),
                "EI {ei} grey"
            );
            assert!(
                close(encode_logc3_at(ei, 0.0), 0.0928, 1e-4),
                "EI {ei} black"
            );
        }
    }

    #[test]
    fn logc3_at_higher_ei_holds_more_highlight() {
        // Lower EI: the same exposure sits higher, so whites reach clip sooner.
        let white = |ei| encode_logc3_at(ei, 4.0);
        for pair in ARRI_EIS.windows(2) {
            assert!(white(pair[0]) > white(pair[1]), "EI {pair:?}");
        }
    }

    #[test]
    fn logc3_fit_matches_arris_table() {
        // EIs between ARRI's rows use the fit; it lands on the rows themselves
        // within a few 16-bit codes.
        for (ei, _) in logc3_ei::TABLE {
            let fit = |x: f64| {
                let lx = (ei as f64 / 800.0).log2();
                let p = logc3_ei::FIT.map(|[q, l, k]| (q * lx + l) * lx + k);
                if x > p[0] {
                    p[3] * (p[1] * x + p[2]).log10() + p[4]
                } else {
                    p[5] * x + p[6]
                }
            };
            for x in [0.001, 0.05, 0.18, 1.0, 8.0] {
                let d = (fit(x) - encode_logc3_at(ei, x)).abs() * 65535.0;
                assert!(d < 4.0, "EI {ei} at {x}: {d} codes");
            }
        }
    }

    #[test]
    fn hlg_inverse_oetf_matches_bt2100() {
        // BT.2100: E' = 0.5 at E = 1/12; E' = 1.0 at E = 1.
        assert!(close(hlg_inverse_oetf(0.5), 1.0 / 12.0, 1e-9));
        assert!(close(hlg_inverse_oetf(1.0), 1.0, 1e-6));
    }

    #[test]
    fn pq_eotf_matches_st2084() {
        assert!(close(pq_eotf_nits(1.0), 10000.0, 1e-6));
        assert!(close(pq_eotf_nits(0.0), 0.0, 1e-9));
        // 100 nits sits at about 0.508 in PQ.
        assert!(close(pq_eotf_nits(0.508_078), 100.0, 0.05));
    }

    #[test]
    fn hdr_grey_anchors_land_on_scene_grey() {
        // BT.2408: an 18% grey card is at 38% HLG and 26 nits PQ.
        assert!(close(to_scene_linear(Transfer::Hlg, 0.38), 0.18, 1e-6));
        let pq_26 = 0.380_032; // ST 2084 code for 26 nits
        assert!(close(to_scene_linear(Transfer::Pq, pq_26), 0.18, 1e-4));
    }
}
