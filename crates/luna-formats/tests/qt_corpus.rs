//! Golden tests over the camera corpus. Set `LUNA_CORPUS` to the CAMERA
//! folder of the test project to run them; clips that are missing are skipped.
//! Expected values were cross-checked against ffprobe (timecode, frame rate,
//! frame count, packet positions and sizes) and the files' own bytes.

use std::path::PathBuf;

use luna_formats::{
    Audio, ByteRange, ClipFiles, ClipMeta, Codec, ColourHint, Container, FrameIndex, Gamut,
    ProResProfile, Probe, Rational, Timecode, Transfer, probe,
};

fn corpus(rel: &str) -> Option<Probe> {
    let root = std::env::var_os("LUNA_CORPUS")?;
    let clip = ClipFiles {
        container: Container::QuickTime,
        main: PathBuf::from(root).join(rel),
        segments: vec![],
        sidecars: vec![],
    };
    if !clip.main.is_file() {
        eprintln!("skipping: {} not found", clip.main.display());
        return None;
    }
    Some(probe(&clip).unwrap_or_else(|e| panic!("{rel}: {e}")))
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

fn rate(num: u32, den: u32) -> Option<Rational> {
    Some(Rational { num, den })
}

/// Frame count plus the first and last ranges of the index.
fn assert_frames(p: &Probe, count: usize, first: (u64, u64), last: (u64, u64)) {
    let FrameIndex::Ranges(r) = &p.frames else {
        panic!("expected byte ranges")
    };
    assert_eq!(r.len(), count);
    let pair = |b: &ByteRange| (b.offset, b.len);
    assert_eq!(r.first().map(pair), Some(first));
    assert_eq!(r.last().map(pair), Some(last));
}

/// 48 kHz sound with this many channels and, for PCM, this bit depth.
fn audio(channels: u32, bits: Option<u32>) -> Option<Audio> {
    Some(Audio {
        channels,
        sample_rate: Some(48_000),
        bits,
    })
}

fn colour(transfer: Transfer, gamut: Option<Gamut>) -> ColourHint {
    ColourHint {
        transfer: Some(transfer),
        gamut,
    }
}

fn arri(date: &str, lens: Option<String>, start: u64, frames: u64, w: u32, h: u32) -> ClipMeta {
    ClipMeta {
        slate: Default::default(),
        recording_date: s(date),
        start_timecode: tc(start, 24),
        frame_rate: rate(24, 1),
        frame_count: Some(frames),
        width: Some(w),
        height: Some(h),
        camera_make: s("ARRI"),
        camera_model: s("ARRI ALEXA Mini"),
        camera_serial: s("20017"),
        lens,
        focal_length: None,
        stop: None,
        iso: s("800"),
        shutter: s("172.8°"),
        white_balance: s("5600 K"),
        nd: s("1.2"),
        sensor_fps: rate(24, 1),
        audio: audio(5, Some(24)),
        embedded_reel: s("M001R00H"),
        embedded_camera_id: s("M"),
        colour: colour(Transfer::LogC3, Some(Gamut::ArriWideGamut3)),
    }
}

#[test]
fn arri_alexa_mini_c001() {
    let Some(p) = corpus("M_CAM/M001/M001C001_161207_R00H.mov") else {
        return;
    };
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444Xq));
    assert_eq!(
        p.meta,
        arri("2016-12-07 15:08:05", None, 151_391, 254, 1920, 1080)
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "01:45:07:23");
    assert_frames(&p, 254, (2_097_152, 2_015_232), (519_843_840, 2_017_280));
    assert_eq!(p.layout.pixel_aspect, rate(1, 1));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn arri_alexa_mini_c005() {
    let Some(p) = corpus("M_CAM/M001/M001C005_161207_R00H.mov") else {
        return;
    };
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444Xq));
    assert_eq!(
        p.meta,
        arri(
            "2016-12-07 15:13:41",
            s("Fujinon Alura AZ30-80 T2.8"),
            159_453,
            253,
            3840,
            2160
        )
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "01:50:43:21");
    assert_frames(&p, 253, (2_097_152, 7_598_080), (1_925_332_992, 7_602_176));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn arri_alexa_mini_c007() {
    let Some(p) = corpus("M_CAM/M001/M001C007_161207_R00H.mov") else {
        return;
    };
    assert_eq!(
        p.meta,
        arri("2016-12-07 15:18:41", None, 166_657, 243, 2048, 858)
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "01:55:44:01");
    assert_frames(&p, 243, (2_097_152, 1_413_120), (351_553_536, 1_413_120));
    // Already de-squeezed by the camera: square pixels.
    assert_eq!(p.layout.pixel_aspect, rate(1, 1));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn resolve_prores_4444() {
    let Some(p) = corpus("C_CAM/C004/Slog3 -proress 444.mov") else {
        return;
    };
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2024-09-19 20:23:02"),
            start_timecode: tc(86_400, 24),
            frame_rate: rate(24000, 1001),
            frame_count: Some(560),
            width: Some(1920),
            height: Some(1080),
            // The name says S-Log3; the file's colr says Rec.709. The file wins.
            colour: colour(Transfer::Rec709, Some(Gamut::Rec709)),
            audio: audio(2, Some(16)),
            ..ClipMeta::default()
        }
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "01:00:00:00");
    assert_frames(&p, 560, (40, 1_181_248), (643_391_688, 1_131_264));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn ronin_4d_prores_4444() {
    let Some(p) = corpus("RONIN-4D/A001C0004_210816_0000_4K_ProRes4444_25FPS.mov") else {
        return;
    };
    assert_eq!(p.codec, Codec::ProRes(ProResProfile::P4444));
    // A re-wrapped file (Core Media handlers): no camera metadata survives.
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2021-12-17 03:59:55"),
            start_timecode: tc(410, 25),
            frame_rate: rate(25, 1),
            frame_count: Some(250),
            width: Some(4096),
            height: Some(1712),
            colour: colour(Transfer::Rec709, Some(Gamut::Rec709)),
            audio: Some(Audio::default()),
            ..ClipMeta::default()
        }
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "00:00:16:10");
    assert_frames(&p, 250, (36, 5_015_008), (1_186_928_600, 4_578_832));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn ronin_4d_prores_raw() {
    let Some(p) = corpus("S_cam/S006/A003C0011_210922_0000_ronin4d_proresraw.mov") else {
        return;
    };
    assert_eq!(p.codec, Codec::ProResRaw);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2021-09-22 10:03:24"),
            start_timecode: tc(2_226_742, 30),
            frame_rate: rate(60, 1),
            frame_count: Some(705),
            width: Some(8192),
            height: Some(4320),
            camera_make: s("DJI"),
            camera_model: s("DJI HG4180"),
            camera_serial: s("4CVLJ62008001A"),
            lens: s("DJI_DL_35mm_F2.8_ND_ASPH"),
            focal_length: s("35 mm"),
            stop: s("f/4.4"),
            iso: s("800"),
            shutter: s("180°"),
            white_balance: s("5600 K"),
            nd: s("2.4"),
            colour: ColourHint {
                transfer: Some(Transfer::DLog),
                gamut: None,
            },
            audio: audio(2, Some(24)),
            ..ClipMeta::default()
        }
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "20:37:04:22");
    assert_frames(&p, 705, (7_904, 7_998_752), (5_617_734_880, 7_970_592));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn lumix_s5() {
    let Some(p) = corpus("S_cam/S002/lumix_s5.mov") else {
        return;
    };
    assert_eq!(p.codec, Codec::H264);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2022-04-19 22:47:20"),
            start_timecode: tc(75_830, 24),
            frame_rate: rate(24000, 1001),
            frame_count: Some(624),
            width: Some(3840),
            height: Some(2160),
            camera_make: s("Panasonic"),
            camera_model: s("DC-S5"),
            camera_serial: s("WJ0LA001863"),
            iso: s("640"),
            shutter: s("1/60"),
            colour: colour(Transfer::VLog, Some(Gamut::VGamut)),
            audio: audio(2, Some(16)),
            ..ClipMeta::default()
        }
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "00:52:39:14");
    assert_frames(&p, 624, (10_747_968, 1_608_054), (503_739_222, 737_578));
    assert_eq!(p.layout.pixel_aspect, None);
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}

