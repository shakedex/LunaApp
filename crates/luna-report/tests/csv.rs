mod common;

use common::{HOSTILE, empty_report, hostile_report, non_ascii_report, sample_report};
use luna_report::write_csv;

const HEADER: &str = "Camera,Reel,File name,Recording date,TC in,TC out,Duration,FPS,Resolution,Codec,Camera model,Camera serial,Lens,Focal length,T/f-stop,ISO/EI,Shutter,WB,ND,File size (bytes)";

fn text(bytes: &[u8]) -> &str {
    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF], "UTF-8 byte-order mark");
    std::str::from_utf8(&bytes[3..]).unwrap()
}

#[test]
fn one_row_per_clip_with_camera_and_reel_first() {
    let csv = write_csv(&sample_report());
    let lines: Vec<&str> = text(&csv).split("\r\n").collect();
    assert_eq!(lines[0], HEADER);
    assert_eq!(
        lines[1],
        "A,A001,A001C001_261001_R1AB.mxf,2026-10-01,10:00:00:00,10:00:10:00,00:00:10:00,25,4096×2160,ProRes 4444,ALEXA 35,60123,Signature Prime 47,47 mm,T2.8,800,180°,5600 K,0.6,4294967296"
    );
    assert_eq!(
        lines.len(),
        1 + 4 + 1,
        "header, four clips, trailing empty after last CRLF"
    );
    assert_eq!(lines[5], "");
}

#[test]
fn missing_values_are_empty_cells() {
    let csv = write_csv(&sample_report());
    let last = text(&csv).split("\r\n").nth(4).unwrap();
    assert_eq!(last, "B,B001,B001C001.mov,,,,,,,,,,,,,,,,,");
}

#[test]
fn empty_report_is_header_only() {
    let csv = write_csv(&empty_report());
    assert_eq!(text(&csv), format!("{HEADER}\r\n"));
}

#[test]
fn commas_quotes_and_newlines_are_quoted() {
    let csv = write_csv(&hostile_report());
    let quoted = format!("\"{}\"", HOSTILE.replace('"', "\"\""));
    let body = text(&csv);
    assert!(body.contains(&format!("{quoted},A001,{quoted},")), "{body}");
}

#[test]
fn non_ascii_survives() {
    let csv = write_csv(&non_ascii_report());
    assert!(text(&csv).contains("צילום_001_Café.mov"));
}
