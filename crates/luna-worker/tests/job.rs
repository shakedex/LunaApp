use luna_formats::{ClipFiles, Container};
use luna_worker::{ClipJob, run_job, strip_positions};

#[test]
fn strip_spans_first_to_last() {
    assert_eq!(strip_positions(100, 3), vec![0, 49, 99]);
    assert_eq!(strip_positions(2, 3), vec![0, 0, 1]);
    assert_eq!(strip_positions(1, 3), vec![0, 0, 0]);
    assert_eq!(strip_positions(0, 3), Vec::<u64>::new());
    assert_eq!(strip_positions(10, 1), vec![0]);
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
