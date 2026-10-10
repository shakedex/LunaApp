//! Golden tests over the camera corpus. Skipped unless `LUNA_CORPUS` points at
//! the CAMERA folder of the test project (camera files never enter the repo).

use std::path::PathBuf;

use luna_formats::{
    ByteRange, ClipFiles, ClipMeta, Codec, ColourHint, Container, FrameIndex, Gamut, PictureLayout,
    ProResProfile, Probe, Rational, SidecarKind, Timecode, Transfer, probe,
};

fn probe_corpus(rel: &str, sidecar: Option<(SidecarKind, &str)>) -> Option<Probe> {
    let root = PathBuf::from(std::env::var_os("LUNA_CORPUS")?);
    let clip = ClipFiles {
        container: Container::Mxf,
        main: root.join(rel),
        segments: vec![],
        sidecars: sidecar
            .map(|(k, p)| vec![(k, root.join(p))])
            .unwrap_or_default(),
    };
    Some(probe(&clip).expect(rel))
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn tc(frames: u64, base: u32) -> Option<Timecode> {
    Some(Timecode {
        frames,
        base,
        drop_frame: false,
    })
}

fn fps(num: u32, den: u32) -> Option<Rational> {
    Some(Rational { num, den })
}

fn colour(t: Transfer, g: Gamut) -> ColourHint {
    ColourHint {
        transfer: Some(t),
        gamut: Some(g),
    }
}

fn layout(w: u32, h: u32, raw_bits: Option<u8>, par: (u32, u32)) -> PictureLayout {
    PictureLayout {
        stored_width: w,
        stored_height: h,
        raw_bits,
        pixel_aspect: Some(Rational {
            num: par.0,
            den: par.1,
        }),
    }
}

/// Frame count and the first and last frame's byte range.
fn frames(p: &Probe) -> (usize, ByteRange, ByteRange) {
    let FrameIndex::Ranges(r) = &p.frames else {
        panic!("expected frame ranges, got {:?}", p.frames);
    };
    (r.len(), r[0], *r.last().unwrap())
}

fn br(offset: u64, len: u64) -> ByteRange {
    ByteRange { offset, len }
}

fn alexa35_prores_meta() -> ClipMeta {
    ClipMeta {
        recording_date: s("2025-10-20 13:54:13"),
        start_timecode: tc(1_201_264, 24),
        frame_rate: fps(24, 1),
        frame_count: Some(176),
        width: Some(3840),
        height: Some(2160),
        camera_make: s("ARRI"),
        camera_model: s("ALEXA 35"),
        camera_serial: s("70253"),
        lens: s("ARRI SZ45-135 T2.8"),
        focal_length: s("45 mm"),
        stop: s("T5.2"),
        iso: s("800"),
        shutter: s("172.8°"),
        white_balance: s("5600 K"),
        nd: s("1.2"),
        embedded_reel: s("DW0001_1I7H"),
        embedded_camera_id: s("DW"),
        colour: colour(Transfer::LogC4, Gamut::ArriWideGamut4),
    }
}

#[test]
fn alexa35_prores_4444() {
    let Some(p) = probe_corpus("A_CAM/A001/A0001C027_251020_135413_p1I7H.mxf", None) else {
        return;
    };
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
    assert_eq!(p.meta, alexa35_prores_meta());
    assert_eq!(p.layout, layout(3840, 2160, None, (1, 1)));
    assert_eq!(
        frames(&p),
        (176, br(16_789_504, 4_845_576), br(875_294_720, 4_878_907))
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    // The A002/A003 copies read the same.
    for copy in [
        "A_CAM/A002/A0001C027_251020_135413_p1I7H.mxf",
        "A_CAM/A003/A0001C027_251020_135413_p1I7H.mxf",
    ] {
        assert_eq!(probe_corpus(copy, None).unwrap(), p);
    }
}

#[test]
fn alexa35_arriraw() {
    let Some(p) = probe_corpus("T_CAM/DW0001C021_251020_132307_a1I7H_arri-raw.mxf", None) else {
        return;
    };
    assert_eq!(p.codec, Codec::ArriRaw);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2025-10-20 13:23:07"),
            start_timecode: tc(1_156_457, 24),
            frame_rate: fps(24, 1),
            frame_count: Some(165),
            width: Some(2048),
            height: Some(1152),
            camera_make: s("ARRI"),
            camera_model: s("ALEXA 35"),
            camera_serial: s("70253"),
            lens: s("ARRI SZ65-300 T2.8"),
            focal_length: s("65 mm"),
            stop: s("T5"),
            iso: s("800"),
            shutter: s("172.8°"),
            white_balance: s("5600 K"),
            nd: s("1.2"),
            embedded_reel: s("DW0001_1I7H"),
            embedded_camera_id: s("DW"),
            colour: colour(Transfer::LogC4, Gamut::ArriWideGamut4),
        }
    );
    assert_eq!(p.layout, layout(2048, 1152, Some(13), (1, 1)));
    let (n, first, last) = frames(&p);
    assert_eq!(
        (n, first, last),
        (165, br(16_797_720, 3_833_896), br(654_282_776, 3_833_896))
    );
    // 40 bytes of 0xFF before the 13-bit image.
    assert_eq!(
        luna_formats::arriraw_image_offset(first.len, 2048, 1152, 13),
        Some(40)
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn alexa35_arricore() {
    let Some(p) = probe_corpus("T_CAM/DW0001C014_251020_123457_c1I7H-arricore.mxf", None) else {
        return;
    };
    assert_eq!(p.codec, Codec::ArriCore);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2025-10-20 12:34:57"),
            start_timecode: tc(1_087_110, 24),
            frame_rate: fps(24, 1),
            frame_count: Some(158),
            width: Some(3840),
            height: Some(1608),
            camera_make: s("ARRI"),
            camera_model: s("ALEXA 35"),
            camera_serial: s("70253"),
            lens: s("ARRI SZ45-135 T2.8"),
            focal_length: s("45 mm"),
            stop: s("T6.5"),
            iso: s("800"),
            shutter: s("172.8°"),
            white_balance: s("5600 K"),
            nd: s("1.2"),
            embedded_reel: s("DW0001_1I7H"),
            embedded_camera_id: s("DW"),
            colour: colour(Transfer::LogC4, Gamut::ArriWideGamut4),
        }
    );
    assert_eq!(p.layout, layout(3840, 1608, None, (1, 1)));
    assert_eq!(
        frames(&p),
        (158, br(16_797_720, 4_481_000), br(728_035_352, 4_481_000))
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

fn mini_lf(lens: &str, focal: &str, wb: &str, date: &str, tc_frames: u64, count: u64) -> ClipMeta {
    ClipMeta {
        recording_date: s(date),
        start_timecode: tc(tc_frames, 24),
        frame_rate: fps(24, 1),
        frame_count: Some(count),
        width: Some(3840),
        height: Some(2160),
        camera_make: s("ARRI"),
        camera_model: s("ALEXA Mini LF"),
        camera_serial: None,
        lens: s(lens),
        focal_length: s(focal),
        stop: None,
        iso: s("800"),
        shutter: s("172.8°"),
        white_balance: s(wb),
        nd: None,
        embedded_reel: s("F003MN99"),
        embedded_camera_id: s("F"),
        colour: colour(Transfer::LogC3, Gamut::ArriWideGamut3),
    }
}

#[test]
fn alexa_mini_lf_prores_4444() {
    let cases = [
        (
            "F_CAM/F003/F003C004_190925_MN99.mxf",
            mini_lf(
                "Zeiss CZ.2 70-200 01",
                "200 mm",
                "5600 K",
                "2019-09-25 10:00:20",
                61_579,
                251,
            ),
            (1, 1),
            (251, br(950_272, 4_369_201), br(1_101_217_792, 4_398_910)),
        ),
        (
            "F_CAM/F003/F003C005_190925_MN99.mxf",
            mini_lf(
                "ARRI SP47 T1.8",
                "47 mm",
                "6490 K",
                "2019-09-25 10:18:07",
                87_186,
                338,
            ),
            (1, 1),
            (338, br(950_272, 3_857_479), br(1_378_119_680, 4_164_776)),
        ),
        (
            // Zeiss Master Anamorphic: the descriptor states a 2:1 pixel aspect.
            "F_CAM/F003/F003C009_190925_MN99.mxf",
            mini_lf(
                "Zeiss MA100 T1.9",
                "100 mm",
                "6490 K",
                "2019-09-25 10:34:23",
                110_619,
                247,
            ),
            (2, 1),
            (247, br(950_272, 5_096_624), br(1_244_344_320, 4_996_799)),
        ),
    ];
    for (rel, meta, par, fr) in cases {
        let Some(p) = probe_corpus(rel, None) else {
            return;
        };
        assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444), "{rel}");
        assert_eq!(p.meta, meta, "{rel}");
        assert_eq!(p.layout, layout(3840, 2160, None, par), "{rel}");
        assert_eq!(frames(&p), fr, "{rel}");
        assert!(p.warnings.is_empty(), "{rel}: {:?}", p.warnings);
    }
}

