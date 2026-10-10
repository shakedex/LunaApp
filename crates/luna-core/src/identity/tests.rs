use std::path::{Path, PathBuf};

use luna_formats::{ClipFiles, ClipMeta, Container, Timecode};
use luna_report::IdentitySource::{self, Card, FileName, Folder, FolderName, Metadata};

use super::*;

#[test]
fn file_name_conventions() {
    let cases: &[(&str, Option<(&str, &str)>)] = &[
        // ARRI classic, ALEXA 35
        ("A001C003_200101_R1AB", Some(("A", "A001"))),
        ("F003C004_190925_MN99", Some(("F", "F003"))),
        ("M001C001_161207_R00H", Some(("M", "M001"))),
        ("A001C001_120101_R1AB.0000001", Some(("A", "A001"))),
        ("A0001C027_251020_135413_p1I7H", Some(("A", "A0001"))),
        (
            "DW0001C014_251020_123457_c1I7H-arricore",
            Some(("DW", "DW0001")),
        ),
        (
            "DW0001C021_251020_132307_a1I7H_arri-raw",
            Some(("DW", "DW0001")),
        ),
        // Sony VENICE
        ("B005C039_201101OZ", Some(("B", "B005"))),
        // Canon
        ("A001C004_22032472_CANON", Some(("A", "A001"))),
        ("S_0001C038X241003_1807575U_CANONRAW", Some(("S", "S0001"))),
        ("s_0004C062A250107_2240217S_CANON", Some(("S", "S0004"))),
        ("A_0001C107H250108_061243ND_CANON", Some(("A", "A0001"))),
        // RED, Blackmagic, DJI
        ("A005_W055_0819CM_001", Some(("A", "A005"))),
        ("A005_C055_0819CM", Some(("A", "A005"))),
        ("D001_09150935_C013", Some(("D", "D001"))),
        (
            "A001C0004_210816_0000_4K_ProRes4444_25FPS",
            Some(("A", "A001")),
        ),
        (
            "A003C0011_210922_0000_ronin4d_proresraw",
            Some(("A", "A003")),
        ),
        // No convention: fall through
        ("Filmplusgear-FX6-testfile-1-254_0110", None),
        ("lumix_s5", None),
        ("Slog3 -proress 444", None),
        ("BURANO XOCN LT Clip 2", None),
        ("VENICE 2 8K 1.8x Anamorphic", None),
        ("A001", None),
        ("A001C001", None),
        ("IMG_1234", None),
        ("C0001", None),
        ("a001c001_200101_r1ab", None),
    ];
    for (stem, want) in cases {
        let got = identity_from_file_name(stem);
        let want = want.map(|(c, r)| NameIdentity {
            camera: c.into(),
            reel: r.into(),
        });
        assert_eq!(got, want, "{stem}");
    }
}

#[test]
fn camera_folder_names() {
    for (name, want) in [
        ("A_CAM", Some("A")),
        ("A-CAM", Some("A")),
        ("ACAM", Some("A")),
        ("A Cam", Some("A")),
        ("CAM_A", Some("A")),
        ("cam b", Some("B")),
        ("S_cam", Some("S")),
        ("B Camera", Some("B")),
        ("CAMERA", None),
        ("BURANO", None),
        ("canon", None),
        ("R3D", None),
        ("A001", None),
        ("AB_CAM", None),
    ] {
        assert_eq!(camera_from_folder_name(name).as_deref(), want, "{name}");
    }
}

#[test]
fn reel_like_folder_names() {
    for yes in ["A001", "s004", "DW0001", "B0123"] {
        assert!(looks_like_reel(yes), "{yes}");
    }
    for no in ["c70", "A_CAM", "Card 1", "A00001", "001", "ABC001"] {
        assert!(!looks_like_reel(no), "{no}");
    }
}

#[test]
fn natural_order() {
    let mut v = vec!["A10", "A2", "a1", "B1", "A02"];
    v.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(v, ["a1", "A2", "A02", "A10", "B1"]);
}

// ---- resolve_identity ----

fn clip(path: &Path) -> ClipFiles {
    ClipFiles {
        container: Container::Mxf,
        main: path.to_path_buf(),
        segments: Vec::new(),
        sidecars: Vec::new(),
    }
}

