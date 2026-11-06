//! ARRI frame-file sequences and ARRIRAW HDE over the new-camera clips beside
//! the corpus (`$LUNA_CORPUS/../new_cameras`). Skipped when they're missing.

use std::path::PathBuf;

use luna_formats::{ClipFiles, Codec, Container, FrameIndex, Gamut, Transfer, probe};

fn new_cameras() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("LUNA_CORPUS")?).join("../new_cameras");
    p.is_dir().then_some(p).or_else(|| {
        eprintln!("skipping: new_cameras not found");
        None
    })
}

#[test]
fn alexa_mini_lf_arx_sequence() {
    let Some(root) = new_cameras() else {
        return;
    };
    let dir = root.join("minilf/F004C028_190925_MN99-arx");
    let mut frames: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "arx"))
        .collect();
    frames.sort();
    let clip = ClipFiles {
        container: Container::AriFrame,
        main: frames[0].clone(),
        segments: frames[1..].to_vec(),
        sidecars: vec![],
    };
    let p = probe(&clip).unwrap();
    assert_eq!(p.codec, Codec::ArriRawHde);
    assert!(matches!(&p.frames, FrameIndex::Files(f) if f.len() == 180));
    // values as ARRI's `art-cmd export` reads them for this clip
    let m = &p.meta;
    assert_eq!(m.recording_date.as_deref(), Some("2019-09-25 13:06:00"));
    let tc = m.start_timecode.unwrap();
    assert_eq!(
        (tc.frames, tc.base),
        ((3 * 3600 + 48 * 60 + 27) * 24 + 11, 24)
    );
    assert_eq!(m.frame_count, Some(180));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    assert_eq!((m.width, m.height), (Some(3840), Some(2160)));
    assert_eq!(m.camera_model.as_deref(), Some("ALEXA Mini LF"));
    assert_eq!(m.camera_serial.as_deref(), Some("30141"));
    assert_eq!(m.lens.as_deref(), Some("Zeiss CZ.2 70-200 01"));
    assert_eq!(m.focal_length.as_deref(), Some("70 mm"));
    assert_eq!(m.stop.as_deref(), Some("T6.9"));
    assert_eq!(m.iso.as_deref(), Some("800"));
    assert_eq!(m.white_balance.as_deref(), Some("5240 K"));
    assert_eq!(m.nd.as_deref(), Some("1.2"));
    assert_eq!(m.embedded_reel.as_deref(), Some("F004MN99"));
    assert_eq!(m.embedded_camera_id.as_deref(), Some("F"));
    assert_eq!(m.colour.transfer, Some(Transfer::LogC3));
    assert_eq!(m.colour.gamut, Some(Gamut::ArriWideGamut3));
}

/// The drive's copy of this ALEXA 65 take lacks frames 0078623–0078929.
#[test]
fn alexa_65_sequence_with_missing_frames() {
    let Some(root) = new_cameras() else {
        return;
    };
    let dir = root.join("alexa65/A003C025_150830_R0D0/6560x3100");
    let mut frames: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ari"))
        .collect();
    frames.sort();
    let p = probe(&ClipFiles {
        container: Container::AriFrame,
        main: frames[0].clone(),
        segments: frames[1..].to_vec(),
        sidecars: vec![],
    })
    .unwrap();
    assert!(matches!(&p.frames, FrameIndex::Files(f) if f.len() == 173));
    let tc = p.meta.start_timecode.unwrap();
    assert_eq!(tc.to_string(), "00:54:29:15");
    // TC in to the last frame's own timecode, 00:54:49:14.
    assert_eq!(p.meta.frame_count, Some(480));
    assert_eq!(tc.offset(479).to_string(), "00:54:49:14");
    assert_eq!(
        p.warnings,
        ["Frames missing from the sequence: 0078623–0078929 (307 of 480)"]
    );
}

#[test]
fn hde_in_mxf_is_named_hde() {
    let Some(root) = new_cameras() else {
        return;
    };
    for rel in [
        "minilf/F004C007_190925_MN99-hde.mxf",
        "alexa35extrme/DW0001C021_251020_132307_h1I7H_mxf-hde.mxf",
    ] {
        let clip = ClipFiles {
            container: Container::Mxf,
            main: root.join(rel),
            segments: vec![],
            sidecars: vec![],
        };
        let p = probe(&clip).unwrap();
        assert_eq!(p.codec, Codec::ArriRawHde, "{rel}");
        assert!(p.warnings.is_empty(), "{rel}: {:?}", p.warnings);
    }
}
