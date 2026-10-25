//! `luna-cli report` and `inspect` as a script runs them: exit codes and
//! stdout. The fixture clips are synthetic, so every clip has problems.

use std::path::Path;
use std::process::{Command, Output};

const MXF: [u8; 16] = [
    0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02, 0x04, 0x00,
];

fn tree(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for rel in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let content = if rel.ends_with(".mxf") {
            [&MXF[..], &[0; 32]].concat()
        } else {
            b"not a camera file".to_vec()
        };
        std::fs::write(path, content).unwrap();
    }
    dir
}

fn two_cameras() -> tempfile::TempDir {
    tree(&[
        "A_CAM/A001/A001C001_251020_R1AB.mxf",
        "A_CAM/A001/A001C002_251020_R1AB.mxf",
        "B_CAM/B001/B001C001_251020_R2CD.mxf",
        "notes.txt",
    ])
}

/// Runs `luna-cli` with its logs in a fresh folder, returned with the output.
fn luna_logged(args: &[&str]) -> (Output, tempfile::TempDir) {
    let logs = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_luna-cli"))
        .args(args)
        .env("LUNA_LOG_DIR", logs.path())
        .output()
        .unwrap();
    (out, logs)
}

fn luna(args: &[&str]) -> Output {
    luna_logged(args).0
}

/// The run logs in `logs`, without `luna.log`.
fn run_logs(logs: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(logs)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "log")
                && p.file_name().is_some_and(|n| n != "luna.log")
        })
        .collect()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)))
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn report_json_exit_1_when_clips_have_problems() {
    let src = two_cameras();
    let out_dir = tempfile::tempdir().unwrap();
    let out = luna(&[
        "report",
        s(src.path()),
        "--output",
        s(out_dir.path()),
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = json(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["clips"], 3);
    assert_eq!(v["clips_with_problems"], 3);
    assert_eq!(v["not_included"], 1);
    assert!(Path::new(v["files"]["pdf"].as_str().unwrap()).exists());
}

#[test]
#[ignore = "needs the run span (Task 3)"]
fn logs_go_to_the_log_dir_not_stdout() {
    let src = two_cameras();
    let out_dir = tempfile::tempdir().unwrap();
    let (out, logs) = luna_logged(&[
        "report",
        s(src.path()),
        "--output",
        s(out_dir.path()),
        "--json",
    ]);
    assert_eq!(json(&out)["clips"], 3);
    assert!(logs.path().join("luna.log").exists());
    assert_eq!(run_logs(logs.path()).len(), 1);
}

#[test]
fn camera_filter_reports_one_camera() {
    let src = two_cameras();
    let out_dir = tempfile::tempdir().unwrap();
    let out = luna(&[
        "report",
        s(src.path()),
        "--output",
        s(out_dir.path()),
        "--camera",
        "B",
        "--json",
    ]);
    let v = json(&out);
    assert_eq!(v["clips"], 1, "{v}");
    let csv = std::fs::read_to_string(v["files"]["csv"].as_str().unwrap()).unwrap();
    assert!(csv.contains("B001C001"));
    assert!(!csv.contains("A001C001"));
}

#[test]
fn missing_source_exits_3() {
    let out_dir = tempfile::tempdir().unwrap();
    let missing = out_dir.path().join("gone");
    let out = luna(&[
        "report",
        s(&missing),
        "--output",
        s(out_dir.path()),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let v = json(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["code"], 3);
    assert!(v["error"].as_str().unwrap().contains("gone"));
    assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 0);
}

#[test]
fn bad_arguments_exit_2() {
    assert_eq!(luna(&["report"]).status.code(), Some(2));
    assert_eq!(
        luna(&["report", ".", "--theme", "purple"]).status.code(),
        Some(2)
    );
}

#[test]
fn unknown_camera_exits_4_and_lists_found() {
    let src = two_cameras();
    let out_dir = tempfile::tempdir().unwrap();
    let out = luna(&[
        "report",
        s(src.path()),
        "--output",
        s(out_dir.path()),
        "--camera",
        "Z",
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(4));
    let err = json(&out)["error"].as_str().unwrap().to_owned();
    assert!(err.contains("A") && err.contains("B"), "{err}");
}

#[test]
fn inspect_json_lists_cards() {
    let src = two_cameras();
    let out = luna(&["inspect", s(src.path()), "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = json(&out);
    let ids: Vec<_> = v["cameras"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["A", "B"]);
    assert_eq!(v["cameras"][0]["cards"][0]["clips"], 2);
}

#[test]
fn plain_output_prints_pdf_then_summary() {
    let src = two_cameras();
    let out_dir = tempfile::tempdir().unwrap();
    let out = luna(&["report", s(src.path()), "--output", s(out_dir.path())]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut lines = stdout.lines();
    assert!(lines.next().unwrap().ends_with(".pdf"));
    assert!(lines.next().unwrap().contains("need attention"));
}

/// `dirs` reads the Windows config folder through the shell API, not
/// `APPDATA`, so this checks the real settings file is byte-for-byte the same
/// after a run with overrides (the run only ever reads it).
#[test]
fn saved_settings_untouched() {
    let settings =
        luna_core::settings::SettingsStore::platform().map(|s| s.dir().join("settings.toml"));
    let before = settings.as_ref().and_then(|p| std::fs::read(p).ok());
    let src = two_cameras();
    let out_dir = tempfile::tempdir().unwrap();
    luna(&[
        "report",
        s(src.path()),
        "--output",
        s(out_dir.path()),
        "--production",
        "Override",
        "--field",
        "Day=9",
    ]);
    let after = settings.as_ref().and_then(|p| std::fs::read(p).ok());
    assert_eq!(before, after);
}
