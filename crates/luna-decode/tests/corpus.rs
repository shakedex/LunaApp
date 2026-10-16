//! Decodes the real camera corpus. Skipped unless `LUNA_CORPUS` points at the
//! corpus root (`...\TEST_PROJECT_LUNA\CAMERA`). R3D also needs the add-on
//! (`LUNA_R3D_ADDON`, plus `LUNA_R3D_SDK_DIR` when RED's library isn't next to it).

mod common;

use common::{corpus, files, probe};
use luna_decode::{Cfa, DecodeError, Decoded, Frame, decode_frames, vendor_probe};
use luna_formats::{Codec, Container, FrameIndex, Gamut, PictureLayout, ProResProfile, Transfer};

fn mosaic_size(f: &Frame) -> (u32, u32) {
    match &f.image {
        Decoded::Mosaic {
            width,
            height,
            data,
            cfa,
            ..
        } => {
            assert_eq!(data.len(), (*width * *height) as usize);
            assert_eq!(*cfa, Cfa::Rggb);
            (*width, *height)
        }
        other => panic!("expected a mosaic, got {other:?}"),
    }
}

/// Mean of a mosaic's samples, to check the picture isn't empty or noise.
fn mosaic_mean(f: &Frame) -> f64 {
    let Decoded::Mosaic { data, .. } = &f.image else {
        unreachable!()
    };
    data.iter().map(|&v| v as f64).sum::<f64>() / data.len() as f64
}

fn xocn(rel: &str, expect_width: u32) {
    let Some(path) = corpus(rel) else { return };
    let clip = files(Container::Mxf, &path);
    let p = probe(Codec::XOcn, FrameIndex::DecoderOwned, Default::default());
    // first, middle, last, and one past the end (clamped)
    let out = decode_frames(&clip, &p, &[0, 10, u64::MAX], 640);
    assert_eq!(out.len(), 3);
    for r in &out {
        let f = r.as_ref().unwrap_or_else(|e| panic!("{rel}: {e}"));
        let (w, _) = mosaic_size(f);
        assert_eq!(w, expect_width, "{rel}");
        assert!(w >= 1280);
        assert_eq!(f.hint.transfer, Some(Transfer::Linear));
        let Decoded::Mosaic { black, .. } = f.image else {
            unreachable!()
        };
        assert!(mosaic_mean(f) > black as f64, "{rel}: picture is black");
    }
}

#[test]
fn xocn_burano() {
    xocn("BURANO/BURANO XOCN LT Clip 2.mxf", 1512);
}

#[test]
fn xocn_venice2_8k() {
    xocn("P_CAM/VENICE 2 8K 1.8x Anamorphic.mxf", 2158);
}

#[test]
fn xocn_venice_6k() {
    xocn("B_CAM/B005/B005C039_201101OZ.mxf", 1512);
}

fn crm(rel: &str, expect: (u32, u32), model: &str) {
    let Some(path) = corpus(rel) else { return };
    let clip = files(Container::Crm, &path);
    let p = vendor_probe(&clip)
        .expect("crm is a vendor container")
        .unwrap();
    assert_eq!(p.codec, Codec::CinemaRawLight);
    assert_eq!(p.meta.camera_model.as_deref(), Some(model));
    assert!(p.meta.start_timecode.is_some());
    assert!(p.meta.frame_rate.is_some());
    assert!(p.meta.iso.is_some() && p.meta.shutter.is_some() && p.meta.stop.is_some());
    let n = p.meta.frame_count.unwrap();
    let out = decode_frames(&clip, &p, &[0, n / 2, n - 1], 640);
    for r in &out {
        let f = r.as_ref().unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(mosaic_size(f), expect, "{rel}");
        assert!(mosaic_mean(f) > 259.0);
    }
}

#[test]
fn crm_c70() {
    crm(
        "canon/c70/A001C004_22032472_CANON.CRM",
        (2048, 1080),
        "Canon EOS C70",
    );
}

