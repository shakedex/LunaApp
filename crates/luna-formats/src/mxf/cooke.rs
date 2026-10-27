//! Lens names from Cooke /i serial numbers.
//!
//! Sony records cine lenses only by the serial the lens sends over /i
//! (`LensAttributes`). The serial's prefix names the lens family, as listed in
//! Cooke's /i Technology Protocol, Part I (2021), section 2.2: Cooke lenses are
//! `NFFF.xxxx` (`NFFF-xxxx` before /i²), N the family and FFF the focal length.

use crate::types::Rational;

/// The lens name for a /i serial, or `None` when the prefix isn't listed.
/// `pixel_aspect` separates Cooke's two `7` families: the Anamorphic/i Full
/// Frame Plus primes squeeze 1.8×, the S7/i primes are spherical.
pub(crate) fn lens_name(serial: &str, pixel_aspect: Option<Rational>) -> Option<String> {
    if let Some(name) = listed(serial) {
        return Some(name.to_string());
    }
    let b = serial.as_bytes();
    let cooke = b.len() == 9
        && matches!(b[4], b'.' | b'-')
        && b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit);
    if !cooke {
        return None;
    }
    let focal = serial[1..4].parse::<u32>().ok().filter(|&f| f > 0)?;
    let family = match b[0] {
        b'3' => "Panchro/i Classic",
        b'4' => "S4/i",
        b'5' => "5/i",
        b'7' if pixel_aspect.is_some_and(|r| (r.as_f64() - 1.8).abs() < 0.01) => {
            "Anamorphic/i Full Frame Plus"
        }
        b'7' => "S7/i",
        b'8' => "miniS4/i",
        b'9' => match focal {
            345 => return Some("Cooke Anamorphic/i 35-140mm".into()),
            459 => return Some("Cooke Anamorphic/i 45-450mm".into()),
            _ => "Anamorphic/i",
        },
        _ => return None,
    };
    Some(format!("Cooke {family} {focal}mm"))
}

/// Lenses the table lists by a fixed serial prefix.
fn listed(serial: &str) -> Option<&'static str> {
    const PREFIXES: &[(&str, &str)] = &[
        ("F0700", "Fujinon 19-90mm"),
        ("F0701", "Fujinon 85-300mm"),
        ("S01P", "Sony F3 35mm"),
        ("S02P", "Sony F3 50mm"),
        ("S03P", "Sony F3 85mm"),
        ("S04Z", "Sony F3 11-16mm"),
        ("S05Z", "Sony F3 18-252mm"),
        ("AA", "Angenieux Optimo 15-40mm"),
        ("AB", "Angenieux Optimo 28-76mm"),
        ("AC", "Angenieux Optimo 45-120mm"),
        ("AD", "Angenieux Optimo DP 16-42mm"),
        ("AE", "Angenieux Optimo DP 30-80mm"),
        ("AF", "Angenieux Optimo 17-80mm"),
        ("AG", "Angenieux Optimo 24-290mm"),
        ("AH", "Angenieux Optimo Style 25-250mm"),
        ("AI", "Angenieux Optimo Style 16-40mm"),
        ("AJ", "Angenieux Optimo Style 30-76mm"),
        ("AK", "Angenieux Optimo 30-72 A2S"),
        ("AL", "Angenieux Optimo 56-152 A2S"),
        ("AM", "Angenieux Optimo 28-340mm"),
        ("AN", "Angenieux Optimo 19.5-94mm"),
        ("AO", "Angenieux Optimo 44-440 A2S"),
        ("AP", "Angenieux 42-425 Anamorphic"),
        ("AQ", "Angenieux 42-420 A2S"),
    ];
    // Every listed serial is nine characters.
    if serial.len() != 9 {
        return None;
    }
    PREFIXES
        .iter()
        .find(|(p, _)| serial.starts_with(p))
        .map(|&(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUEEZE_18: Option<Rational> = Some(Rational { num: 9, den: 5 });
    const SPHERICAL: Option<Rational> = Some(Rational { num: 1, den: 1 });

    #[test]
    fn cooke_families() {
        let n = |s, pa| lens_name(s, pa);
        assert_eq!(n("4050.0093", None).as_deref(), Some("Cooke S4/i 50mm"));
        assert_eq!(n("4025-1234", None).as_deref(), Some("Cooke S4/i 25mm"));
        assert_eq!(n("8025.1234", None).as_deref(), Some("Cooke miniS4/i 25mm"));
        assert_eq!(
            n("3025.1234", None).as_deref(),
            Some("Cooke Panchro/i Classic 25mm")
        );
        assert_eq!(
            n("9459.1234", None).as_deref(),
            Some("Cooke Anamorphic/i 45-450mm")
        );
        assert_eq!(
            n("9040.1234", None).as_deref(),
            Some("Cooke Anamorphic/i 40mm")
        );
    }

    #[test]
    fn seven_split_by_squeeze() {
        // VENICE 2 "8K 1.8x Anamorphic" corpus clip.
        assert_eq!(
            lens_name("7085.2501", SQUEEZE_18).as_deref(),
            Some("Cooke Anamorphic/i Full Frame Plus 85mm")
        );
        assert_eq!(
            lens_name("7085.2501", SPHERICAL).as_deref(),
            Some("Cooke S7/i 85mm")
        );
    }

    #[test]
    fn listed_prefixes() {
        assert_eq!(
            lens_name("F07001234", None).as_deref(),
            Some("Fujinon 19-90mm")
        );
        assert_eq!(
            lens_name("AG1234567", None).as_deref(),
            Some("Angenieux Optimo 24-290mm")
        );
        assert_eq!(
            lens_name("S03P00001", None).as_deref(),
            Some("Sony F3 85mm")
        );
    }

    #[test]
    fn unlisted_stay_unnamed() {
        // BURANO E-mount codes and a Fujinon prefix the table doesn't list.
        for s in [
            "2050.0212",
            "F08000183",
            "Unknown",
            "7085",
            "7000.0001",
            "A1",
        ] {
            assert_eq!(lens_name(s, None), None, "{s}");
        }
    }
}