const SONY_UTC: &str = "The recording time is stored in UTC only";

#[test]
fn burano_xocn_with_xml() {
    let Some(p) = probe_corpus(
        "BURANO/BURANO XOCN LT Clip 2.mxf",
        Some((SidecarKind::SonyXml, "BURANO/BURANO XOCN LT Clip 2M01.xml")),
    ) else {
        return;
    };
    assert_eq!(p.codec, Codec::XOcn);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: None,
            start_timecode: tc(1_080_147, 24),
            frame_rate: fps(24, 1),
            frame_count: Some(269),
            width: Some(6052),
            height: Some(3192),
            camera_make: s("Sony"),
            camera_model: s("BURANO"),
            camera_serial: s("1000389"),
            lens: s("2050.0212"),
            focal_length: s("50 mm"),
            stop: s("T1.9"),
            iso: s("800"),
            shutter: s("180°"),
            white_balance: s("3600 K"),
            nd: s("0.6"),
            embedded_reel: None,
            embedded_camera_id: None,
            colour: colour(Transfer::Linear, Gamut::SGamut3Cine),
        }
    );
    assert_eq!(p.layout, layout(6080, 3208, None, (1, 1)));
    assert_eq!(
        frames(&p),
        (269, br(11_800, 4_368_360), br(1_180_618_264, 4_368_360))
    );
    assert_eq!(p.warnings.len(), 1);
    assert!(p.warnings[0].starts_with(SONY_UTC));
}

