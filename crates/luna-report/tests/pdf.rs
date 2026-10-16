mod common;

use common::{HOSTILE, empty_report, full_clip, hostile_report, non_ascii_report, sample_report};
use luna_report::{Card, Thumbnail, inspect_pdf, write_pdf};

#[test]
fn corrupt_thumbnail_still_produces_a_pdf() {
    let mut report = sample_report();
    report.cameras[0].cards[0].clips[0].thumbnails[0] =
        Thumbnail::from_jpeg(vec![0xFF, 0xD8, 0xFF, 0x00, 0x01, 0x02]);
    assert!(write_pdf(&report).unwrap().starts_with(b"%PDF-"));
    let i = inspect_pdf(&report).unwrap();
    assert!(i.text.contains("Thumbnail could not be read"));
}

#[test]
fn no_two_text_runs_overlap_on_a_line() {
    let mut report = sample_report();
    // Standard camera names join everything with underscores, so they never wrap.
    report.cameras[0].cards[0].clips[0].fields.file_name =
        "A001C001_261001_R1AB_LONG_CAMERA_ORIGINAL_NAME.mxf".into();
    let i = inspect_pdf(&report).unwrap();
    for (n, a) in i.runs.iter().enumerate() {
        for b in &i.runs[n + 1..] {
            let same_line = a.page == b.page && (a.y - b.y).abs() < 0.5;
            let overlap = a.x < b.x + b.width - 0.1 && b.x < a.x + a.width - 0.1;
            assert!(
                !(same_line && overlap),
                "{:?} overlaps {:?} on page {}",
                a.text,
                b.text,
                a.page
            );
        }
    }
}

#[test]
fn writes_a_pdf_file() {
    let pdf = write_pdf(&sample_report()).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn renders_grouping_fields_and_lists() {
    let i = inspect_pdf(&sample_report()).unwrap();
    for expected in [
        "Night Shift",
        "Lumen Rentals",
        "Dana DIT",
        "Shoot day",
        "Camera report",
        "25 (sensor 50)",
        "4 ch, 48 kHz, 24-bit",
        "2 cards, 3 clips, 12.88 GB",
        "Camera A",
        "Reel A001",
        "Reel A002",
        "Camera B",
        "A001C002_261001_R1AB.mxf",
        "4.29 GB",
        "Could not be decoded",
        "Worker crashed while decoding",
        "A001/notes.txt",
        "Access denied",
    ] {
        assert!(
            i.text.contains(expected),
            "missing {expected:?} in:\n{}",
            i.text
        );
    }
    assert_eq!(i.images, 9 + 1, "nine thumbnails and the logo");
}

#[test]
fn missing_values_print_a_dash() {
    let i = inspect_pdf(&sample_report()).unwrap();
    assert!(i.text.lines().any(|l| l == "—"));
}

#[test]
fn markup_in_text_prints_literally() {
    let i = inspect_pdf(&hostile_report()).unwrap();
    for fragment in ["<b>A&B</b>", "*x*", "#eval", "$y$", "_z_", "@ref"] {
        assert!(
            i.text.contains(fragment),
            "{fragment:?} was interpreted:\n{}",
            i.text
        );
    }
    assert!(HOSTILE.contains("line2") && i.text.contains("line2"));
}

#[test]
fn hebrew_and_accents_have_glyphs() {
    let i = inspect_pdf(&non_ascii_report()).unwrap();
    assert!(i.text.contains("Zoë Ångström"));
    assert!(
        i.missing_glyphs.is_empty(),
        "no font for: {:?}",
        i.missing_glyphs
    );
}

#[test]
fn empty_report_still_renders() {
    let i = inspect_pdf(&empty_report()).unwrap();
    assert_eq!(i.pages, 1);
    assert!(i.text.contains("No camera clips in this folder."));
}

#[test]
fn large_report_compiles() {
    let mut report = sample_report();
    report.cameras[0].cards = (0..10)
        .map(|c| Card {
            reel: format!("A{c:03}"),
            clips: (0..100)
                .map(|n| full_clip(&format!("A{c:03}C{n:03}.mxf")))
                .collect(),
        })
        .collect();
    let i = inspect_pdf(&report).unwrap();
    assert!(
        i.pages > 100,
        "1000 clips should span many pages, got {}",
        i.pages
    );
    assert!(i.text.contains("A009C099.mxf"));
}

#[test]
fn dark_theme_compiles() {
    let mut r = sample_report();
    r.theme = luna_report::ReportTheme::Dark;
    assert!(write_pdf(&r).unwrap().starts_with(b"%PDF-"));
}
