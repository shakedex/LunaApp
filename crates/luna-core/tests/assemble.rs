use luna_core::assemble::{clip_fields, output_folder, thumbnails, write_outputs};
use luna_formats::{ClipMeta, Codec, ProResProfile, Rational, Timecode};
use luna_report::Thumbnail;
use std::path::{Path, PathBuf};

fn meta() -> ClipMeta {
    ClipMeta {
        recording_date: Some("2025-10-20 13:54:13".into()),
        start_timecode: Some(Timecode {
            frames: 10 * 3600 * 24,
            base: 24,
            drop_frame: false,
        }),
        frame_rate: Some(Rational { num: 24, den: 1 }),
        frame_count: Some(176),
        width: Some(3840),
        height: Some(2160),
        camera_make: Some("ARRI".into()),
        camera_model: Some("ALEXA 35".into()),
        camera_serial: Some("60123".into()),
        lens: Some("Signature Prime 47".into()),
        focal_length: Some("47 mm".into()),
        stop: Some("T2.8".into()),
        iso: Some("800".into()),
        shutter: Some("180°".into()),
        white_balance: Some("5600 K".into()),
        nd: Some("0.6".into()),
        ..ClipMeta::default()
    }
}

#[test]
fn fields_come_from_metadata() {
    let f = clip_fields(
        "A0001C027.mxf",
        Some(&meta()),
        Some(&Codec::ProRes(ProResProfile::P4444)),
        913_793_108,
    );
    assert_eq!(f.file_name, "A0001C027.mxf");
    assert_eq!(f.recording_date.as_deref(), Some("2025-10-20 13:54:13"));
    assert_eq!(f.tc_in.as_deref(), Some("10:00:00:00"));
    assert_eq!(
        f.tc_out.as_deref(),
        Some("10:00:07:07"),
        "last frame, not one past it"
    );
    assert_eq!(f.duration.as_deref(), Some("00:00:07:08"));
    assert_eq!(f.fps.as_deref(), Some("24"));
    assert_eq!(f.resolution.as_deref(), Some("3840×2160"));
    assert_eq!(f.codec.as_deref(), Some("ProRes 4444"));
    assert_eq!(f.camera_model.as_deref(), Some("ARRI ALEXA 35"));
    assert_eq!(f.camera_serial.as_deref(), Some("60123"));
    assert_eq!(f.iso.as_deref(), Some("800"));
    assert_eq!(f.file_size, Some(913_793_108));
}

#[test]
fn model_already_naming_the_make_is_not_doubled() {
    let mut m = meta();
    m.camera_make = Some("Sony".into());
    m.camera_model = Some("Sony BURANO".into());
    let f = clip_fields("x.mxf", Some(&m), None, 1);
    assert_eq!(f.camera_model.as_deref(), Some("Sony BURANO"));
}

#[test]
fn model_naming_the_brand_skips_the_company_name() {
    let mut m = meta();
    m.camera_make = Some("Blackmagic Design".into());
    m.camera_model = Some("Blackmagic Cinema Camera 6K".into());
    let f = clip_fields("x.braw", Some(&m), None, 1);
    assert_eq!(
        f.camera_model.as_deref(),
        Some("Blackmagic Cinema Camera 6K")
    );
}

#[test]
fn sensor_rate_and_sound_reach_the_report() {
    let mut m = meta();
    m.sensor_fps = Some(luna_formats::Rational { num: 50, den: 1 });
    m.audio = Some(luna_formats::Audio {
        channels: 2,
        sample_rate: Some(48_000),
        bits: Some(24),
    });
    let f = clip_fields("x.mxf", Some(&m), None, 1);
    assert_eq!(f.sensor_fps.as_deref(), Some("50"));
    assert_eq!(f.audio.as_deref(), Some("2 ch, 48 kHz, 24-bit"));
}

#[test]
fn unknown_metadata_stays_missing() {
    let f = clip_fields("x.mov", None, None, 10);
    assert_eq!(f.file_name, "x.mov");
    assert!(f.tc_in.is_none() && f.duration.is_none() && f.codec.is_none());
    assert_eq!(f.file_size, Some(10));
}

#[test]
fn duration_without_timecode_uses_frame_rate() {
    let mut m = meta();
    m.start_timecode = None;
    m.frame_rate = Some(Rational {
        num: 24000,
        den: 1001,
    });
    m.frame_count = Some(560);
    let f = clip_fields("x.mov", Some(&m), None, 1);
    assert!(f.tc_in.is_none() && f.tc_out.is_none());
    assert_eq!(f.duration.as_deref(), Some("00:00:23:08"));
    assert_eq!(f.fps.as_deref(), Some("23.976"));
}

#[test]
fn thumbnails_keep_order_and_reasons() {
    let t = thumbnails(vec![
        Err("No decoder for ProRes RAW yet".into()),
        Ok(vec![1, 2, 3]),
    ]);
    assert_eq!(
        t[0],
        Thumbnail::Missing {
            reason: "No decoder for ProRes RAW yet".into()
        }
    );
    assert_eq!(
        t[1],
        Thumbnail::Missing {
            reason: "Thumbnail could not be read".into()
        },
        "bytes that don't decode never reach the writers"
    );
}

#[test]
fn output_folder_is_named_and_never_collides() {
    let dir = tempfile::tempdir().unwrap();
    let a = output_folder(dir.path(), "Night: Shift?", None, "2026-10-10 1432");
    assert_eq!(a.file_name().unwrap(), "Night_ Shift_ 2026-10-10 1432");
    std::fs::create_dir(&a).unwrap();
    let b = output_folder(dir.path(), "Night: Shift?", None, "2026-10-10 1432");
    assert_eq!(b.file_name().unwrap(), "Night_ Shift_ 2026-10-10 1432 (2)");
    let c = output_folder(dir.path(), "", None, "2026-10-10 1432");
    assert_eq!(c.file_name().unwrap(), "Luna Report 2026-10-10 1432");
}