#[test]
fn venice_xocn_with_xml() {
    let Some(p) = probe_corpus(
        "B_CAM/B005/B005C039_201101OZ.mxf",
        Some((SidecarKind::SonyXml, "B_CAM/B005/B005C039_201101OZM01.xml")),
    ) else {
        return;
    };
    assert_eq!(p.codec, Codec::XOcn);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: None,
            start_timecode: tc(419_016, 24),
            frame_rate: fps(24000, 1001),
            frame_count: Some(79),
            width: Some(6048),
            height: Some(4032),
            camera_make: s("Sony"),
            camera_model: s("VENICE"),
            camera_serial: s("0010658"),
            lens: s("F08000183"),
            focal_length: s("86 mm"),
            stop: s("T7.8"),
            iso: s("500"),
            shutter: s("180°"),
            white_balance: s("6300 K"),
            nd: s("Clear"),
            embedded_reel: None,
            embedded_camera_id: None,
            colour: colour(Transfer::Linear, Gamut::SGamut3Cine),
        }
    );
    assert_eq!(p.layout, layout(6080, 4056, None, (1, 1)));
    assert_eq!(
        frames(&p),
        (79, br(11_800, 5_523_432), br(433_716_760, 5_523_432))
    );
    assert_eq!(p.warnings.len(), 1);
    assert!(p.warnings[0].starts_with(SONY_UTC));
}

