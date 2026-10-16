//! `run` with a `RunRequest`: what is decided before any worker starts.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use luna_core::run::{RunError, RunRequest, WorkerSpec, run};
use luna_core::settings::Settings;

#[test]
fn unreadable_second_source_fails_up_front() {
    let good = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let missing = good.path().join("not here");
    let req = RunRequest {
        sources: vec![good.path().to_path_buf(), missing.clone()],
        cameras: vec![],
        settings: Settings {
            output_dir: out.path().to_path_buf(),
            ..Settings::default()
        },
        name: None,
    };
    // A worker that can't exist: the run must fail before starting any.
    let workers = WorkerSpec {
        exe: PathBuf::from("no-such-luna-worker"),
        args: vec![],
        workers: 1,
        timeout: Duration::from_secs(5),
    };
    let err = run(&req, &workers, &|_| {}, &AtomicBool::new(false)).unwrap_err();
    match err {
        RunError::SourceUnreadable(m) => assert!(m.contains("not here"), "{m}"),
        other => panic!("expected SourceUnreadable, got {other:?}"),
    }
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
}