#[test]
fn outputs_appear_all_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Report 1");
    let files = write_outputs(&target, b"%PDF-1.7", b"<html>", b"csv").unwrap();
    assert_eq!(std::fs::read(&files.pdf).unwrap(), b"%PDF-1.7");
    assert_eq!(files.html, target.join("Report 1.html"));
    assert_eq!(files.csv, target.join("Report 1.csv"));
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers,
        vec![std::ffi::OsString::from("Report 1")],
        "no temp folder left behind"
    );
}

#[test]
fn a_name_taken_meanwhile_gets_the_next_number() {
    // Another Luna window wrote "Day 1" after this run picked the name.
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Day 1");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("Day 1.pdf"), b"theirs").unwrap();
    let files = write_outputs(&target, b"ours", b"<html>", b"csv").unwrap();
    assert_eq!(files.folder, dir.path().join("Day 1 (2)"));
    assert_eq!(
        files.pdf,
        dir.path().join("Day 1 (2)").join("Day 1 (2).pdf")
    );
    assert_eq!(std::fs::read(target.join("Day 1.pdf")).unwrap(), b"theirs");
    // A numbered name continues its numbering.
    let again = write_outputs(&dir.path().join("Day 1 (2)"), b"x", b"x", b"x").unwrap();
    assert_eq!(again.folder, dir.path().join("Day 1 (3)"));
}

#[test]
fn runs_writing_at_once_each_get_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Day 1");
    let folders: Vec<_> = std::thread::scope(|s| {
        let runs: Vec<_> = (0..4)
            .map(|_| {
                s.spawn(|| {
                    write_outputs(&target, b"pdf", b"html", b"csv")
                        .unwrap()
                        .folder
                })
            })
            .collect();
        runs.into_iter().map(|r| r.join().unwrap()).collect()
    });
    let mut names: Vec<_> = folders
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Day 1", "Day 1 (2)", "Day 1 (3)", "Day 1 (4)"]);
    let entries = std::fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(entries, 4, "no temp folders left behind");
}

#[test]
fn name_sets_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let a = output_folder(dir.path(), "Prod", Some("A Cam"), "2026-10-16 1200");
    assert_eq!(a.file_name().unwrap(), "A Cam");
    std::fs::create_dir(&a).unwrap();
    let b = output_folder(dir.path(), "Prod", Some("A Cam"), "2026-10-16 1200");
    assert_eq!(b.file_name().unwrap(), "A Cam (2)");
    let c = output_folder(dir.path(), "Prod", Some("A/B"), "2026-10-16 1200");
    assert_eq!(c.file_name().unwrap(), "A_B");
}

#[cfg(windows)]
#[test]
fn drive_root_is_the_letter_or_the_share() {
    use luna_core::assemble::drive_root;
    assert_eq!(
        drive_root(Path::new(r"E:\SHOOT\A001")),
        PathBuf::from(r"E:\")
    );
    assert_eq!(
        drive_root(Path::new(r"\\?\E:\SHOOT")),
        PathBuf::from(r"\\?\E:\")
    );
    assert_eq!(
        drive_root(Path::new(r"\\nas\media\SHOOT")),
        PathBuf::from(r"\\nas\media\")
    );
}

#[cfg(unix)]
#[test]
fn drive_root_is_the_volume_or_the_startup_disk() {
    use luna_core::assemble::drive_root;
    assert_eq!(
        drive_root(Path::new("/Volumes/MASTER_01/SHOOT/A001")),
        PathBuf::from("/Volumes/MASTER_01")
    );
    assert_eq!(
        drive_root(Path::new("/Users/dana/Footage")),
        PathBuf::from("/")
    );
}

#[cfg(unix)]
#[test]
fn copies_go_to_other_volumes_never_the_startup_disk() {
    use luna_core::assemble::copy_drives;
    let roots = [
        PathBuf::from("/Volumes/CARD_A/A001"),
        PathBuf::from("/Volumes/CARD_A/A002"),
        PathBuf::from("/Volumes/CARD_B/B001"),
        PathBuf::from("/Users/dana/Footage/C001"),
    ];
    // Report on the Mac: each card once; the Mac's own footage gets none.
    assert_eq!(
        copy_drives(
            Path::new("/Users/dana/Documents/Luna Reports/Day 1"),
            &roots
        ),
        [
            PathBuf::from("/Volumes/CARD_A"),
            PathBuf::from("/Volumes/CARD_B")
        ]
    );
    // Report on CARD_B: CARD_A only; still nothing on the startup disk.
    assert_eq!(
        copy_drives(Path::new("/Volumes/CARD_B/Luna/Day 1"), &roots),
        [PathBuf::from("/Volumes/CARD_A")]
    );
}

#[cfg(windows)]
#[test]
fn same_drive_ignores_case_and_the_verbatim_prefix() {
    use luna_core::assemble::same_drive;
    assert!(same_drive(
        Path::new(r"\\?\e:\SHOOT"),
        Path::new(r"E:\Reports")
    ));
    assert!(!same_drive(
        Path::new(r"D:\SHOOT"),
        Path::new(r"C:\Reports")
    ));
}

#[cfg(unix)]
#[test]
fn same_drive_tells_volumes_apart() {
    use luna_core::assemble::same_drive;
    assert!(same_drive(
        Path::new("/Users/dana/Footage"),
        Path::new("/Users/dana/Documents")
    ));
    assert!(!same_drive(
        Path::new("/Volumes/CARD/A001"),
        Path::new("/Users/dana/Documents")
    ));
}
