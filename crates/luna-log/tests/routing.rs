//! The global subscriber, end to end. It can only be installed once per
//! process, so every test here shares one `init` and one logs folder. Events
//! are DEBUG so the CLI mode keeps them off stderr.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use luna_log::{Mode, init_with_dir};

fn logs() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap()
            .keep();
        init_with_dir(Mode::Cli, Some(dir.clone()));
        dir
    })
}

fn files_ending(suffix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(logs())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(suffix))
        .collect();
    v.sort();
    v
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn run_span_gets_its_own_file() {
    logs();
    {
        let _run = tracing::info_span!(target: "luna::run", "run", source = %"Card A").entered();
        tracing::debug!(target: "luna::test", "inside the run");
        tracing::event!(
            target: "luna::worker_line",
            tracing::Level::DEBUG,
            line = %"12:00:00.000 INFO  [c] x: hi"
        );
    }
    tracing::debug!(target: "luna::test", "outside the run");

    let runs = files_ending(" Card A.log");
    assert_eq!(runs.len(), 1, "{runs:?}");
    let run = read(&runs[0]);
    assert!(run.contains("luna::test: inside the run\n"), "{run}");
    assert!(run.contains("\n12:00:00.000 INFO  [c] x: hi\n"), "{run}");
    assert!(!run.contains("outside the run"));

    let app = read(&logs().join("luna.log"));
    assert!(app.contains("luna::test: outside the run\n"), "{app}");
    assert!(!app.contains("inside the run"));
}

#[test]
fn two_runs_same_second_get_two_files() {
    logs();
    let first = tracing::info_span!(target: "luna::run", "run", source = %"Twin");
    let second = tracing::info_span!(target: "luna::run", "run", source = %"Twin");
    first.in_scope(|| tracing::debug!(target: "luna::test", "from the first"));
    second.in_scope(|| tracing::debug!(target: "luna::test", "from the second"));
    drop(first);
    drop(second);

    let mut runs = files_ending(" Twin.log");
    runs.extend(files_ending(" Twin (2).log"));
    assert_eq!(runs.len(), 2, "{runs:?}");
    let texts: Vec<String> = runs.iter().map(|p| read(p)).collect();
    let only = |a: &str, b: &str| texts.iter().any(|t| t.contains(a) && !t.contains(b));
    assert!(only("from the first", "from the second"), "{texts:?}");
    assert!(only("from the second", "from the first"), "{texts:?}");
}
