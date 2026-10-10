mod common;

use common::{empty_report, hostile_report, non_ascii_report, sample_report};
use luna_report::write_html;

fn html(report: &luna_report::Report) -> String {
    String::from_utf8(write_html(report).unwrap()).unwrap()
}

#[test]
fn renders_grouping_fields_and_lists() {
    let h = html(&sample_report());
    for expected in [
        "Camera report · Night Shift",
        "Lumen Rentals",
        "Report by Dana DIT",
        "Shoot day: 12",
        "Camera A",
        "Reel A001",
        "Reel A002",
        "Camera B",
        "A001C002_261001_R1AB.mxf",
        "4.29 GB",
        "Camera from embedded metadata, reel from file name",
        "Could not be decoded",
        "Worker crashed while decoding",
        "A001/notes.txt",
        "Folder C001: Access denied",
        "2 cameras · 3 cards · 4 clips · 12.88 GB · 1 clips with problems",
    ] {
        assert!(h.contains(expected), "missing {expected:?}");
    }
}

#[test]
fn camera_sections_keep_report_order() {
    let h = html(&sample_report());
    let a = h.find("Reel A001").unwrap();
    let a2 = h.find("Reel A002").unwrap();
    let b = h.find("Reel B001").unwrap();
    assert!(a < a2 && a2 < b);
}

#[test]
fn missing_values_print_a_dash() {
    let h = html(&sample_report());
    let bare = &h[h.find("B001C001.mov").unwrap()..];
    assert!(bare.contains("<dt>Codec</dt><dd>—</dd>"));
}

#[test]
fn self_contained_no_scripts_no_remote_resources() {
    let h = html(&sample_report());
    assert!(!h.contains("<script"));
    assert!(!h.contains("http://") && !h.contains("https://"));
    assert_eq!(
        h.matches("data:image/jpeg;base64,").count(),
        9,
        "three strips of three"
    );
    assert_eq!(h.matches("data:image/png;base64,").count(), 1, "the logo");
}

#[test]
fn markup_in_text_is_escaped() {
    let h = html(&hostile_report());
    assert!(!h.contains("<b>A&B</b>"));
    assert!(h.contains("&#60;b&#62;A&#38;B&#60;/b&#62;"));
}

#[test]
fn empty_report_says_so() {
    let h = html(&empty_report());
    assert!(h.contains("No camera clips found in this source."));
    assert!(!h.contains("Not included"));
    assert!(!h.contains("Unreadable"));
}

#[test]
fn non_ascii_survives() {
    let h = html(&non_ascii_report());
    assert!(h.contains("<meta charset=\"utf-8\">"));
    assert!(h.contains("הפקת לילה"));
    assert!(h.contains("צילום_001_Café.mov"));
}
