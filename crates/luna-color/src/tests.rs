//! End-to-end tests of `render` and `encode_jpeg` on synthetic frames.

use luna_decode::{Cfa, Decoded, Frame, YcbcrMatrix};
use luna_formats::{ColourHint, Gamut, Transfer};

use crate::{Rgb8, curves, encode_jpeg, gamut, mosaic, render};

/// Where 18% grey must land on every camera: 0.42 signal → code 107.
const GREY: u8 = 107;

const CAMERAS: [(Transfer, Gamut); 10] = [
    (Transfer::LogC3, Gamut::ArriWideGamut3),
    (Transfer::LogC4, Gamut::ArriWideGamut4),
    (Transfer::SLog3, Gamut::SGamut3Cine),
    (Transfer::SLog3, Gamut::SGamut3),
    (Transfer::CanonLog2, Gamut::CinemaGamut),
    (Transfer::CanonLog3, Gamut::CinemaGamut),
    (Transfer::VLog, Gamut::VGamut),
    (Transfer::DLog, Gamut::DGamut),
    (Transfer::BmdFilmGen5, Gamut::BmdWideGamutGen5),
    (Transfer::Log3G10, Gamut::RedWideGamutRgb),
];

fn hint(transfer: Option<Transfer>, gamut: Option<Gamut>) -> ColourHint {
    ColourHint { transfer, gamut }
}

fn rgb16(width: u32, height: u32, rgb: [u16; 3], transfer: Transfer, gamut: Gamut) -> Frame {
    Frame {
        image: Decoded::Rgb16 {
            width,
            height,
            data: rgb.repeat((width * height) as usize),
            transfer,
            gamut,
        },
        hint: ColourHint::default(),
    }
}

/// A flat 10-bit 4:2:2 Y′CbCr frame at luma code `y`, neutral chroma.
fn ycbcr_flat(width: u32, height: u32, y: u16, full_range: bool, hint: ColourHint) -> Frame {
    let n = (width * height) as usize;
    Frame {
        image: Decoded::Ycbcr {
            width,
            height,
            chroma_width: width / 2,
            chroma_height: height,
            bits: 10,
            y: vec![y; n],
            cb: vec![512; n / 2],
            cr: vec![512; n / 2],
            matrix: YcbcrMatrix::Bt709,
            full_range,
        },
        hint,
    }
}

fn all_pixels(img: &Rgb8) -> impl Iterator<Item = [u8; 3]> + '_ {
    img.data.chunks(3).map(|p| [p[0], p[1], p[2]])
}

fn assert_flat(img: &Rgb8, want: u8, tol: u8, what: &str) {
    for px in all_pixels(img) {
        for c in px {
            assert!(c.abs_diff(want) <= tol, "{what}: {px:?}, want {want}");
        }
    }
}

#[test]
fn eighteen_percent_grey_matches_across_every_camera_rgb16() {
    for (t, g) in CAMERAS {
        let code = (curves::encode(t, 0.18) * 65535.0).round() as u16;
        let out = render(&rgb16(64, 36, [code; 3], t, g), 32, None);
        assert!(!out.transform_unknown);
        assert_flat(&out.image, GREY, 1, &format!("{t:?}/{g:?}"));
    }
}

#[test]
fn eighteen_percent_grey_matches_across_every_camera_ycbcr_legal_range() {
    // Cameras write their log code values straight into the file's code range;
    // a legal-range decode must undo that before the curve.
    for (t, g) in CAMERAS {
        let y = (curves::encode(t, 0.18) * 1023.0).round() as u16;
        let out = render(
            &ycbcr_flat(64, 36, y, false, hint(Some(t), Some(g))),
            32,
            None,
        );
        assert!(!out.transform_unknown);
        assert_flat(&out.image, GREY, 1, &format!("{t:?}/{g:?}"));
    }
}

