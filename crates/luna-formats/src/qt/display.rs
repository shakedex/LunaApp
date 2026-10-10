//! Turning recorded values into the report's display strings.

/// Seconds since 1904-01-01 (QuickTime epoch) → "YYYY-MM-DD HH:MM:SS".
/// Zero means "not set" in QuickTime and yields `None`.
pub(crate) fn qt_time(secs: u64) -> Option<String> {
    if secs == 0 {
        return None;
    }
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // 1904-01-01 is 24107 days before 1970-01-01.
    let (y, m, d) = civil_from_days(days - 24_107);
    if !(1904..=9999).contains(&y) {
        return None;
    }
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    ))
}

/// Howard Hinnant's days-to-civil algorithm (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn valid_date(y: &str, m: &str, d: &str) -> bool {
    digits(y)
        && digits(m)
        && digits(d)
        && y.len() == 4
        && m.len() == 2
        && d.len() == 2
        && (1..=12).contains(&m.parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&d.parse::<u32>().unwrap_or(0))
}

fn valid_time(h: &str, mi: &str, s: &str) -> bool {
    [h, mi, s].iter().all(|p| p.len() == 2 && digits(p))
        && h.parse::<u32>().unwrap_or(99) < 24
        && mi.parse::<u32>().unwrap_or(99) < 60
        && s.parse::<u32>().unwrap_or(99) < 61
}

/// ISO 8601 local date-time as written ("2022-04-19T22:47:20-05:00", with or
/// without seconds fraction/zone, or a bare date) → "YYYY-MM-DD[ HH:MM:SS]".
/// The wall-clock part is kept as recorded; the zone is dropped, not applied.
pub(crate) fn iso_local(s: &str) -> Option<String> {
    let s = s.trim();
    let date = s.get(0..10)?;
    let (y, m, d) = (date.get(0..4)?, date.get(5..7)?, date.get(8..10)?);
    if date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' || !valid_date(y, m, d) {
        return None;
    }
    let rest = &s[10..];
    if rest.is_empty() {
        return Some(date.to_string());
    }
    if !rest.starts_with('T') && !rest.starts_with(' ') {
        return None;
    }
    let t = rest.get(1..9)?;
    let (h, mi, sec) = (t.get(0..2)?, t.get(3..5)?, t.get(6..8)?);
    if t.as_bytes()[2] != b':' || t.as_bytes()[5] != b':' || !valid_time(h, mi, sec) {
        return Some(date.to_string());
    }
    Some(format!("{date} {t}"))
}

/// Exif "YYYY:MM:DD HH:MM:SS" → "YYYY-MM-DD HH:MM:SS".
pub(crate) fn exif_local(s: &str) -> Option<String> {
    let s = s.trim();
    let (y, m, d) = (s.get(0..4)?, s.get(5..7)?, s.get(8..10)?);
    if s.as_bytes().get(4) != Some(&b':') || !valid_date(y, m, d) {
        return None;
    }
    let date = format!("{y}-{m}-{d}");
    match (s.get(11..13), s.get(14..16), s.get(17..19)) {
        (Some(h), Some(mi), Some(sec)) if valid_time(h, mi, sec) => {
            Some(format!("{date} {h}:{mi}:{sec}"))
        }
        _ => Some(date),
    }
}

/// A decimal with at most `places` places, trailing zeros trimmed.
pub(crate) fn decimal(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Exposure time in seconds → "1/50", or "2 s" for long exposures.
pub(crate) fn exposure_time(num: u32, den: u32) -> Option<String> {
    if num == 0 || den == 0 {
        return None;
    }
    let secs = num as f64 / den as f64;
    if secs >= 1.0 {
        return Some(format!("{} s", decimal(secs, 1)));
    }
    Some(format!("1/{}", decimal(1.0 / secs, 0)))
}

pub(crate) fn f_number(v: f64) -> Option<String> {
    (v.is_finite() && v > 0.0).then(|| format!("f/{}", decimal(v, 1)))
}

pub(crate) fn focal_length(mm: f64) -> Option<String> {
    (mm.is_finite() && mm > 0.0).then(|| format!("{} mm", decimal(mm, 1)))
}

pub(crate) fn kelvin(k: i64) -> Option<String> {
    (k > 0).then(|| format!("{k} K"))
}

pub(crate) fn shutter_angle(deg: f64) -> Option<String> {
    (deg.is_finite() && deg > 0.0).then(|| format!("{}°", decimal(deg, 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qt_epoch() {
        // ARRI M001C001 mvhd creation_time
        assert_eq!(qt_time(0xD46D_D655).as_deref(), Some("2016-12-07 15:08:05"));
        assert_eq!(qt_time(0), None);
        assert_eq!(qt_time(86_400 * 60).as_deref(), Some("1904-03-01 00:00:00"));
    }

    #[test]
    fn iso_and_exif_dates() {
        assert_eq!(
            iso_local("2022-04-19T22:47:20-05:00").as_deref(),
            Some("2022-04-19 22:47:20")
        );
        assert_eq!(
            iso_local("2021-08-16T10:00:00.5+0200").as_deref(),
            Some("2021-08-16 10:00:00")
        );
        assert_eq!(iso_local("2021-08-16").as_deref(), Some("2021-08-16"));
        assert_eq!(iso_local("2021-13-16"), None);
        assert_eq!(iso_local("garbage"), None);
        assert_eq!(
            exif_local("2025:01:08 06:12:43").as_deref(),
            Some("2025-01-08 06:12:43")
        );
        assert_eq!(exif_local("9999:99:99 00:00:00"), None);
        assert_eq!(exif_local("    :  :     :  :  "), None);
    }

    #[test]
    fn exposure_and_lens_values() {
        assert_eq!(exposure_time(10, 600).as_deref(), Some("1/60"));
        assert_eq!(exposure_time(1, 50).as_deref(), Some("1/50"));
        assert_eq!(exposure_time(2, 1).as_deref(), Some("2 s"));
        assert_eq!(exposure_time(0, 10), None);
        assert_eq!(f_number(2.8).as_deref(), Some("f/2.8"));
        assert_eq!(f_number(4.0).as_deref(), Some("f/4"));
        assert_eq!(f_number(0.0), None);
        assert_eq!(focal_length(24.0).as_deref(), Some("24 mm"));
        assert_eq!(shutter_angle(172.8).as_deref(), Some("172.8°"));
        assert_eq!(shutter_angle(180.0).as_deref(), Some("180°"));
        assert_eq!(kelvin(5600).as_deref(), Some("5600 K"));
    }
}
