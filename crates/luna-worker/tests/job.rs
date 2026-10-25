#[path = "common/collect.rs"]
mod collect;

use std::path::PathBuf;

use luna_formats::{ClipFiles, Container};
use luna_worker::{ClipJob, run_job, strip_positions};
use tracing::Level;

#[test]
fn strip_spans_first_to_last() {
    assert_eq!(strip_positions(100, 3, 0), vec![0, 49, 99]);
    assert_eq!(strip_positions(2, 3, 0), vec![0, 0, 1]);
    assert_eq!(strip_positions(1, 3, 0), vec![0, 0, 0]);
    assert_eq!(strip_positions(0, 3, 0), Vec::<u64>::new());
    assert_eq!(strip_positions(10, 1, 0), vec![0]);
}

#[test]
fn outer_thumbnails_sit_inside_the_clip() {
    // 10 s at 25 fps, one second in from each end.
    assert_eq!(strip_positions(250, 3, 25), vec![25, 124, 224]);
    // A 2 s clip gives up at most a tenth of its frames at each end.
    assert_eq!(strip_positions(50, 3, 25), vec![5, 24, 44]);
    // Too short to inset at all.
    assert_eq!(strip_positions(5, 3, 25), vec![0, 2, 4]);
}

#[test]
fn an_unreadable_clip_still_answers_with_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.mov");
    std::fs::write(&path, b"not a movie at all").unwrap();
    let result = run_job(&ClipJob {
        id: 9,
        files: ClipFiles {
            container: Container::QuickTime,
            main: path,
            segments: vec![],
            sidecars: vec![],
        },
        strip: 3,
        thumb_width: 640,
    });
    assert_eq!(result.id, 9);
    assert_eq!(result.bytes, 18);
    assert!(result.meta.is_none());
    assert_eq!(result.thumbnails.len(), 3);
    assert!(result.thumbnails.iter().all(|t| t.is_err()));
    assert!(!result.warnings.is_empty());
}

fn header_slice(name: &str) -> ClipJob {
    ClipJob {
        id: 1,
        files: ClipFiles {
            container: Container::Mxf,
            main: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../luna-formats/tests/fixtures/mxf")
                .join(name),
            segments: vec![],
            sidecars: vec![],
        },
        strip: 3,
        thumb_width: 640,
    }
}

fn has(events: &[collect::Seen], level: Level, text: &str) -> bool {
    events
        .iter()
        .any(|e| e.level == level && e.text.contains(text))
}

#[test]
fn a_job_logs_probe_meta_and_decode_errors_inside_its_clip() {
    // A header slice: it probes, but has no essence to decode.
    let (result, events) = collect::capture(|| run_job(&header_slice("c50_xfavc_header.bin")));
    assert!(result.meta.is_some());
    assert!(
        events
            .iter()
            .all(|e| e.clip.as_deref() == Some("c50_xfavc_header")),
        "{events:#?}"
    );
    assert!(has(&events, Level::INFO, "probe: own"), "{events:#?}");
    assert!(
        has(&events, Level::DEBUG, "camera_model=EOS C50"),
        "{events:#?}"
    );
    assert!(has(&events, Level::DEBUG, "lens=missing"), "{events:#?}");
    assert!(has(&events, Level::DEBUG, "positions="), "{events:#?}");
    assert!(has(&events, Level::DEBUG, "decode: H.264"), "{events:#?}");
    let decode_errors = events
        .iter()
        .filter(|e| e.level == Level::WARN && e.text.contains("decode failed: frame"))
        .count();
    assert_eq!(decode_errors, 3, "{events:#?}");
}

#[test]
fn an_unknown_frame_count_is_logged() {
    let (_, events) = collect::capture(|| run_job(&header_slice("minilf_prores_header.bin")));
    assert!(
        has(&events, Level::DEBUG, "frame_count=missing"),
        "{events:#?}"
    );
    assert!(
        has(&events, Level::WARN, "no thumbnails: frame count unknown"),
        "{events:#?}"
    );
}

#[test]
fn an_unreadable_clip_logs_the_probe_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.mov");
    std::fs::write(&path, b"not a movie at all").unwrap();
    let (_, events) = collect::capture(|| {
        run_job(&ClipJob {
            id: 2,
            files: ClipFiles {
                container: Container::QuickTime,
                main: path,
                segments: vec![],
                sidecars: vec![],
            },
            strip: 3,
            thumb_width: 640,
        })
    });
    assert!(
        events.iter().any(|e| e.level == Level::WARN
            && e.clip.as_deref() == Some("broken")
            && e.text.contains("probe failed")),
        "{events:#?}"
    );
}
