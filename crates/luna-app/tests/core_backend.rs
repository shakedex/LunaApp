//! The real backend, end to end, with the `luna-app` binary as its own worker.

use std::sync::mpsc;
use std::time::Duration;

use luna_app::backend::{RunBackend, RunEvent};
use luna_app::core_backend::CoreBackend;
use luna_core::run::WorkerSpec;
use luna_core::settings::Settings;

fn backend() -> CoreBackend {
    CoreBackend::new(WorkerSpec {
        exe: env!("CARGO_BIN_EXE_luna-app").into(),
        args: vec!["--worker".into()],
        workers: 2,
        timeout: Duration::from_secs(60),
    })
}

fn collect(
    backend: &CoreBackend,
    source: &std::path::Path,
    out: &std::path::Path,
) -> Vec<RunEvent> {
    let (tx, rx) = mpsc::channel();
    let settings = Settings {
        production: "Test".into(),
        output_dir: out.to_path_buf(),
        ..Settings::default()
    };
    let _control = backend.start(
        source.to_path_buf(),
        settings,
        Box::new(move |e| {
            let _ = tx.send(e);
        }),
    );
    let mut events = Vec::new();
    while let Ok(e) = rx.recv_timeout(Duration::from_secs(120)) {
        let last = matches!(
            e,
            RunEvent::Finished(_) | RunEvent::Failed(_) | RunEvent::Cancelled
        );
        events.push(e);
        if last {
            break;
        }
    }
    events
}

#[test]
fn a_source_becomes_three_report_files() {
    let src = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    std::fs::create_dir(src.path().join("A001")).unwrap();
    // A movie-shaped file the parser will reject: the clip still gets a row.
    let mut mov = 16u32.to_be_bytes().to_vec();
    mov.extend_from_slice(b"ftypqt  \0\0\0\0");
    std::fs::write(src.path().join("A001/A001C001_251020_R1AB.mov"), mov).unwrap();
    std::fs::write(src.path().join("notes.txt"), b"hello").unwrap();

    let events = collect(&backend(), src.path(), out.path());
    assert!(events.contains(&RunEvent::FilesFound(2)), "{events:?}");
    assert!(events.contains(&RunEvent::ClipsTotal(1)), "{events:?}");
    let Some(RunEvent::Finished(outcome)) = events.last() else {
        panic!("{events:?}")
    };
    assert!(std::fs::read(&outcome.pdf).unwrap().starts_with(b"%PDF-"));
    assert!(outcome.html.exists() && outcome.csv.exists());
    assert_eq!(outcome.folder, outcome.pdf.parent().unwrap());
    assert_eq!(outcome.summary, "The clip needs attention");
}

#[test]
fn an_unreadable_source_fails_the_run() {
    let out = tempfile::tempdir().unwrap();
    let events = collect(
        &backend(),
        std::path::Path::new("Z:/no/such/folder"),
        out.path(),
    );
    assert!(
        matches!(events.last(), Some(RunEvent::Failed(_))),
        "{events:?}"
    );
}