#[test]
fn grey_ramps_render_in_the_same_order_and_close_together() {
    // A ramp from −4 to +4 stops; every camera should give nearly the same
    // display values at each stop.
    let stops: Vec<f64> = (-4..=4).map(|s| 0.18 * 2f64.powi(s)).collect();
    let mut rows: Vec<Vec<u8>> = Vec::new();
    for (t, g) in CAMERAS {
        let data: Vec<u16> = stops
            .iter()
            .flat_map(|&x| [(curves::encode(t, x) * 65535.0).round() as u16; 3])
            .collect();
        let frame = Frame {
            image: Decoded::Rgb16 {
                width: stops.len() as u32,
                height: 1,
                data,
                transfer: t,
                gamut: g,
            },
            hint: ColourHint::default(),
        };
        let out = render(&frame, 64, None);
        let row: Vec<u8> = out.image.data.chunks(3).map(|p| p[1]).collect();
        assert!(row.windows(2).all(|w| w[0] < w[1]), "{t:?}: {row:?}");
        rows.push(row);
    }
    for row in &rows[1..] {
        for (a, b) in row.iter().zip(&rows[0]) {
            assert!(a.abs_diff(*b) <= 2, "{row:?} vs {:?}", rows[0]);
        }
    }
}

#[test]
fn hdr_grey_lines_up_with_the_cameras() {
    // HLG 38% and PQ 26 nits are the BT.2408 grey card.
    let hlg_y = (64.0 + 0.38 * 876.0_f64).round() as u16;
    let out = render(
        &ycbcr_flat(
            32,
            18,
            hlg_y,
            false,
            hint(Some(Transfer::Hlg), Some(Gamut::Rec2020)),
        ),
        32,
        None,
    );
    assert_flat(&out.image, GREY, 1, "HLG");
    let pq_y = (64.0 + 0.380_032 * 876.0_f64).round() as u16;
    let out = render(
        &ycbcr_flat(
            32,
            18,
            pq_y,
            false,
            hint(Some(Transfer::Pq), Some(Gamut::Rec2020)),
        ),
        32,
        None,
    );
    assert_flat(&out.image, GREY, 1, "PQ");
}

#[test]
fn rec709_is_shown_as_coded() {
    let h = hint(Some(Transfer::Rec709), Some(Gamut::Rec709));
    assert_flat(
        &render(&ycbcr_flat(16, 8, 940, false, h), 16, None).image,
        255,
        0,
        "white",
    );
    assert_flat(
        &render(&ycbcr_flat(16, 8, 64, false, h), 16, None).image,
        0,
        0,
        "black",
    );
    let mid = render(&ycbcr_flat(16, 8, 502, false, h), 16, None);
    assert_flat(&mid.image, 128, 1, "50%");
    assert!(!mid.transform_unknown);
    // Full range: code 511 of 1023 is half.
    let full = render(&ycbcr_flat(16, 8, 511, true, h), 16, None);
    assert_flat(&full.image, 127, 1, "full-range 50%");
}

#[test]
fn rec709_in_a_wide_gamut_container_is_converted_not_passed_through() {
    // Pure Rec.2020 green is outside Rec.709: it must not stay (0, 255, 0).
    let frame = rgb16(8, 8, [0, 65535, 0], Transfer::Rec709, Gamut::Rec2020);
    let out = render(&frame, 8, None);
    let px = all_pixels(&out.image).next().unwrap();
    assert!(px[0] > 0 || px[2] > 0, "{px:?}");
    // Neutral stays neutral.
    let grey = rgb16(8, 8, [30000; 3], Transfer::Rec709, Gamut::Rec2020);
    let out = render(&grey, 8, None);
    let px = all_pixels(&out.image).next().unwrap();
    assert!(
        px[0].abs_diff(px[1]) <= 1 && px[1].abs_diff(px[2]) <= 1,
        "{px:?}"
    );
}

#[test]
fn unknown_transfer_is_flagged_and_shown_as_decoded() {
    let out = render(&ycbcr_flat(16, 8, 502, false, hint(None, None)), 16, None);
    assert!(out.transform_unknown);
    assert_flat(&out.image, 128, 1, "unknown");
    // Gamut alone is not enough to render.
    let out = render(
        &ycbcr_flat(16, 8, 502, false, hint(None, Some(Gamut::SGamut3))),
        16,
        None,
    );
    assert!(out.transform_unknown);
}

