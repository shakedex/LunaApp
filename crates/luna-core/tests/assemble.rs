use luna_core::assemble::{clip_fields, output_folder, thumbnails, write_outputs};
use luna_formats::{ClipMeta, Codec, ProResProfile, Rational, Timecode};
use luna_report::Thumbnail;

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
        Err("No decoder for ARRICORE yet".into()),
        Ok(vec![1, 2, 3]),
    ]);
    assert_eq!(
        t[0],
        Thumbnail::Missing {
            reason: "No decoder for ARRICORE yet".into()
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
    let a = output_folder(dir.path(), "Night: Shift?", "2026-10-10 1432");
    assert_eq!(a.file_name().unwrap(), "Night_ Shift_ 2026-10-10 1432");
    std::fs::create_dir(&a).unwrap();
    let b = output_folder(dir.path(), "Night: Shift?", "2026-10-10 1432");
    assert_eq!(b.file_name().unwrap(), "Night_ Shift_ 2026-10-10 1432 (2)");
    let c = output_folder(dir.path(), "", "2026-10-10 1432");
    assert_eq!(c.file_name().unwrap(), "Luna Report 2026-10-10 1432");
}

#[test]
fn outputs_appear_all_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Report 1");
    let files = write_outputs(&target, "Report 1", b"%PDF-1.7", b"<html>", b"csv").unwrap();
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
