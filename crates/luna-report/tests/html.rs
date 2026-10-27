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
        "<h1>Night Shift</h1>",
        "<p class=\"subtitle\">Camera report</p>",
        "<dt>Company</dt><dd>Lumen Rentals</dd>",
        "<dt>Report by</dt><dd>Dana DIT</dd>",
        "<dt>Shoot day</dt><dd>12</dd>",
        "Camera A",
        "Reel A001",
        "Reel A002",
        "Camera B",
        "A001C002_261001_R1AB.mxf",
        "4.29 GB",
        "4 ch, 48 kHz, 24-bit",
        "Reel named from the file name",
        "Camera and reel named from the folder name",
        "No image",
        "<span>Need attention</span>",
        "Could not be decoded",
        "Worker crashed while decoding",
        "A001/notes.txt",
        "<td class=\"muted\">Folder</td><td>C001</td><td class=\"reason\">Access denied</td>",
        "<strong>2</strong><span>Cameras</span>",
        "<strong>12.88 GB</strong><span>Data</span>",
        "2 cards, 3 clips, 12.88 GB",
        "2 clips, 8.59 GB",
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
    assert!(bare.contains("<dt>Codec</dt><dd class=\"missing\">—</dd>"));
}

#[test]
fn off_speed_frame_rate_stands_out() {
    let h = html(&sample_report());
    assert!(h.contains("<dt>FPS</dt><dd class=\"accent\">25 (sensor 50)</dd>"));
}

#[test]
fn production_absent_means_a_plain_title() {
    let mut r = sample_report();
    r.header.production = None;
    let h = html(&r);
    assert!(h.contains("<h1>Camera report</h1>"));
    assert!(!h.contains("class=\"subtitle\""));
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
    assert!(h.contains("No camera clips in this folder."));
    assert!(!h.contains("Other files"));
    assert!(!h.contains("Unreadable"));
}

#[test]
fn non_ascii_survives() {
    let h = html(&non_ascii_report());
    assert!(h.contains("<meta charset=\"utf-8\">"));
    assert!(h.contains("הפקת לילה"));
    assert!(h.contains("צילום_001_Café.mov"));
}

#[test]
fn dark_theme_marks_the_page() {
    let mut r = sample_report();
    assert!(!html(&r).contains("class=\"dark\""));
    r.theme = luna_report::ReportTheme::Dark;
    assert!(html(&r).contains("<html lang=\"en\" class=\"dark\">"));
}

#[test]
fn a_failed_frame_says_why_once() {
    let h = html(&sample_report());
    assert_eq!(h.matches("Worker crashed while decoding").count(), 1);
}

#[test]
fn html_lists_several_sources() {
    let mut r = common::sample_report();
    r.sources = vec!["E:/Footage".into(), "F:/Footage".into()];
    let html = String::from_utf8(luna_report::write_html(&r).unwrap()).unwrap();
    assert!(html.contains("E:/Footage, F:/Footage"));
}

#[test]
fn slate_shows_only_when_the_file_has_it() {
    let h = html(&sample_report());
    // Three full clips carry scene 12, shot 3 and no take; the bare clip has none.
    assert_eq!(
        h.matches("<dt>Slate</dt><dd>Sc 12 · Sh 3</dd>").count(),
        3,
        "{h}"
    );
    assert_eq!(h.matches("<dt>Slate</dt>").count(), 3);
}

#[test]
fn hidden_problems_leave_the_report() {
    let mut r = sample_report();
    r.show_problems = false;
    let h = html(&r);
    for gone in [
        "Need attention",
        "Could not be decoded",
        "Worker crashed while decoding",
        "Unreadable",
        "Access denied",
        "class=\"clip problem\"",
    ] {
        assert!(!h.contains(gone), "still has {gone:?}");
    }
    for kept in [
        "No image",
        "Reel named from the file name",
        "A001/notes.txt",
    ] {
        assert!(h.contains(kept), "missing {kept:?}");
    }
}

#[test]
fn clip_colour_cell_shows_when_set() {
    let mut r = sample_report();
    r.cameras[0].cards[0].clips[0].colour = Some("show.cube".into());
    let h = html(&r);
    assert!(h.contains("<dt>Colour</dt><dd>show.cube</dd>"), "{h}");
}

#[test]
fn no_colour_cell_without_one() {
    assert!(!html(&sample_report()).contains("<dt>Colour</dt>"));
}
