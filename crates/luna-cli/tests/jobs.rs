//! The MCP report queue with the real worker binary.

use std::path::Path;
use std::time::{Duration, Instant};

use luna_cli::jobs::{JobState, Jobs};
use luna_core::run::{RunRequest, WorkerSpec};
use luna_core::settings::Settings;

const MXF: [u8; 16] = [
    0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02, 0x04, 0x00,
];

fn tree(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for rel in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, [&MXF[..], &[0; 32]].concat()).unwrap();
    }
    dir
}

fn jobs() -> Jobs {
    // Clips named `__hang__` make the worker hang (luna-worker test hooks).
    // SAFETY: set before any worker starts; every test sets the same value.
    unsafe { std::env::set_var("LUNA_WORKER_TEST_HOOKS", "1") };
    Jobs::new(WorkerSpec {
        exe: env!("CARGO_BIN_EXE_luna-cli").into(),
        args: vec!["--worker".into()],
        workers: 1,
        timeout: Duration::from_secs(120),
    })
}

fn request(src: &Path, out: &Path) -> RunRequest {
    RunRequest::folder(
        src.to_path_buf(),
        Settings {
            output_dir: out.to_path_buf(),
            ..Settings::default()
        },
    )
}

fn wait_for(jobs: &Jobs, id: &str, want: impl Fn(JobState) -> bool) -> JobState {
    let start = Instant::now();
    loop {
        let state = jobs.status(id).unwrap().state;
        if want(state) {
            return state;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "stuck in {state:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn runs_in_order_and_finishes() {
    let jobs = jobs();
    let hang = tree(&["A001/A001C001__hang__.mxf"]);
    let ok = tree(&["B001/B001C001_251020_R2CD.mxf"]);
    let out = tempfile::tempdir().unwrap();
    let first = jobs.start(request(hang.path(), out.path()));
    let second = jobs.start(request(ok.path(), out.path()));
    wait_for(&jobs, &first, |s| s == JobState::Running);
    assert_eq!(jobs.status(&second).unwrap().state, JobState::Queued);
    assert_eq!(jobs.cancel(&first), Some(JobState::Cancelled));
    wait_for(&jobs, &second, |s| s == JobState::Done);
    let result = jobs.status(&second).unwrap().result.unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(jobs.status(&first).unwrap().state, JobState::Cancelled);
}

#[test]
fn cancel_queued_job() {
    let jobs = jobs();
    let hang = tree(&["A001/A001C001__hang__.mxf"]);
    let ok = tree(&["B001/B001C001_251020_R2CD.mxf"]);
    let out = tempfile::tempdir().unwrap();
    let first = jobs.start(request(hang.path(), out.path()));
    let second = jobs.start(request(ok.path(), out.path()));
    assert_eq!(jobs.cancel(&second), Some(JobState::Cancelled));
    jobs.cancel(&first);
    wait_for(&jobs, &first, |s| s == JobState::Cancelled);
    jobs.shutdown();
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
}

#[test]
fn unknown_id_is_none() {
    let jobs = jobs();
    assert!(jobs.status("r99").is_none());
    assert!(jobs.cancel("r99").is_none());
}

#[test]
fn exclusive_waits_for_the_running_report() {
    let jobs = std::sync::Arc::new(jobs());
    let hang = tree(&["A001/A001C001__hang__.mxf"]);
    let out = tempfile::tempdir().unwrap();
    let first = jobs.start(request(hang.path(), out.path()));
    wait_for(&jobs, &first, |s| s == JobState::Running);
    let (tx, rx) = std::sync::mpsc::channel();
    let j = jobs.clone();
    std::thread::spawn(move || tx.send(j.exclusive(|_| "ran")).unwrap());
    // The pool is busy with the report: the inspect-style work must wait.
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    jobs.cancel(&first);
    rx.recv_timeout(Duration::from_secs(30))
        .expect("runs once the report stops");
}

#[test]
fn shutdown_stops_exclusive_work() {
    let jobs = std::sync::Arc::new(jobs());
    let (tx, rx) = std::sync::mpsc::channel();
    let j = jobs.clone();
    std::thread::spawn(move || {
        j.exclusive(|stop| {
            tx.send("started").unwrap();
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        tx.send("stopped").unwrap();
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)), Ok("started"));
    jobs.shutdown();
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)), Ok("stopped"));
}
