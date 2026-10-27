use std::path::{Path, PathBuf};

use luna_core::run::{build_report, merge_outcomes, summary};
use luna_core::scan::Scan;
use luna_core::settings::Settings;
use luna_formats::{ClipFiles, ClipMeta, Container};
use luna_report::{Thumbnail, Unreadable, UnreadableKind};
use luna_worker::pool::JobOutcome;
use luna_worker::{ClipResult, ThumbnailColour};

const THUMB: &[u8] = include_bytes!("../../luna-report/tests/fixtures/thumb.jpg");

fn clip(path: &str) -> ClipFiles {
    ClipFiles {
        container: Container::Mxf,
        main: Path::new("D:/Shoot").join(path),
        segments: vec![],
        sidecars: vec![],
    }
}

fn read(id: u64, thumbs: Vec<Result<Vec<u8>, String>>) -> JobOutcome {
    JobOutcome::Done(Box::new(ClipResult {
        id,
        meta: Some(ClipMeta {
            camera_model: Some("ALEXA 35".into()),
            ..ClipMeta::default()
        }),
        codec: None,
        bytes: 1000,
        thumbnails: thumbs,
        warnings: vec![],
        colour: None,
    }))
}

fn scan() -> Scan {
    Scan {
        clips: vec![
            clip("A_CAM/A001/A001C001_251020_R1AB.mxf"),
            clip("A_CAM/A001/A001C002_251020_R1AB.mxf"),
            clip("B_CAM/B001/B001C001_251020_R2CD.mxf"),
        ],
        not_included: vec!["notes.txt".into()],
        unreadable: vec![Unreadable {
            path: "C_CAM".into(),
            kind: UnreadableKind::Folder,
            reason: "Access denied".into(),
        }],
    }
}

fn settings() -> Settings {
    Settings {
        production: "Night Shift".into(),
        author: "Dana".into(),
        custom: vec![("Day".into(), "3".into())],
        ..Settings::default()
    }
}

#[test]
fn groups_clips_and_carries_lists_and_header() {
    let outcomes = vec![
        read(0, vec![Err("No decoder".into())]),
        read(1, vec![Err("No decoder".into())]),
        read(2, vec![Err("No decoder".into())]),
    ];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan(),
        outcomes,
        "2026-10-10 14:32",
        None,
    );
    assert_eq!(r.header.production.as_deref(), Some("Night Shift"));
    assert_eq!(r.header.author.as_deref(), Some("Dana"));
    assert_eq!(r.header.company, None, "empty settings fields are omitted");
    assert_eq!(r.header.custom[0].label, "Day");
    assert_eq!(r.sources, vec!["D:/Shoot".to_string()]);
    assert_eq!(r.generated_at, "2026-10-10 14:32");
    let cams: Vec<_> = r.cameras.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cams, ["A", "B"]);
    assert_eq!(r.cameras[0].cards[0].reel, "A001");
    assert_eq!(r.cameras[0].cards[0].clips.len(), 2);
    let c = &r.cameras[0].cards[0].clips[0];
    assert_eq!(c.fields.file_name, "A001C001_251020_R1AB.mxf");
    assert_eq!(c.fields.camera_model.as_deref(), Some("ALEXA 35"));
    assert_eq!(c.fields.file_size, Some(1000));
    assert_eq!(r.not_included, ["notes.txt"]);
    assert_eq!(r.unreadable.len(), 1);
}

#[test]
fn a_crashed_or_slow_clip_keeps_its_row_with_reasons() {
    let outcomes = vec![JobOutcome::Crashed, JobOutcome::TimedOut, read(2, vec![])];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan(),
        outcomes,
        "now",
        None,
    );
    let a = &r.cameras[0].cards[0].clips;
    assert_eq!(a[0].fields.file_name, "A001C001_251020_R1AB.mxf");
    assert_eq!(a[0].thumbnails.len(), 3);
    assert!(
        matches!(&a[0].thumbnails[0], Thumbnail::Missing { reason } if reason.contains("crashed"))
    );
    assert!(a[0].warnings.iter().any(|w| w.contains("crashed")));
    assert!(a[1].warnings.iter().any(|w| w.contains("timed out")));
    assert!(a[0].has_problems() && a[1].has_problems());
}

#[test]
fn summary_names_the_problems() {
    let good = || vec![Ok(THUMB.to_vec()); 3];
    let ok = vec![read(0, good()), read(1, good()), read(2, good())];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan(),
        ok,
        "now",
        None,
    );
    assert_eq!(summary(&r), "3 clips, no problems");

    let bad = vec![
        JobOutcome::Crashed,
        read(1, vec![Err("x".into())]),
        read(2, good()),
    ];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan(),
        bad,
        "now",
        None,
    );
    assert_eq!(summary(&r), "2 of 3 clips need attention");

    let mut empty = scan();
    empty.clips.clear();
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        empty,
        vec![],
        "now",
        None,
    );
    assert_eq!(summary(&r), "No camera clips in this folder");
}