#[test]
fn crm_c400() {
    crm(
        "S_cam/S001/S_0001C038X241003_1807575U_CANONRAW.CRM",
        (1500, 790),
        "Canon EOS C400",
    );
}

#[test]
fn braw_cinema_camera_6k() {
    let Some(path) = corpus("D_CAM/D001/D001_09150935_C013.braw") else {
        return;
    };
    let clip = files(Container::Braw, &path);
    let p = vendor_probe(&clip)
        .expect("braw is a vendor container")
        .unwrap();
    assert_eq!(p.codec, Codec::Braw);
    assert_eq!(
        p.meta.camera_model.as_deref(),
        Some("Blackmagic Cinema Camera 6K")
    );
    assert_eq!(p.meta.frame_count, Some(49));
    assert_eq!(p.meta.iso.as_deref(), Some("400"));
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "09:35:19:06");
    let out = decode_frames(&clip, &p, &[0, 24, 48], 640);
    for r in out {
        let f = r.unwrap();
        let Decoded::Rgb16 {
            width,
            height,
            data,
            transfer,
            gamut,
        } = f.image
        else {
            panic!("expected rgb16")
        };
        assert_eq!((width, height), (1512, 1008));
        assert_eq!(data.len(), (width * height * 3) as usize);
        assert_eq!(
            (transfer, gamut),
            (Transfer::BmdFilmGen5, Gamut::BmdWideGamutGen5)
        );
    }
}

#[test]
fn r3d_through_the_addon() {
    let Some(path) = corpus("R3D/A005_W055_0819CM_001.R3D") else {
        return;
    };
    let clip = files(Container::R3d, &path);
    let probed = vendor_probe(&clip).expect("r3d is a vendor container");
    if std::env::var_os("LUNA_R3D_ADDON").is_none() {
        // No add-on configured: must be reported, not crash.
        if let Err(e) = &probed {
            eprintln!("R3D add-on not configured: {e}");
            let out = decode_frames(
                &clip,
                &probe(Codec::Redcode, FrameIndex::DecoderOwned, Default::default()),
                &[0],
                640,
            );
            assert!(matches!(out[0], Err(DecodeError::Unsupported(_))));
            return;
        }
    }
    let p = probed.unwrap();
    assert_eq!(p.meta.camera_model.as_deref(), Some("V-RAPTOR XE 8K VV"));
    assert_eq!(p.meta.frame_count, Some(284));
    assert_eq!(p.meta.embedded_reel.as_deref(), Some("A005_W055_0819CM"));
    // user_timecode_preference = 1 (absolute) on this clip; edge is 03:33:58:06.
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "15:43:20:20");
    assert_eq!(
        p.meta.recording_date.as_deref(),
        Some("2025-08-19 23:39:20")
    );
    assert_eq!(p.meta.iso.as_deref(), Some("800"));
    assert_eq!(p.meta.white_balance.as_deref(), Some("5600 K"));
    assert_eq!(p.meta.shutter.as_deref(), Some("288°"));
    assert_eq!(p.meta.nd.as_deref(), Some("1.5"));
    assert_eq!(
        p.meta.sensor_fps,
        Some(luna_formats::Rational {
            num: 48000,
            den: 1001
        })
    );
    // Sound went to a separate WAV (wav_filename), so it's unknown, not "None".
    assert_eq!(p.meta.audio, None);
    let out = decode_frames(&clip, &p, &[0, 142, 283], 640);
    for r in out {
        let f = r.unwrap();
        let Decoded::Rgb16 { width, height, .. } = f.image else {
            panic!("expected rgb16")
        };
        assert_eq!((width, height), (1024, 540));
        assert_eq!(f.hint.transfer, Some(Transfer::Log3G10));
    }
}

