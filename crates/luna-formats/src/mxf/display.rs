//! Display strings for ClipMeta, in the forms the field docs promise.

/// One decimal, trailing ".0" dropped.
fn one_decimal(x: f64) -> String {
    let s = format!("{x:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// "180°", "172.8°".
pub(crate) fn angle(deg: f64) -> Option<String> {
    (deg.is_finite() && deg > 0.0 && deg <= 360.0).then(|| format!("{}°", one_decimal(deg)))
}

/// Shutter from exposure time and frame rate: an angle when it is one, else "1/x".
pub(crate) fn shutter(exposure_s: f64, fps: Option<f64>) -> Option<String> {
    if !(exposure_s.is_finite() && exposure_s > 0.0) {
        return None;
    }
    if let Some(fps) = fps.filter(|f| f.is_finite() && *f > 0.0)
        && let Some(a) = angle(360.0 * fps * exposure_s)
    {
        return Some(a);
    }
    Some(format!("1/{}", one_decimal(1.0 / exposure_s)))
}

/// "T2.8".
pub(crate) fn t_stop(t: f64) -> Option<String> {
    (t.is_finite() && t > 0.3 && t < 100.0).then(|| format!("T{}", one_decimal(t)))
}

/// "f/4".
pub(crate) fn f_stop(f: f64) -> Option<String> {
    (f.is_finite() && f > 0.3 && f < 100.0).then(|| format!("f/{}", one_decimal(f)))
}

/// "47 mm".
pub(crate) fn focal(mm: f64) -> Option<String> {
    (mm.is_finite() && mm > 0.0 && mm < 10_000.0).then(|| format!("{} mm", one_decimal(mm)))
}

/// "5600 K".
pub(crate) fn kelvin(k: u32) -> Option<String> {
    (1000..=20000).contains(&k).then(|| format!("{k} K"))
}

/// Optical density: "Clear", "0.6", "1.2".
pub(crate) fn nd_density(od: f64) -> Option<String> {
    if !od.is_finite() || od < 0.0 || od > 12.0 {
        return None;
    }
    Some(if od < 0.05 {
        "Clear".into()
    } else {
        one_decimal(od)
    })
}

/// RDD 18 ND wheel setting: the transmittance denominator (1 = clear,
/// 4 = 1/4 = ND 0.6).
pub(crate) fn nd_from_transmittance(n: u16) -> Option<String> {
    (n >= 1).then(|| nd_density((n as f64).log10())).flatten()
}

pub(crate) fn stamp(s: (u16, u8, u8, u8, u8, u8)) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        s.0, s.1, s.2, s.3, s.4, s.5
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms() {
        assert_eq!(shutter(1.0 / 50.0, Some(24.0)).as_deref(), Some("172.8°"));
        assert_eq!(shutter(1.0 / 48.0, Some(24.0)).as_deref(), Some("180°"));
        assert_eq!(shutter(1.0 / 50.0, None).as_deref(), Some("1/50"));
        assert_eq!(shutter(1.0, Some(24.0)).as_deref(), Some("1/1"));
        assert_eq!(t_stop(5.19).as_deref(), Some("T5.2"));
        assert_eq!(t_stop(2.0).as_deref(), Some("T2"));
        assert_eq!(focal(44.978).as_deref(), Some("45 mm"));
        assert_eq!(focal(24.5).as_deref(), Some("24.5 mm"));
        assert_eq!(kelvin(5600).as_deref(), Some("5600 K"));
        assert_eq!(nd_density(1.2).as_deref(), Some("1.2"));
        assert_eq!(nd_density(0.0).as_deref(), Some("Clear"));
        assert_eq!(nd_from_transmittance(4).as_deref(), Some("0.6"));
        assert_eq!(nd_from_transmittance(1).as_deref(), Some("Clear"));
        assert_eq!(nd_from_transmittance(0), None);
        assert_eq!(stamp((2025, 1, 7, 22, 40, 21)), "2025-01-07 22:40:21");
    }
}