#[test]
fn filter_keeps_only_selected_camera() {
    let outcomes = vec![read(0, vec![]), read(1, vec![]), read(2, vec![])];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan(),
        outcomes,
        "now",
        Some(&[2]),
    );
    let cams: Vec<_> = r.cameras.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cams, ["B"]);
    assert_eq!(r.cameras[0].cards[0].clips.len(), 1);
    assert_eq!(
        r.not_included,
        ["notes.txt"],
        "filtered clips are not 'not included'"
    );
}

fn probed_body(id: u64, model: &str) -> JobOutcome {
    JobOutcome::Done(Box::new(ClipResult {
        id,
        meta: Some(ClipMeta {
            camera_model: Some(model.into()),
            ..ClipMeta::default()
        }),
        codec: None,
        bytes: 1000,
        thumbnails: vec![],
        warnings: vec![],
        colour: None,
    }))
}

#[test]
fn filtered_clip_that_fails_to_decode_stays_in_its_camera() {
    let scan = Scan {
        clips: vec![
            clip("A_CAM/A001/A001C001_251020_R1AB.mxf"),
            clip("A_CAM/A002/A002C001_251020_R1AB.mxf"),
        ],
        not_included: vec![],
        unreadable: vec![],
    };
    let probed = vec![probed_body(0, "ALEXA 35"), probed_body(1, "EOS C70")];
    // The selected C70 clip probes fine but crashes when decoded.
    let outcomes = merge_outcomes(probed, vec![(1, JobOutcome::Crashed)], &[1]);
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan,
        outcomes,
        "now",
        Some(&[1]),
    );
    let cams: Vec<_> = r.cameras.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cams, ["A (EOS C70)"]);
    let clip = &r.cameras[0].cards[0].clips[0];
    assert!(clip.has_problems());
    assert_eq!(clip.fields.camera_model.as_deref(), Some("EOS C70"));
    assert!(clip.warnings.iter().any(|w| w.contains("crashed")));
}

fn with_colour(id: u64, colour: &str) -> JobOutcome {
    let JobOutcome::Done(mut r) = read(id, vec![Ok(THUMB.to_vec())]) else {
        unreachable!()
    };
    r.colour = Some(colour.into());
    JobOutcome::Done(r)
}

#[test]
fn colour_choice_is_named_in_the_header_and_clips() {
    let mut s = settings();
    s.colour = ThumbnailColour::Lut;
    s.lut = Some(PathBuf::from("D:/LUTs/show.cube"));
    s.camera_look = true;
    let outcomes = vec![
        with_colour(0, "show.cube"),
        with_colour(1, "BLURRED_B_CAMERA_BM.cube"),
        with_colour(2, "Rec.709"),
    ];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &s,
        scan(),
        outcomes,
        "now",
        None,
    );
    let colour = r
        .header
        .custom
        .iter()
        .find(|f| f.label == "Colour")
        .unwrap();
    assert_eq!(colour.value, "show.cube + camera LUT");
    let clips: Vec<_> = r
        .cameras
        .iter()
        .flat_map(|c| &c.cards)
        .flat_map(|c| &c.clips)
        .map(|c| c.colour.as_deref())
        .collect();
    assert_eq!(
        clips,
        [
            Some("show.cube"),
            Some("BLURRED_B_CAMERA_BM.cube"),
            Some("Rec.709")
        ]
    );
}

#[test]
fn rec709_reports_no_colour() {
    let outcomes = vec![
        with_colour(0, "Rec.709"),
        with_colour(1, "Rec.709"),
        with_colour(2, "Rec.709"),
    ];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &settings(),
        scan(),
        outcomes,
        "now",
        None,
    );
    assert!(r.header.custom.iter().all(|f| f.label != "Colour"));
    assert!(r.cameras[0].cards[0].clips[0].colour.is_none());
}

#[test]
fn none_choice_is_named_none() {
    let mut s = settings();
    s.colour = ThumbnailColour::AsRecorded;
    let outcomes = vec![read(0, vec![]), read(1, vec![]), read(2, vec![])];
    let r = build_report(
        &[PathBuf::from("D:/Shoot")],
        &s,
        scan(),
        outcomes,
        "now",
        None,
    );
    let colour = r
        .header
        .custom
        .iter()
        .find(|f| f.label == "Colour")
        .unwrap();
    assert_eq!(colour.value, "None");
}