#[test]
fn nikon_nraw_through_the_addon() {
    let Some(path) = corpus("nikon/05_6K60P_skin_tone_N-Log.NEV") else {
        return;
    };
    let clip = files(Container::Nev, &path);
    let probed = vendor_probe(&clip).expect("nev is a vendor container");
    if std::env::var_os("LUNA_R3D_ADDON").is_none()
        && let Err(e) = &probed
    {
        eprintln!("R3D add-on not configured: {e}");
        return;
    }
    let p = probed.unwrap();
    assert_eq!(p.codec, Codec::NRaw);
    assert_eq!(p.meta.camera_make.as_deref(), Some("Nikon"));
    assert_eq!(p.meta.camera_model.as_deref(), Some("Z6_3"));
    assert_eq!(p.meta.frame_count, Some(306));
    assert_eq!((p.meta.width, p.meta.height), (Some(6048), Some(3402)));
    // The camera's tmcd track (ffprobe) says 03:00:00;00; the SDK's absolute TC is time of day.
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "03:00:00;00");
    assert_eq!(p.meta.stop.as_deref(), Some("f/2.5"));
    assert_eq!(p.meta.iso.as_deref(), Some("800"));
    assert_eq!(p.meta.lens.as_deref(), Some("NIKON NIKKOR Z 85mm f/1.8 S"));
    // No sound track in the file (ffprobe): no audio, not unknown.
    assert_eq!(p.meta.audio.map(|a| a.channels), Some(0));
    let out = decode_frames(&clip, &p, &[0, 153, 305], 640);
    for r in out {
        let f = r.unwrap();
        let Decoded::Rgb16 { width, height, .. } = f.image else {
            panic!("expected rgb16")
        };
        assert_eq!((width, height), (756, 425));
        assert_eq!(f.hint.transfer, Some(Transfer::Log3G10));
    }
}

#[test]
fn arriraw_alexa35_through_the_verified_decoder() {
    let Some(path) = corpus("T_CAM/DW0001C021_251020_132307_a1I7H_arri-raw.mxf") else {
        return;
    };
    let p = probe(
        Codec::ArriRaw,
        FrameIndex::DecoderOwned,
        PictureLayout::default(),
    );
    let out = decode_frames(&files(Container::Mxf, &path), &p, &[0, 82, 164], 640);
    for r in out {
        let f = r.unwrap();
        let Decoded::Rgb16 {
            width,
            transfer,
            gamut,
            ..
        } = f.image
        else {
            panic!("expected LogC4 RGB")
        };
        assert!(width >= 640, "{width}");
        assert_eq!((transfer, gamut), (Transfer::LogC4, Gamut::ArriWideGamut4));
    }
}

#[test]
fn arricore_alexa35_through_the_verified_decoder() {
    let Some(path) = corpus("T_CAM/DW0001C014_251020_123457_c1I7H-arricore.mxf") else {
        return;
    };
    let clip = files(Container::Mxf, &path);
    let p = luna_formats::probe(&clip).unwrap();
    assert_eq!(p.codec, Codec::ArriCore);
    let out = decode_frames(&clip, &p, &[0, 79, 157], 640);
    for r in out {
        let f = r.unwrap();
        let Decoded::Rgb16 {
            width,
            height,
            transfer,
            gamut,
            ref data,
        } = f.image
        else {
            panic!("expected LogC4 RGB")
        };
        // quarter of 3840x1608: the base image binned 2x2
        assert_eq!((width, height), (960, 402));
        assert_eq!((transfer, gamut), (Transfer::LogC4, Gamut::ArriWideGamut4));
        // a lit scene, not a flat or clipped frame
        let mut g: Vec<u16> = data.iter().skip(1).step_by(3).copied().collect();
        g.sort_unstable();
        let (lo, mid, hi) = (g[g.len() / 20], g[g.len() / 2], g[g.len() * 19 / 20]);
        assert!(
            lo > 10_000 && hi < 60_000 && hi - lo > 8_000,
            "{lo} {mid} {hi}"
        );
    }
}

