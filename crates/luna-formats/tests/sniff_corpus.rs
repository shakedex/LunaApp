//! Classifies every file of the local test corpus. Runs only when `LUNA_CORPUS`
//! points at it (e.g. `D:\LUNA_TEST\TEST_PROJECT_LUNA\CAMERA`).

use std::path::{Path, PathBuf};

use luna_formats::{Container, SidecarKind, Sniffed, sniff_path};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let ft = entry.file_type().unwrap();
        if ft.is_dir() {
            files(&entry.path(), out);
        } else if ft.is_file() {
            out.push(entry.path());
        }
    }
}

fn expected(name: &str) -> Sniffed {
    use Container::*;
    use Sniffed::{CameraOriginal as O, Sidecar as S};
    let lower = name.to_ascii_lowercase();
    match lower.rsplit('.').next().unwrap_or("") {
        "mxf" => O(Mxf),
        "mov" | "mp4" => O(QuickTime),
        "crm" => O(Crm),
        "braw" => O(Braw),
        "r3d" => O(R3d),
        "nev" => O(Nev),
        "ari" | "arx" => O(AriFrame),
        "ale" => S(SidecarKind::Ale),
        "rmd" => S(SidecarKind::RedRmd),
        "sidecar" => S(SidecarKind::BrawSidecar),
        "xml" if lower.ends_with("m01.xml") => S(SidecarKind::SonyXml),
        "xml" if lower.contains("canon") => S(SidecarKind::CanonXml),
        _ => Sniffed::Other,
    }
}

#[test]
fn corpus_files_classify_by_content() {
    let Some(root) = std::env::var_os("LUNA_CORPUS") else {
        eprintln!("LUNA_CORPUS not set; skipping");
        return;
    };
    if !Path::new(&root).is_dir() {
        eprintln!("skipping: {} not found", Path::new(&root).display());
        return;
    }
    let mut all = Vec::new();
    files(Path::new(&root), &mut all);
    assert!(!all.is_empty(), "corpus is empty");
    let mut wrong = Vec::new();
    for path in &all {
        let name = path.file_name().unwrap().to_string_lossy();
        let got = sniff_path(path).unwrap();
        let want = expected(&name);
        eprintln!("{got:?}\t{}", path.display());
        if got != want {
            wrong.push(format!("{}: got {got:?}, want {want:?}", path.display()));
        }
    }
    assert!(wrong.is_empty(), "misclassified:\n{}", wrong.join("\n"));
}