#[test]
fn canon_eos_c50() {
    let Some(p) = corpus("S_cam/S005/A_0001C107H250108_061243ND_CANON.MP4") else {
        return;
    };
    assert_eq!(p.codec, Codec::Hevc);
    assert_eq!(
        p.meta,
        ClipMeta {
            recording_date: s("2025-01-08 06:12:43"),
            start_timecode: tc(360_227, 25),
            frame_rate: rate(25, 1),
            frame_count: Some(732),
            width: Some(4096),
            height: Some(2160),
            camera_make: s("Canon"),
            camera_model: s("Canon EOS C50"),
            // As the camera writes it (Exif BodySerialNumber and RDD 18 agree).
            camera_serial: s("123456789012"),
            lens: s("RF24-105mm F2.8 L IS USM Z"),
            focal_length: s("24 mm"),
            stop: s("f/2.8"),
            iso: s("800"),
            shutter: s("1/50"),
            white_balance: s("5600 K"),
            audio: audio(2, None),
            // Canon's private RDD 18 labels in the `cndm` track.
            colour: colour(Transfer::CanonLog2, Some(Gamut::CinemaGamut)),
            ..ClipMeta::default()
        }
    );
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "04:00:09:02");
    // Long-GOP with B-frames: display order differs from decode order.
    assert_frames(&p, 732, (5_328_290, 384_714), (471_131_493, 781_188));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
}
