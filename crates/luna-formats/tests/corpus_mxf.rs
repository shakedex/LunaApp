//! Golden tests over the camera corpus. Skipped unless `LUNA_CORPUS` points at
//! the CAMERA folder of the test project.

use std::path::{Path, PathBuf};
use std::time::Instant;

use luna_formats::{ClipFiles, Container, FrameIndex, Probe, SidecarKind, probe};

fn corpus() -> Option<PathBuf> {
    std::env::var_os("LUNA_CORPUS").map(PathBuf::from)
}

fn clip(root: &Path, rel: &str, sidecar: Option<(SidecarKind, &str)>) -> ClipFiles {
    ClipFiles {
        container: Container::Mxf,
        main: root.join(rel),
        segments: vec![],
        sidecars: sidecar
            .map(|(k, p)| vec![(k, root.join(p))])
            .unwrap_or_default(),
    }
}

fn run(rel: &str, sidecar: Option<(SidecarKind, &str)>) -> Option<Probe> {
    let root = corpus()?;
    let t = Instant::now();
    let p = probe(&clip(&root, rel, sidecar)).expect(rel);
    eprintln!("{rel}: {:?}", t.elapsed());
    Some(p)
}

fn dump(p: &Probe) {
    eprintln!("{:#?}", p.codec);
    eprintln!("{:#?}", p.meta);
    eprintln!("{:#?}", p.layout);
    eprintln!("{:#?}", p.warnings);
    if let FrameIndex::Ranges(r) = &p.frames {
        eprintln!(
            "frames {} first {:?} last {:?}",
            r.len(),
            r.first(),
            r.last()
        );
    }
}

#[test]
fn dump_all() {
    let files: &[(&str, Option<(SidecarKind, &str)>)] = &[
        ("A_CAM/A001/A0001C027_251020_135413_p1I7H.mxf", None),
        (
            "BURANO/BURANO XOCN LT Clip 2.mxf",
            Some((SidecarKind::SonyXml, "BURANO/BURANO XOCN LT Clip 2M01.xml")),
        ),
        (
            "B_CAM/B005/B005C039_201101OZ.mxf",
            Some((SidecarKind::SonyXml, "B_CAM/B005/B005C039_201101OZM01.xml")),
        ),
        ("F_CAM/F003/F003C004_190925_MN99.mxf", None),
        ("P_CAM/VENICE 2 8K 1.8x Anamorphic.mxf", None),
        ("S_cam/S003/Filmplusgear-FX6-testfile-1-254_0110.MXF", None),
        ("S_cam/s004/s_0004C062A250107_2240217S_CANON.MXF", None),
        ("T_CAM/DW0001C014_251020_123457_c1I7H-arricore.mxf", None),
        ("T_CAM/DW0001C021_251020_132307_a1I7H_arri-raw.mxf", None),
    ];
    for (f, s) in files {
        if let Some(p) = run(f, *s) {
            eprintln!("===== {f}");
            dump(&p);
        }
    }
}