fn meta(camera: Option<&str>, reel: Option<&str>) -> ClipMeta {
    ClipMeta {
        embedded_camera_id: camera.map(Into::into),
        embedded_reel: reel.map(Into::into),
        ..ClipMeta::default()
    }
}

/// A flattened view: (camera, reel, [(file, camera source, reel source)]).
type Flat = Vec<(
    String,
    String,
    Vec<(String, IdentitySource, IdentitySource)>,
)>;

fn flat(groups: &[CameraGroup], clips: &[(ClipFiles, Option<&ClipMeta>)]) -> Flat {
    let mut out = Vec::new();
    for cam in groups {
        for card in &cam.cards {
            let files = card
                .clips
                .iter()
                .map(|c| {
                    let name = clips[c.index].0.main.file_name().unwrap();
                    (
                        name.to_string_lossy().into_owned(),
                        c.camera_source,
                        c.reel_source,
                    )
                })
                .collect();
            out.push((cam.name.clone(), card.reel.clone(), files));
        }
    }
    out
}

fn f(
    cam: &str,
    reel: &str,
    files: &[(&str, IdentitySource, IdentitySource)],
) -> (
    String,
    String,
    Vec<(String, IdentitySource, IdentitySource)>,
) {
    (
        cam.into(),
        reel.into(),
        files
            .iter()
            .map(|(n, c, r)| (n.to_string(), *c, *r))
            .collect(),
    )
}

fn root() -> PathBuf {
    PathBuf::from("/drive")
}

fn at(rel: &str) -> PathBuf {
    root().join(rel)
}

#[test]
fn single_card_as_root_with_camera_names() {
    let root = at("A_CAM/A001");
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![
        (clip(&root.join("A001C002_200101_R1AB.mxf")), None),
        (clip(&root.join("A001C001_200101_R1AB.mxf")), None),
    ];
    let g = resolve_identity(&root, &clips);
    assert_eq!(
        flat(&g, &clips),
        [f(
            "A",
            "A001",
            &[
                ("A001C001_200101_R1AB.mxf", FileName, FileName),
                ("A001C002_200101_R1AB.mxf", FileName, FileName),
            ]
        )]
    );
    assert_eq!(g[0].cards[0].folder, ".");
}

#[test]
fn single_card_as_root_uses_its_own_folder_names() {
    // Picked `.../B_CAM/B007` itself; the parent outside the root still names the camera.
    let root = at("B_CAM/B007");
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![(clip(&root.join("clip.mov")), None)];
    assert_eq!(
        flat(&resolve_identity(&root, &clips), &clips),
        [f("B", "B007", &[("clip.mov", Folder, Folder)])]
    );
    // An arbitrary parent outside the root is not evidence.
    let root = at("Desktop/Card 7");
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![(clip(&root.join("clip.mov")), None)];
    assert_eq!(
        flat(&resolve_identity(&root, &clips), &clips),
        [f(
            "Card 7",
            "Card 7",
            &[("clip.mov", FolderName, FolderName)]
        )]
    );
}

#[test]
fn whole_drive_with_several_cameras() {
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![
        (clip(&at("P_CAM/VENICE 2 8K.mxf")), None),
        (clip(&at("D_CAM/D001/D001_09150935_C013.braw")), None),
        (clip(&at("B_CAM/B005/B005C039_201101OZ.mxf")), None),
        (clip(&at("BURANO/BURANO Clip 2.mxf")), None),
        (clip(&at("A_CAM/A010/A010C001_200101_R1AB.mxf")), None),
        (clip(&at("A_CAM/A002/A002C001_200101_R1AB.mxf")), None),
        (clip(&at("S_cam/S002/lumix_s5.mov")), None),
        (clip(&at("Shoot/X001/generic.mov")), None),
    ];
    assert_eq!(
        flat(&resolve_identity(&root(), &clips), &clips),
        [
            f(
                "A",
                "A002",
                &[("A002C001_200101_R1AB.mxf", FileName, FileName)]
            ),
            f(
                "A",
                "A010",
                &[("A010C001_200101_R1AB.mxf", FileName, FileName)]
            ),
            f(
                "B",
                "B005",
                &[("B005C039_201101OZ.mxf", FileName, FileName)]
            ),
            f(
                "BURANO",
                "BURANO",
                &[("BURANO Clip 2.mxf", FolderName, FolderName)]
            ),
            f(
                "D",
                "D001",
                &[("D001_09150935_C013.braw", FileName, FileName)]
            ),
            f("P", "P_CAM", &[("VENICE 2 8K.mxf", Folder, FolderName)]),
            f("S", "S002", &[("lumix_s5.mov", Folder, Folder)]),
            f("Shoot", "X001", &[("generic.mov", Folder, Folder)]),
        ]
    );
}