#[test]
fn display8_is_passed_through_untouched() {
    let data: Vec<u8> = (0..4 * 2 * 3).map(|i| (i * 10) as u8).collect();
    let frame = Frame {
        image: Decoded::Display8 {
            width: 4,
            height: 2,
            data: data.clone(),
        },
        hint: hint(Some(Transfer::SLog3), Some(Gamut::SGamut3)),
    };
    let out = render(&frame, 640, None);
    assert!(!out.transform_unknown);
    assert_eq!((out.image.width, out.image.height), (4, 2));
    assert_eq!(out.image.data, data);
}

#[test]
fn display8_is_box_scaled() {
    // 2×1 → 1×1 averages.
    let frame = Frame {
        image: Decoded::Display8 {
            width: 2,
            height: 2,
            data: vec![0, 100, 200, 100, 200, 0, 0, 100, 200, 100, 200, 0],
        },
        hint: ColourHint::default(),
    };
    let out = render(&frame, 1, None);
    assert_eq!(out.image.data, vec![50, 150, 100]);
}

fn mosaic_frame(width: u32, height: u32, cfa: Cfa, rgb: [u16; 3], hint: ColourHint) -> Frame {
    let cell = match cfa {
        Cfa::Rggb => [rgb[0], rgb[1], rgb[1], rgb[2]],
        Cfa::Grbg => [rgb[1], rgb[0], rgb[2], rgb[1]],
        Cfa::Gbrg => [rgb[1], rgb[2], rgb[0], rgb[1]],
        Cfa::Bggr => [rgb[2], rgb[1], rgb[1], rgb[0]],
    };
    let mut data = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            data.push(cell[((y % 2) * 2 + x % 2) as usize]);
        }
    }
    Frame {
        image: Decoded::Mosaic {
            width,
            height,
            cfa,
            data,
            black: 256.0,
            white: 65535.0,
            wb: None,
            camera_to_xyz: None,
            scene_scale: None,
        },
        hint,
    }
}

/// The raw code an 18% grey card gives on a sensor clipping `stops` over grey.
fn raw_grey(stops: f32) -> u16 {
    (256.0 + (65535.0 - 256.0) * 0.18 / (0.18 * stops.exp2())).round() as u16
}

#[test]
fn raw_grey_lands_on_the_same_grey_for_every_cfa() {
    for g in [
        Gamut::ArriWideGamut3,
        Gamut::ArriWideGamut4,
        Gamut::SGamut3,
        Gamut::CinemaGamut,
    ] {
        let v = raw_grey(mosaic::clip_stops_above_grey(Some(g)));
        for cfa in [Cfa::Rggb, Cfa::Grbg, Cfa::Gbrg, Cfa::Bggr] {
            let out = render(
                &mosaic_frame(64, 36, cfa, [v; 3], hint(Some(Transfer::Linear), Some(g))),
                16,
                None,
            );
            assert!(!out.transform_unknown);
            assert_eq!((out.image.width, out.image.height), (16, 9));
            assert_flat(&out.image, GREY, 1, &format!("{g:?} {cfa:?}"));
        }
    }
}

#[test]
fn decoder_stated_exposure_places_grey() {
    // scene-linear = (raw − black) × scale; grey 0.18 sits at raw 256 + 0.18 / scale.
    for scale in [1.0f32 / 2000.0, 1.0 / 20_000.0] {
        let v = (256.0 + 0.18 / scale).round() as u16;
        let mut frame = mosaic_frame(
            32,
            18,
            Cfa::Rggb,
            [v; 3],
            hint(Some(Transfer::Linear), Some(Gamut::CinemaGamut)),
        );
        if let Decoded::Mosaic { scene_scale, .. } = &mut frame.image {
            *scene_scale = Some(scale);
        }
        let out = render(&frame, 16, None);
        assert_flat(&out.image, GREY, 1, &format!("scale {scale}"));
    }
}

#[test]
fn raw_colour_lands_in_the_right_channel() {
    let v = raw_grey(6.0);
    let out = render(
        &mosaic_frame(
            16,
            16,
            Cfa::Grbg,
            [v * 2, 256, 256],
            hint(None, Some(Gamut::Rec709)),
        ),
        8,
        None,
    );
    let px = all_pixels(&out.image).next().unwrap();
    assert!(px[0] > 120 && px[1] < 60 && px[2] < 60, "{px:?}");
}