#[test]
fn venice2_xocn_anamorphic_truncated() {
    let Some(p) = probe_corpus("P_CAM/VENICE 2 8K 1.8x Anamorphic.mxf", None) else {
        return;
    };
    assert_eq!(p.codec, Codec::XOcn);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: None,
            start_timecode: tc(1_079_731, 24),
            frame_rate: fps(24, 1),
            frame_count: Some(718),
            width: Some(8640),
            height: Some(5760),
            camera_make: s("Sony"),
            camera_model: s("VENICE 2"),
            camera_serial: s("0010119"),
            lens: s("7085.2501"),
            focal_length: s("85 mm"),
            stop: s("T2.8"),
            iso: s("1250"),
            shutter: s("180°"),
            white_balance: s("3600 K"),
            nd: s("0.3"),
            embedded_reel: None,
            embedded_camera_id: None,
            colour: colour(Transfer::Linear, Gamut::SGamut3Cine),
        }
    );
    assert_eq!(p.layout, layout(8664, 5784, None, (9, 5)));
    // The 11 GB file stops part-way: 597 of 718 frames are whole.
    assert_eq!(
        frames(&p),
        (597, br(444_952, 19_054_568), br(11_378_952_728, 19_054_568))
    );
    assert!(p.warnings.iter().any(|w| w.contains("597 of 718")));
    assert!(p.warnings.iter().any(|w| w.starts_with(SONY_UTC)));
}

#[test]
fn fx6_xavc_intra() {
    let Some(p) = probe_corpus("S_cam/S003/Filmplusgear-FX6-testfile-1-254_0110.MXF", None) else {
        return;
    };
    assert_eq!(p.codec, Codec::H264);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: None,
            start_timecode: tc(210_394, 50),
            frame_rate: fps(50, 1),
            frame_count: Some(886),
            width: Some(4096),
            height: Some(2160),
            camera_make: s("Sony"),
            camera_model: s("ILME-FX6V"),
            camera_serial: s("4000254"),
            lens: None,
            focal_length: None,
            stop: None,
            iso: s("12800"),
            shutter: s("180°"),
            white_balance: s("5600 K"),
            nd: s("Clear"),
            embedded_reel: None,
            embedded_camera_id: None,
            colour: colour(Transfer::SLog3, Gamut::SGamut3Cine),
        }
    );
    assert_eq!(p.layout, layout(4096, 2160, None, (1, 1)));
    assert_eq!(
        frames(&p),
        (886, br(1_049_108, 1_260_032), br(1_147_895_828, 1_260_032))
    );
    assert_eq!(p.warnings.len(), 1);
    assert!(p.warnings[0].starts_with(SONY_UTC));
}

#[test]
fn canon_c50_xf_avc() {
    let Some(p) = probe_corpus("S_cam/s004/s_0004C062A250107_2240217S_CANON.MXF", None) else {
        return;
    };
    assert_eq!(p.codec, Codec::H264);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2025-01-07 22:40:21"),
            start_timecode: tc(1_187_062, 25),
            frame_rate: fps(25, 1),
            frame_count: Some(240),
            width: Some(3840),
            height: Some(2160),
            camera_make: s("Canon"),
            camera_model: s("EOS C50"),
            camera_serial: s("123456789012"),
            lens: None,
            focal_length: None,
            stop: None,
            iso: s("800"),
            shutter: s("180°"),
            white_balance: s("5600 K"),
            nd: None,
            embedded_reel: None,
            embedded_camera_id: None,
            colour: ColourHint::default(),
        }
    );
    assert_eq!(p.layout, layout(3840, 2160, None, (1, 1)));
    // Long GOP: display frame 0 is a leading B frame (coded second).
    assert_eq!(
        frames(&p),
        (240, br(6_312_980, 337_646), br(179_702_804, 1_151_799))
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}
