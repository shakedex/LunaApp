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
    assert_eq!(p.meta.start_timecode.unwrap().to_string(), "15:43:20:20");
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
fn arriraw_alexa35() {
    let Some(path) = corpus("T_CAM/DW0001C021_251020_132307_a1I7H_arri-raw.mxf") else {
        return;
    };
    let ranges = common::mxf_picture_ranges(&path).unwrap();
    assert_eq!(ranges.len(), 165);
    let p = probe(
        Codec::ArriRaw,
        FrameIndex::Ranges(ranges),
        PictureLayout {
            stored_width: 2048,
            stored_height: 1152,
            raw_bits: Some(13),
            pixel_aspect: None,
        },
    );
    let clip = files(Container::Mxf, &path);
    let out = decode_frames(&clip, &p, &[0, 82, 164], 640);
    for r in out {
        let f = r.unwrap();
        assert_eq!(mosaic_size(&f), (2048, 1152));
        let Decoded::Mosaic {
            ref data,
            black,
            white,
            ..
        } = f.image
        else {
            unreachable!()
        };
        assert_eq!((black, white), (256.0, 8191.0));
        // Smooth, sane data: no sample under black - noise, mean well above black.
        assert!(data.iter().all(|&v| v >= 200));
        assert!(mosaic_mean(&f) > 300.0);
    }
}

#[test]
fn h264_422_is_unsupported_on_windows() {
    let Some(path) = corpus("S_cam/S002/lumix_s5.mov") else {
        return;
    };
    let clip = files(Container::QuickTime, &path);
    let p = probe(Codec::H264, FrameIndex::DecoderOwned, Default::default());
    let out = decode_frames(&clip, &p, &[0, 100], 640);
    for r in out {
        match r {
            Err(DecodeError::Unsupported(m)) => assert!(m.contains("isn't supported"), "{m}"),
            other => panic!("expected unsupported, got {other:?}"),
        }
    }
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
