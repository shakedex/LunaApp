use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::test_support::{tree, write};

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn single_sony_card_attaches_m01_xml() {
    let t = tree(&["B005C039_201101OZ.mxf", "B005C039_201101OZM01.XML"]);
    let s = scan(t.path());
    assert_eq!(s.clips.len(), 1);
    let c = &s.clips[0];
    assert_eq!(c.container, Container::Mxf);
    assert_eq!(name(&c.main), "B005C039_201101OZ.mxf");
    assert_eq!(c.sidecars.len(), 1);
    assert_eq!(c.sidecars[0].0, SidecarKind::SonyXml);
    assert!(s.not_included.is_empty());
    assert!(s.unreadable.is_empty());
}

#[test]
fn sidecars_attach_by_name_and_the_rest_is_not_included() {
    let t = tree(&[
        "S001/S_0001C038X241003_1807575U_CANONRAW.CRM",
        "S001/S_0001C038X241003_1807575U_CANONRAW.XML",
        "D001/D001_09150935_C013.braw",
        "D001/D001_09150935_C013.sidecar",
        "R/A005_C055_0819CM_001.R3D",
        "R/A005_C055_0819CM.RMD",
        "M001/M001C001_161207_R00H.mov",
        "M001/M001R00H_AVID.ale",
        "M001/M001R00H_BIN.bin",
        "M001/orphan.XML",
        "report.pdf",
    ]);
    let s = scan(t.path());
    let by_main: BTreeMap<String, &ClipFiles> =
        s.clips.iter().map(|c| (name(&c.main), c)).collect();
    assert_eq!(by_main.len(), 4);
    let kinds = |n: &str| -> Vec<SidecarKind> { by_main[n].sidecars.iter().map(|s| s.0).collect() };
    assert_eq!(
        kinds("S_0001C038X241003_1807575U_CANONRAW.CRM"),
        [SidecarKind::CanonXml]
    );
    assert_eq!(kinds("D001_09150935_C013.braw"), [SidecarKind::BrawSidecar]);
    assert_eq!(kinds("A005_C055_0819CM_001.R3D"), [SidecarKind::RedRmd]);
    assert!(kinds("M001C001_161207_R00H.mov").is_empty());
    assert_eq!(
        by_main["D001_09150935_C013.braw"].container,
        Container::Braw
    );
    assert_eq!(
        by_main["S_0001C038X241003_1807575U_CANONRAW.CRM"].container,
        Container::Crm
    );
    assert_eq!(
        s.not_included,
        [
            "M001/M001R00H_AVID.ale",
            "M001/M001R00H_BIN.bin",
            "M001/orphan.XML",
            "report.pdf"
        ]
    );
}

#[test]
fn spanned_r3d_is_one_clip_in_segment_order() {
    let t = tree(&[
        "A005_C055_0819CM_003.R3D",
        "A005_C055_0819CM_001.R3D",
        "A005_C055_0819CM_002.R3D",
        "A005_C056_0819XY_001.R3D",
    ]);
    let s = scan(t.path());
    assert_eq!(s.clips.len(), 2);
    let a = &s.clips[0];
    assert_eq!(a.container, Container::R3d);
    assert_eq!(name(&a.main), "A005_C055_0819CM_001.R3D");
    assert_eq!(
        names(&a.segments),
        ["A005_C055_0819CM_002.R3D", "A005_C055_0819CM_003.R3D"]
    );
    assert_eq!(name(&s.clips[1].main), "A005_C056_0819XY_001.R3D");
    assert!(s.clips[1].segments.is_empty());
}

#[test]
fn ari_frames_form_one_clip_per_sequence() {
    let t = tree(&[
        "A001C001_120101_R1AB/A001C001_120101_R1AB.0000002.ari",
        "A001C001_120101_R1AB/A001C001_120101_R1AB.0000010.ari",
        "A001C001_120101_R1AB/A001C001_120101_R1AB.0000001.ari",
        "A001C001_120101_R1AB/A001C002_120101_R1AB.0000001.ari",
    ]);
    let s = scan(t.path());
    assert_eq!(s.clips.len(), 2);
    let a = &s.clips[0];
    assert_eq!(a.container, Container::AriFrame);
    assert_eq!(name(&a.main), "A001C001_120101_R1AB.0000001.ari");
    assert_eq!(
        names(&a.segments),
        [
            "A001C001_120101_R1AB.0000002.ari",
            "A001C001_120101_R1AB.0000010.ari"
        ]
    );
    assert_eq!(name(&s.clips[1].main), "A001C002_120101_R1AB.0000001.ari");
}

#[test]
fn clutter_is_skipped_entirely() {
    let t = tree(&[
        "A001/A001C001_120101_R1AB.mxf",
        "A001/._A001C001_120101_R1AB.mxf",
        "A001/.DS_Store",
        "A001/Thumbs.db",
        "A001/desktop.ini",
        ".Trashes/501/x.mxf",
        ".Spotlight-V100/Store-V2/x",
        ".fseventsd/0000",
        ".TemporaryItems/x.mov",
        "System Volume Information/IndexerVolumeGuid",
        "$RECYCLE.BIN/S-1-5/x.mxf",
    ]);
    let s = scan(t.path());
    assert_eq!(s.clips.len(), 1);
    assert!(s.not_included.is_empty(), "{:?}", s.not_included);
    assert!(s.unreadable.is_empty());
}