#[test]
fn raw_camera_matrix_is_used_when_given() {
    let v = raw_grey(6.0);
    let mut frame = mosaic_frame(16, 16, Cfa::Rggb, [v; 3], hint(None, None));
    let plain = render(&frame, 8, None);
    // Rec.709's own RGB → XYZ matrix: the same picture as no matrix.
    let npm = gamut::rgb_to_xyz(&gamut::primaries(Gamut::Rec709));
    let m: Vec<f32> = npm.iter().flatten().map(|&v| v as f32).collect();
    if let Decoded::Mosaic { camera_to_xyz, .. } = &mut frame.image {
        *camera_to_xyz = Some(m.try_into().unwrap());
    }
    assert_eq!(render(&frame, 8, None), plain);
    // A matrix that sends camera red to Rec.709 blue: red raw renders blue.
    let swapped: Vec<f32> = npm
        .iter()
        .flat_map(|row| [row[2], row[1], row[0]])
        .map(|v| v as f32)
        .collect();
    let mut red = mosaic_frame(16, 16, Cfa::Rggb, [v * 2, 256, 256], hint(None, None));
    if let Decoded::Mosaic { camera_to_xyz, .. } = &mut red.image {
        *camera_to_xyz = Some(swapped.try_into().unwrap());
    }
    let px = all_pixels(&render(&red, 8, None).image).next().unwrap();
    assert!(px[2] > 120 && px[0] < 10, "{px:?}");
}

#[test]
fn raw_white_balance_neutralises_a_cast() {
    let v = raw_grey(6.0);
    let mut frame = mosaic_frame(
        16,
        16,
        Cfa::Rggb,
        [v / 2 + 128, v, v / 2 + 128],
        hint(None, None),
    );
    if let Decoded::Mosaic { wb, .. } = &mut frame.image {
        *wb = Some([2.0, 1.0, 2.0]);
    }
    let out = render(&frame, 8, None);
    let px = all_pixels(&out.image).next().unwrap();
    assert!(
        px[0].abs_diff(px[1]) <= 2 && px[2].abs_diff(px[1]) <= 2,
        "{px:?}"
    );
}

#[test]
fn output_size_follows_max_width_and_pixel_aspect() {
    let frame = rgb16(400, 300, [30000; 3], Transfer::SLog3, Gamut::SGamut3Cine);
    let out = render(&frame, 100, None);
    assert_eq!((out.image.width, out.image.height), (100, 75));
    assert_eq!(out.image.data.len(), 100 * 75 * 3);
    let out = render(&frame, 100, Some((2, 1)));
    assert_eq!((out.image.width, out.image.height), (100, 38));
    let out = render(&frame, 1000, None);
    assert_eq!((out.image.width, out.image.height), (400, 300));
    // Anamorphic smaller than max width: widened.
    let out = render(&frame, 1000, Some((2, 1)));
    assert_eq!((out.image.width, out.image.height), (800, 300));
}

#[test]
fn short_buffers_give_a_black_frame_not_a_panic() {
    let frames = [
        Frame {
            image: Decoded::Rgb16 {
                width: 100,
                height: 100,
                data: vec![1; 10],
                transfer: Transfer::SLog3,
                gamut: Gamut::SGamut3,
            },
            hint: ColourHint::default(),
        },
        Frame {
            image: Decoded::Display8 {
                width: 100,
                height: 100,
                data: vec![],
            },
            hint: ColourHint::default(),
        },
        Frame {
            image: Decoded::Mosaic {
                width: 100,
                height: 100,
                cfa: Cfa::Rggb,
                data: vec![0; 5],
                black: 0.0,
                white: 0.0,
                wb: None,
                camera_to_xyz: None,
                scene_scale: None,
            },
            hint: ColourHint::default(),
        },
        Frame {
            image: Decoded::Ycbcr {
                width: 100,
                height: 100,
                chroma_width: 50,
                chroma_height: 50,
                bits: 10,
                y: vec![],
                cb: vec![],
                cr: vec![],
                matrix: YcbcrMatrix::Bt709,
                full_range: false,
            },
            hint: ColourHint::default(),
        },
    ];
    for f in &frames {
        let out = render(f, 50, None);
        assert_eq!((out.image.width, out.image.height), (50, 50));
        assert!(out.image.data.iter().all(|&v| v == 0));
    }
}