/// First, middle and last frame of an H.264 clip through the real parser
/// and our own intra decoder (directly for MXF, as the platform fallback for
/// MOV).
fn h264_thumbnails(rel: &str, container: Container, size: (u32, u32), full_range: bool) {
    let Some(path) = corpus(rel) else {
        return;
    };
    let clip = files(container, &path);
    let p = luna_formats::probe(&clip).unwrap();
    assert_eq!(p.codec, Codec::H264);
    let n = p.meta.frame_count.unwrap();
    let t = std::time::Instant::now();
    let out = decode_frames(&clip, &p, &[0, n / 2, n - 1], 640);
    eprintln!("{rel}: 3 frames in {:?}", t.elapsed());
    for r in out {
        let f = r.unwrap_or_else(|e| panic!("{rel}: {e}"));
        let Decoded::Ycbcr {
            width,
            height,
            chroma_width,
            chroma_height,
            bits,
            full_range: fr,
            ref y,
            ..
        } = f.image
        else {
            panic!("expected ycbcr")
        };
        assert_eq!((width, height), size);
        assert_eq!(
            (chroma_width, chroma_height, bits),
            (size.0 / 2, size.1, 10)
        );
        assert_eq!(fr, full_range);
        // A real picture, not a flat fill.
        let mean = y.iter().map(|&v| v as f64).sum::<f64>() / y.len() as f64;
        let var = y.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / y.len() as f64;
        assert!(var > 10.0, "{rel}: flat picture");
    }
}

#[test]
fn h264_xavc_intra_mxf() {
    h264_thumbnails(
        "S_cam/S003/Filmplusgear-FX6-testfile-1-254_0110.MXF",
        Container::Mxf,
        (4096, 2160),
        true,
    );
}

#[test]
fn h264_canon_xf_avc_long_gop_mxf() {
    h264_thumbnails(
        "S_cam/s004/s_0004C062A250107_2240217S_CANON.MXF",
        Container::Mxf,
        (3840, 2160),
        true,
    );
}

#[test]
fn h264_422_mov_falls_back_to_luna_h264() {
    h264_thumbnails(
        "S_cam/S002/lumix_s5.mov",
        Container::QuickTime,
        (3840, 2160),
        true,
    );
}

#[cfg(windows)]
#[test]
fn hevc_422_decodes_when_the_hevc_extension_offers_it() {
    let Some(path) = corpus("S_cam/S005/A_0001C107H250108_061243ND_CANON.MP4") else {
        return;
    };
    let clip = files(Container::QuickTime, &path);
    let p = probe(Codec::Hevc, FrameIndex::DecoderOwned, Default::default());
    match decode_frames(&clip, &p, &[0], 640).pop().unwrap() {
        Ok(f) => {
            let Decoded::Ycbcr {
                width,
                height,
                chroma_width,
                chroma_height,
                bits,
                ..
            } = f.image
            else {
                panic!("expected ycbcr")
            };
            assert_eq!(
                (width, height, chroma_width, chroma_height, bits),
                (4096, 2160, 2048, 2160, 10)
            );
        }
        // Machines without the HEVC Video Extensions report it cleanly.
        Err(DecodeError::Unsupported(m)) => eprintln!("HEVC unsupported here: {m}"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn prores_frames_reach_the_decoder() {
    let Some(path) = corpus("M_CAM/M001/M001C001_161207_R00H.mov") else {
        return;
    };
    let ranges = common::mov_video_ranges(&path).unwrap();
    assert!(!ranges.is_empty());
    // the index points at ProRes frames
    let mut f = std::fs::File::open(&path).unwrap();
    let mut head = [0u8; 8];
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(ranges[0].offset)).unwrap();
    f.read_exact(&mut head).unwrap();
    assert_eq!(&head[4..8], b"icpf");
    let n = ranges.len() as u64;
    let p = probe(
        Codec::ProRes(ProResProfile::Hq),
        FrameIndex::Ranges(ranges),
        Default::default(),
    );
    let clip = files(Container::QuickTime, &path);
    for r in decode_frames(&clip, &p, &[0, n / 2, n - 1], 640) {
        match r {
            Ok(f) => assert!(matches!(f.image, Decoded::Ycbcr { .. })),
            // luna-prores may not decode yet; it must still be a clean error.
            Err(DecodeError::Unsupported(m)) => eprintln!("ProRes: {m}"),
            Err(e) => panic!("{e}"),
        }
    }
}