#[test]
fn unreadable_folder_and_file_are_listed_and_the_walk_continues() {
    let t = tree(&[
        "A_CAM/A001/A001C001_120101_R1AB.mxf",
        "B_CAM/B001/B001C001_120101_R1AB.mxf",
        "C_CAM/C001/C001C001_120101_R1AB.mxf",
        "C_CAM/C001/C001C002_120101_R1AB.mxf",
    ]);
    hook::deny(&t.path().join("B_CAM"));
    hook::deny(&t.path().join("C_CAM/C001/C001C002_120101_R1AB.mxf"));
    let s = scan(t.path());
    assert_eq!(
        s.clips.iter().map(|c| name(&c.main)).collect::<Vec<_>>(),
        ["A001C001_120101_R1AB.mxf", "C001C001_120101_R1AB.mxf"]
    );
    assert_eq!(
        s.unreadable,
        [
            Unreadable {
                path: "B_CAM".into(),
                kind: UnreadableKind::Folder,
                reason: "permission denied".into(),
            },
            Unreadable {
                path: "C_CAM/C001/C001C002_120101_R1AB.mxf".into(),
                kind: UnreadableKind::File,
                reason: "permission denied".into(),
            },
        ]
    );
}

#[test]
fn missing_root_is_one_unreadable_folder() {
    let t = tree(&[]);
    let s = scan(&t.path().join("gone"));
    assert!(s.clips.is_empty());
    assert_eq!(s.unreadable.len(), 1);
    assert_eq!(s.unreadable[0].path, ".");
    assert_eq!(s.unreadable[0].kind, UnreadableKind::Folder);
}

#[test]
fn symlinks_are_not_followed() {
    let t = tree(&["A001/A001C001_120101_R1AB.mxf"]);
    let link = t.path().join("loop");
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(t.path(), &link);
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(t.path(), &link);
    if made.is_err() {
        eprintln!("symlinks need privileges here; skipping");
        return;
    }
    let s = scan(t.path());
    assert_eq!(s.clips.len(), 1);
    assert_eq!(s.not_included, ["loop"]);
}

/// Every file on disk (clutter aside) lands in exactly one place.
#[test]
fn every_file_is_accounted_for_exactly_once() {
    let files = [
        "A_CAM/A001/A001C001_120101_R1AB.mxf",
        "A_CAM/A001/A001C002_120101_R1AB.mxf",
        "A_CAM/A001/A001R1AB.ale",
        "A_CAM/A001/A001.mhl",
        "A_CAM/A002/A002C001_120102_R1AB.0000001.ari",
        "A_CAM/A002/A002C001_120102_R1AB.0000002.ari",
        "B_CAM/B005/B005C039_201101OZ.mxf",
        "B_CAM/B005/B005C039_201101OZM01.XML",
        "B_CAM/B005/B005C040_201101OZM01.XML",
        "R/A005_C055_0819CM_001.R3D",
        "R/A005_C055_0819CM_002.R3D",
        "R/A005_C055_0819CM.RMD",
        "D/D001_09150935_C013.braw",
        "D/D001_09150935_C013.sidecar",
        "S/S_0001C038X241003_1807575U_CANONRAW.CRM",
        "S/S_0001C038X241003_1807575U_CANONRAW.XML",
        "S/notes.txt",
        "S/broken.mxf",
        "S/locked/inside.mov",
        "readme.pdf",
        "A_CAM/.DS_Store",
        "A_CAM/A001/._A001C001_120101_R1AB.mxf",
    ];
    let t = tree(&files);
    // "broken.mxf" is unreadable; so is the "locked" folder.
    let root = t.path();
    hook::deny(&root.join("S/broken.mxf"));
    hook::deny(&root.join("S/locked"));
    let s = scan(root);

    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for c in &s.clips {
        let all = std::iter::once(&c.main)
            .chain(&c.segments)
            .chain(c.sidecars.iter().map(|(_, p)| p));
        for p in all {
            *seen.entry(relative(root, p)).or_default() += 1;
        }
    }
    for p in &s.not_included {
        *seen.entry(p.clone()).or_default() += 1;
    }
    let mut unreadable_folders = Vec::new();
    for u in &s.unreadable {
        match u.kind {
            UnreadableKind::File => *seen.entry(u.path.clone()).or_default() += 1,
            UnreadableKind::Folder => unreadable_folders.push(u.path.clone()),
        }
    }
    assert_eq!(unreadable_folders, ["S/locked"]);
    assert!(seen.values().all(|&n| n == 1), "counted twice: {seen:?}");

    let expected: Vec<String> = files
        .iter()
        .filter(|f| !is_clutter(f.rsplit('/').next().unwrap()))
        .filter(|f| !f.starts_with("S/locked/"))
        .map(|f| f.to_string())
        .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(seen.keys().cloned().collect::<Vec<_>>(), expected);
}

#[test]
fn relative_paths_use_forward_slashes() {
    let root = Path::new("root");
    assert_eq!(relative(root, &root.join("a").join("b.mxf")), "a/b.mxf");
    assert_eq!(relative(root, root), ".");
}

#[test]
fn write_helper_makes_sniffable_files() {
    let t = tree(&[]);
    let p = write(t.path(), "x/A.mxf");
    assert_eq!(
        luna_formats::sniff_path(&p).unwrap(),
        Sniffed::CameraOriginal(Container::Mxf)
    );
}