#[test]
fn mixed_evidence_in_one_card_borrows_the_strongest() {
    let m = meta(Some("A"), Some("A007"));
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![
        (clip(&at("Day1/Card1/A007C001_200101_R1AB.mxf")), Some(&m)),
        (clip(&at("Day1/Card1/A007C002_200101_R1AB.mxf")), None),
        (clip(&at("Day1/Card1/odd name.mov")), None),
    ];
    assert_eq!(
        flat(&resolve_identity(&root(), &clips), &clips),
        [f(
            "A",
            "A007",
            &[
                ("A007C001_200101_R1AB.mxf", Metadata, Metadata),
                ("A007C002_200101_R1AB.mxf", FileName, FileName),
                ("odd name.mov", Card, Card),
            ]
        )]
    );
}

#[test]
fn metadata_outranks_file_name_and_folder() {
    let m = meta(Some("Z"), Some("Z009"));
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> =
        vec![(clip(&at("A_CAM/A001/A001C001_200101_R1AB.mxf")), Some(&m))];
    assert_eq!(
        flat(&resolve_identity(&root(), &clips), &clips),
        [f(
            "Z",
            "Z009",
            &[("A001C001_200101_R1AB.mxf", Metadata, Metadata)]
        )]
    );
}

#[test]
fn blank_metadata_is_not_evidence() {
    let m = meta(Some("  "), Some(""));
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> =
        vec![(clip(&at("A_CAM/A001/A001C001_200101_R1AB.mxf")), Some(&m))];
    assert_eq!(
        flat(&resolve_identity(&root(), &clips), &clips),
        [f(
            "A",
            "A001",
            &[("A001C001_200101_R1AB.mxf", FileName, FileName)]
        )]
    );
}

#[test]
fn disagreeing_embedded_reels_split_the_folder() {
    let m1 = meta(Some("A"), Some("A001"));
    let m2 = meta(Some("A"), Some("A002"));
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![
        (clip(&at("dump/one.mxf")), Some(&m1)),
        (clip(&at("dump/two.mxf")), Some(&m2)),
        (clip(&at("dump/A002C005_200101_R1AB.mxf")), None),
        (clip(&at("dump/stray.mov")), None),
    ];
    assert_eq!(
        flat(&resolve_identity(&root(), &clips), &clips),
        [
            f("A", "A001", &[("one.mxf", Metadata, Metadata)]),
            f(
                "A",
                "A002",
                &[
                    ("A002C005_200101_R1AB.mxf", FileName, FileName),
                    ("two.mxf", Metadata, Metadata),
                ]
            ),
            f("dump", "dump", &[("stray.mov", FolderName, FolderName)]),
        ]
    );
}

#[test]
fn same_reel_from_different_folders_stays_apart_and_says_where() {
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![
        (
            clip(&at("A_CAM/A003/A0001C027_251020_135413_p1I7H.mxf")),
            None,
        ),
        (
            clip(&at("A_CAM/A001/A0001C027_251020_135413_p1I7H.mxf")),
            None,
        ),
        (
            clip(&at("A_CAM/A002/A0001C027_251020_135413_p1I7H.mxf")),
            None,
        ),
    ];
    let g = resolve_identity(&root(), &clips);
    assert_eq!(g.len(), 1);
    let reels: Vec<_> = g[0].cards.iter().map(|c| c.reel.as_str()).collect();
    assert_eq!(
        reels,
        [
            "A0001 (A_CAM/A001)",
            "A0001 (A_CAM/A002)",
            "A0001 (A_CAM/A003)"
        ]
    );
    let folders: Vec<_> = g[0].cards.iter().map(|c| c.folder.as_str()).collect();
    assert_eq!(folders, ["A_CAM/A001", "A_CAM/A002", "A_CAM/A003"]);
}