#[test]
fn saturated_wide_gamut_colours_stay_valid() {
    for (t, g) in CAMERAS {
        for rgb in [
            [65535, 0, 0],
            [0, 65535, 0],
            [0, 0, 65535],
            [0, 0, 0],
            [65535; 3],
        ] {
            let out = render(&rgb16(4, 4, rgb, t, g), 4, None);
            assert_eq!(out.image.data.len(), 4 * 4 * 3);
        }
    }
}

/// Finds the SOF0 segment and returns (height, width).
fn sof0_size(jpeg: &[u8]) -> Option<(u16, u16)> {
    let mut i = 2;
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xFF {
            return None;
        }
        let marker = jpeg[i + 1];
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if marker == 0xC0 {
            let h = u16::from_be_bytes([jpeg[i + 5], jpeg[i + 6]]);
            let w = u16::from_be_bytes([jpeg[i + 7], jpeg[i + 8]]);
            return Some((h, w));
        }
        i += 2 + len;
    }
    None
}

#[test]
fn jpeg_is_baseline_with_the_right_size() {
    let img = Rgb8 {
        width: 37,
        height: 21,
        data: (0..37 * 21 * 3).map(|i| (i % 251) as u8).collect(),
    };
    let jpeg = encode_jpeg(&img);
    assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "SOI");
    assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9], "EOI");
    assert_eq!(
        sof0_size(&jpeg),
        Some((21, 37)),
        "baseline SOF0 with dimensions"
    );
}

#[test]
fn jpeg_of_a_bad_image_is_empty() {
    let img = Rgb8 {
        width: 10,
        height: 10,
        data: vec![0; 3],
    };
    assert!(encode_jpeg(&img).is_empty());
}

#[test]
fn rendered_thumbnail_encodes() {
    let out = render(
        &rgb16(1280, 720, [30000; 3], Transfer::SLog3, Gamut::SGamut3Cine),
        640,
        None,
    );
    let jpeg = encode_jpeg(&out.image);
    assert_eq!(sof0_size(&jpeg), Some((360, 640)));
}

/// `cargo test -p luna-color --release -- --ignored --nocapture perf`
#[test]
#[ignore = "timing; run in release"]
fn perf_8k_mosaic_and_4k_ycbcr() {
    use std::time::Instant;
    let (w, h) = (8192u32, 4320u32);
    let data: Vec<u16> = (0..w * h).map(|i| (i % 4096) as u16 * 16).collect();
    let frame = Frame {
        image: Decoded::Mosaic {
            width: w,
            height: h,
            cfa: Cfa::Rggb,
            data,
            black: 4096.0,
            white: 65535.0,
            wb: Some([2.0, 1.0, 1.5]),
            camera_to_xyz: None,
            scene_scale: None,
        },
        hint: hint(Some(Transfer::Linear), Some(Gamut::SGamut3)),
    };
    let t = Instant::now();
    let out = render(&frame, 640, None);
    let jpeg = encode_jpeg(&out.image);
    println!(
        "8K mosaic → {}×{} JPEG ({} B): {:?}",
        out.image.width,
        out.image.height,
        jpeg.len(),
        t.elapsed()
    );

    let (w, h) = (4096u32, 2160u32);
    let n = (w * h) as usize;
    let frame = Frame {
        image: Decoded::Ycbcr {
            width: w,
            height: h,
            chroma_width: w,
            chroma_height: h,
            bits: 12,
            y: (0..n).map(|i| (i % 3500) as u16 + 256).collect(),
            cb: (0..n).map(|i| (i % 1000) as u16 + 1500).collect(),
            cr: (0..n).map(|i| (i % 1200) as u16 + 1400).collect(),
            matrix: YcbcrMatrix::Bt709,
            full_range: false,
        },
        hint: hint(Some(Transfer::LogC3), Some(Gamut::ArriWideGamut3)),
    };
    let t = Instant::now();
    let out = render(&frame, 640, None);
    let jpeg = encode_jpeg(&out.image);
    println!(
        "4K 4:4:4 12-bit → {}×{} JPEG ({} B): {:?}",
        out.image.width,
        out.image.height,
        jpeg.len(),
        t.elapsed()
    );
}
