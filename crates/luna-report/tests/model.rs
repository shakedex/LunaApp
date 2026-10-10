mod common;

use common::{LOGO_PNG, THUMB_JPEG, bare_clip, empty_report, sample_report};
use luna_report::{FIELD_LABELS, Logo, LogoError, LogoFormat, Totals, format_bytes};

#[test]
fn totals_count_every_level_and_problem_clips() {
    assert_eq!(
        sample_report().totals(),
        Totals {
            cameras: 2,
            cards: 3,
            clips: 4,
            clips_with_problems: 1,
            bytes: 3 * 4_294_967_296,
        }
    );
}

#[test]
fn empty_report_totals_are_zero() {
    assert_eq!(empty_report().totals(), Totals::default());
}

#[test]
fn display_values_follow_field_labels() {
    let values = common::full_clip("X.mxf").fields.display_values();
    assert_eq!(values.len(), FIELD_LABELS.len());
    assert_eq!(values[0].as_deref(), Some("X.mxf"));
    assert_eq!(values[7].as_deref(), Some("ProRes 4444"));
    assert_eq!(values[17].as_deref(), Some("4.29 GB"));
}

#[test]
fn missing_fields_stay_missing() {
    let values = bare_clip("X.mov").fields.display_values();
    assert_eq!(values[0].as_deref(), Some("X.mov"));
    assert!(values[1..].iter().all(Option::is_none));
}

#[test]
fn bytes_use_decimal_units() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(999), "999 B");
    assert_eq!(format_bytes(1000), "1.00 kB");
    assert_eq!(format_bytes(999_999), "1.00 MB");
    assert_eq!(format_bytes(4_294_967_296), "4.29 GB");
    assert_eq!(format_bytes(u64::MAX), "18446.74 PB");
}

#[test]
fn logo_accepts_png_jpeg_and_svg() {
    assert_eq!(
        Logo::from_bytes(LOGO_PNG.to_vec()).unwrap().format(),
        LogoFormat::Png
    );
    assert_eq!(
        Logo::from_bytes(THUMB_JPEG.to_vec()).unwrap().format(),
        LogoFormat::Jpeg
    );
    let svg =
        br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#;
    assert_eq!(
        Logo::from_bytes(svg.to_vec()).unwrap().format(),
        LogoFormat::Svg
    );
}

#[test]
fn logo_rejects_unknown_and_corrupt_files() {
    assert_eq!(
        Logo::from_bytes(b"GIF89a".to_vec()),
        Err(LogoError::UnknownFormat)
    );
    assert_eq!(Logo::from_bytes(vec![]), Err(LogoError::UnknownFormat));
    let truncated_png = LOGO_PNG[..20].to_vec();
    assert!(matches!(
        Logo::from_bytes(truncated_png),
        Err(LogoError::Unreadable(_))
    ));
    assert!(matches!(
        Logo::from_bytes(b"<svg".to_vec()),
        Err(LogoError::Unreadable(_))
    ));
}