#[test]
fn clips_order_by_recording_start_then_file_name() {
    let tc = |frames| Timecode {
        frames,
        base: 24,
        drop_frame: false,
    };
    let early = ClipMeta {
        recording_date: Some("2026-10-09 09:00:00".into()),
        ..ClipMeta::default()
    };
    let late_tc = ClipMeta {
        recording_date: Some("2026-10-10".into()),
        start_timecode: Some(tc(24 * 3600)),
        ..ClipMeta::default()
    };
    let early_tc = ClipMeta {
        recording_date: Some("2026-10-10".into()),
        start_timecode: Some(tc(24 * 60)),
        ..ClipMeta::default()
    };
    let clips: Vec<(ClipFiles, Option<&ClipMeta>)> = vec![
        (clip(&at("X001/c10.mov")), None),
        (clip(&at("X001/c2.mov")), None),
        (clip(&at("X001/late.mov")), Some(&late_tc)),
        (clip(&at("X001/early.mov")), Some(&early)),
        (clip(&at("X001/mid.mov")), Some(&early_tc)),
    ];
    let g = resolve_identity(&root(), &clips);
    let order: Vec<_> = g[0].cards[0]
        .clips
        .iter()
        .map(|c| {
            clips[c.index]
                .0
                .main
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        order,
        ["early.mov", "mid.mov", "late.mov", "c2.mov", "c10.mov"]
    );
}

#[test]
fn nothing_in_nothing_out() {
    assert!(resolve_identity(&root(), &[]).is_empty());
}

fn body(model: &str, serial: Option<&str>) -> ClipMeta {
    ClipMeta {
        camera_model: Some(model.into()),
        camera_serial: serial.map(Into::into),
        ..ClipMeta::default()
    }
}

fn camera_names(groups: &[CameraGroup]) -> Vec<String> {
    groups.iter().map(|c| c.name.clone()).collect()
}

#[test]
fn one_letter_on_different_bodies_splits_into_cameras() {
    let root = PathBuf::from("D:/Shoot");
    let alexa = body("ARRI ALEXA 35", Some("60123"));
    let c70 = body("Canon EOS C70", Some("1234"));
    let clips = vec![
        (
            clip(&root.join("A_CAM/A001/A0001C027_251020_135413_p1I7H.mxf")),
            Some(&alexa),
        ),
        (
            clip(&root.join("canon/c70/A001C004_22032472_CANON.CRM")),
            Some(&c70),
        ),
    ];
    let g = resolve_identity(&root, &clips);
    assert_eq!(camera_names(&g), ["A · ARRI ALEXA 35", "A · Canon EOS C70"]);
}

#[test]
fn same_model_different_serials_name_the_serial() {
    let root = PathBuf::from("D:/Shoot");
    let one = body("ARRI ALEXA 35", Some("60123"));
    let two = body("ARRI ALEXA 35", Some("60999"));
    let clips = vec![
        (
            clip(&root.join("X/A001/A001C001_251020_R1AB.mxf")),
            Some(&one),
        ),
        (
            clip(&root.join("Y/A002/A002C001_251020_R1AB.mxf")),
            Some(&two),
        ),
    ];
    let g = resolve_identity(&root, &clips);
    assert_eq!(
        camera_names(&g),
        ["A · ARRI ALEXA 35 #60123", "A · ARRI ALEXA 35 #60999"]
    );
}

#[test]
fn one_body_or_unknown_bodies_stay_one_camera() {
    let root = PathBuf::from("D:/Shoot");
    let alexa = body("ARRI ALEXA 35", Some("60123"));
    let clips = vec![
        (
            clip(&root.join("A_CAM/A001/A001C001_251020_R1AB.mxf")),
            Some(&alexa),
        ),
        (
            clip(&root.join("A_CAM/A002/A002C001_251020_R1AB.mxf")),
            Some(&alexa),
        ),
        (
            clip(&root.join("A_CAM/A003/A003C001_251020_R1AB.mxf")),
            None,
        ),
    ];
    let g = resolve_identity(&root, &clips);
    assert_eq!(camera_names(&g), ["A"]);
    assert_eq!(g[0].cards.len(), 3);
}
