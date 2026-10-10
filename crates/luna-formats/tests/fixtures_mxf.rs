//! Header-metadata slices of real camera files (no essence): what each
//! camera's MXF header says on its own.

use std::path::PathBuf;

use luna_formats::{ClipFiles, Codec, Container, Gamut, ProResProfile, Probe, Transfer, probe};

fn fixture(name: &str) -> Probe {
    let main = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mxf")
        .join(name);
    probe(&ClipFiles {
        container: Container::Mxf,
        main,
        segments: vec![],
        sidecars: vec![],
    })
    .expect(name)
}

fn s(v: &Option<String>) -> Option<&str> {
    v.as_deref()
}

#[test]
fn alexa35_prores() {
    let p = fixture("alexa35_prores_header.bin");
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
    let m = &p.meta;
    assert_eq!(s(&m.camera_make), Some("ARRI"));
    assert_eq!(s(&m.camera_model), Some("ALEXA 35"));
    assert_eq!(s(&m.camera_serial), Some("70253"));
    assert_eq!(s(&m.lens), Some("ARRI SZ45-135 T2.8"));
    assert_eq!(s(&m.embedded_reel), Some("DW0001_1I7H"));
    assert_eq!(s(&m.embedded_camera_id), Some("DW"));
    assert_eq!(m.start_timecode.unwrap().to_string(), "13:54:12:16");
    assert_eq!(s(&m.recording_date), Some("2025-10-20 13:54:13"));
    assert_eq!((m.width, m.height), (Some(3840), Some(2160)));
    assert_eq!(m.colour.transfer, Some(Transfer::LogC4));
    assert_eq!(m.colour.gamut, Some(Gamut::ArriWideGamut4));
    // The open header carries no duration; the slice has no frames.
    assert_eq!(m.frame_count, None);
    assert!(!p.warnings.is_empty());
}

#[test]
fn alexa35_arriraw() {
    let p = fixture("alexa35_arriraw_header.bin");
    assert_eq!(p.codec, Codec::ArriRaw);
    assert_eq!(s(&p.meta.lens), Some("ARRI SZ65-300 T2.8"));
    assert_eq!(
        (p.layout.stored_width, p.layout.stored_height),
        (2048, 1152)
    );
}

#[test]
fn mini_lf_prores() {
    let p = fixture("minilf_prores_header.bin");
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
    assert_eq!(s(&p.meta.camera_model), Some("ALEXA Mini LF"));
    assert_eq!(p.meta.colour.transfer, Some(Transfer::LogC3));
    assert_eq!(p.meta.colour.gamut, Some(Gamut::ArriWideGamut3));
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "00:42:45:19");
}

#[test]
fn burano_xocn() {
    let p = fixture("burano_xocn_header.bin");
    assert_eq!(p.codec, Codec::XOcn);
    let m = &p.meta;
    assert_eq!(s(&m.camera_make), Some("Sony"));
    // Identification names the recorder, not the camera.
    assert_eq!(m.camera_model, None);
    assert_eq!(m.frame_count, Some(269));
    assert_eq!(m.start_timecode.unwrap().to_string(), "12:30:06:03");
    assert_eq!((m.width, m.height), (Some(6052), Some(3192)));
    // Sony stamps UTC; it is shown as such (see the corpus test).
    assert_eq!(s(&m.recording_date), Some("2024-10-24 20:28:24 UTC"));
}

#[test]
fn venice2_xocn_anamorphic() {
    let p = fixture("venice2_xocn_header.bin");
    assert_eq!(p.codec, Codec::XOcn);
    assert_eq!(p.meta.frame_count, Some(718));
    assert_eq!((p.meta.width, p.meta.height), (Some(8640), Some(5760)));
    let par = p.layout.pixel_aspect.unwrap();
    assert_eq!((par.num, par.den), (9, 5));
}

#[test]
fn fx6_xavc() {
    let p = fixture("fx6_xavc_header.bin");
    assert_eq!(p.codec, Codec::H264);
    let m = &p.meta;
    assert_eq!(m.frame_count, Some(886));
    assert_eq!(m.start_timecode.unwrap().to_string(), "01:10:07:44");
    assert_eq!(m.colour.transfer, Some(Transfer::SLog3));
    assert_eq!(m.colour.gamut, Some(Gamut::SGamut3Cine));
    assert_eq!((m.width, m.height), (Some(4096), Some(2160)));
}

#[test]
fn c50_xfavc() {
    let p = fixture("c50_xfavc_header.bin");
    assert_eq!(p.codec, Codec::H264);
    let m = &p.meta;
    assert_eq!(s(&m.camera_make), Some("Canon"));
    assert_eq!(s(&m.camera_model), Some("EOS C50"));
    assert_eq!(m.frame_count, Some(240));
    assert_eq!(m.start_timecode.unwrap().to_string(), "13:11:22:12");
    assert_eq!(s(&m.recording_date), Some("2025-01-07 22:40:21"));
}
