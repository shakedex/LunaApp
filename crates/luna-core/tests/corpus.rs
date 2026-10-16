//! Scans and groups the local test corpus. Runs only when `LUNA_CORPUS` points
//! at it (`D:\LUNA_TEST\TEST_PROJECT_LUNA\CAMERA`). No probing yet, so identity
//! comes from file names and folders.

use std::collections::BTreeMap;
use std::path::Path;

use luna_core::identity::resolve_identity;
use luna_core::scan::scan;
use luna_formats::{ClipFiles, ClipMeta};
use luna_report::IdentitySource::{self, FileName, Folder, FolderName};

#[test]
fn corpus_groups_by_camera_and_card() {
    let Some(root) = std::env::var_os("LUNA_CORPUS") else {
        eprintln!("LUNA_CORPUS not set; skipping");
        return;
    };
    let root = Path::new(&root);
    let s = scan(root);
    assert!(s.unreadable.is_empty(), "{:?}", s.unreadable);
    assert_eq!(
        s.not_included,
        [
            "M_CAM/M001/M001R00H_AVID.ale",
            "M_CAM/M001/M001R00H_BIN.bin"
        ]
    );
    assert_eq!(s.clips.len(), 27);
    let sidecars: usize = s.clips.iter().map(|c| c.sidecars.len()).sum();
    assert_eq!(sidecars, 3, "two Sony M01 XMLs and one Canon XML");

    let input: Vec<(ClipFiles, Option<&ClipMeta>)> =
        s.clips.iter().cloned().map(|c| (c, None)).collect();
    let groups = resolve_identity(&[root.to_path_buf()], &input);

    // folder → (camera, reel, clip count, camera source, reel source)
    let mut cards: BTreeMap<String, (String, String, usize, IdentitySource, IdentitySource)> =
        BTreeMap::new();
    for cam in &groups {
        eprintln!("{}", cam.name);
        for card in &cam.cards {
            eprintln!("  {}  [{}]", card.reel, card.folder);
            for c in &card.clips {
                let name = input[c.index].0.main.file_name().unwrap().to_string_lossy();
                eprintln!(
                    "    {name}  camera: {}, reel: {}",
                    c.camera_source.label(),
                    c.reel_source.label()
                );
            }
            let first = card.clips[0];
            let prev = cards.insert(
                card.folder.clone(),
                (
                    cam.name.clone(),
                    card.reel.clone(),
                    card.clips.len(),
                    first.camera_source,
                    first.reel_source,
                ),
            );
            assert!(prev.is_none(), "folder {} split", card.folder);
        }
    }

    let want: &[(&str, &str, &str, usize, IdentitySource, IdentitySource)] = &[
        (
            "A_CAM/A001",
            "A",
            "A0001 (A_CAM/A001)",
            1,
            FileName,
            FileName,
        ),
        (
            "A_CAM/A002",
            "A",
            "A0001 (A_CAM/A002)",
            1,
            FileName,
            FileName,
        ),
        (
            "A_CAM/A003",
            "A",
            "A0001 (A_CAM/A003)",
            1,
            FileName,
            FileName,
        ),
        ("BURANO", "BURANO", "BURANO", 1, FolderName, FolderName),
        ("B_CAM/B005", "B", "B005", 1, FileName, FileName),
        ("C_CAM/C004", "C", "C004", 1, Folder, Folder),
        ("D_CAM/D001", "D", "D001", 1, FileName, FileName),
        ("F_CAM/F003", "F", "F003", 3, FileName, FileName),
        ("M_CAM/M001", "M", "M001", 3, FileName, FileName),
        ("P_CAM", "P", "P_CAM", 1, Folder, FolderName),
        ("R3D", "A", "A005", 1, FileName, FileName),
        ("RONIN-4D", "A", "A001 (RONIN-4D)", 1, FileName, FileName),
        ("S_cam/S001", "S", "S0001", 1, FileName, FileName),
        ("S_cam/S002", "S", "S002", 1, Folder, Folder),
        ("S_cam/S003", "S", "S003", 1, Folder, Folder),
        (
            "S_cam/S005",
            "A",
            "A0001 (S_cam/S005)",
            1,
            FileName,
            FileName,
        ),
        ("S_cam/S006", "A", "A003", 1, FileName, FileName),
        ("S_cam/s004", "S", "S0004", 1, FileName, FileName),
        ("T_CAM", "DW", "DW0001", 2, FileName, FileName),
        ("canon/c70", "A", "A001 (canon/c70)", 1, FileName, FileName),
        ("nikon", "nikon", "nikon", 2, FolderName, FolderName),
    ];
    let got: Vec<_> = cards
        .iter()
        .map(|(f, (c, r, n, cs, rs))| (f.as_str(), c.as_str(), r.as_str(), *n, *cs, *rs))
        .collect();
    assert_eq!(got, want);

    let order: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(
        order,
        [
            "A", "B", "BURANO", "C", "D", "DW", "F", "M", "nikon", "P", "S"
        ]
    );
}
