//! Shared report fixtures. Each test binary uses a different subset.
#![allow(dead_code)]

use luna_report::{
    Camera, Card, Clip, ClipFields, CustomField, Header, IdentitySource, Logo, Report, ReportTheme,
    Thumbnail, Unreadable, UnreadableKind,
};

pub const THUMB_JPEG: &[u8] = include_bytes!("../fixtures/thumb.jpg");
pub const LOGO_PNG: &[u8] = include_bytes!("../fixtures/logo.png");

pub fn full_clip(file_name: &str) -> Clip {
    Clip {
        fields: ClipFields {
            file_name: file_name.to_owned(),
            recording_date: Some("2026-10-01".into()),
            tc_in: Some("10:00:00:00".into()),
            tc_out: Some("10:00:10:00".into()),
            duration: Some("00:00:10:00".into()),
            fps: Some("25".into()),
            sensor_fps: Some("50".into()),
            resolution: Some("4096×2160".into()),
            codec: Some("ProRes 4444".into()),
            camera_model: Some("ALEXA 35".into()),
            camera_serial: Some("60123".into()),
            lens: Some("Signature Prime 47".into()),
            focal_length: Some("47 mm".into()),
            stop: Some("T2.8".into()),
            iso: Some("800".into()),
            shutter: Some("180°".into()),
            white_balance: Some("5600 K".into()),
            nd: Some("0.6".into()),
            audio: Some("4 ch, 48 kHz, 24-bit".into()),
            file_size: Some(4_294_967_296),
            scene: Some("12".into()),
            shot: Some("3".into()),
            take: None,
        },
        camera_source: IdentitySource::Metadata,
        reel_source: IdentitySource::FileName,
        thumbnails: vec![Thumbnail::from_jpeg(THUMB_JPEG.to_vec()); 3],
        warnings: vec![],
    }
}

/// Only the file name is known, and nothing decoded.
pub fn bare_clip(file_name: &str) -> Clip {
    Clip {
        fields: ClipFields {
            file_name: file_name.to_owned(),
            ..ClipFields::default()
        },
        camera_source: IdentitySource::FolderName,
        reel_source: IdentitySource::FolderName,
        thumbnails: vec![
            Thumbnail::Missing {
                reason: "Worker crashed while decoding".into()
            };
            3
        ],
        warnings: vec!["Could not be decoded".into()],
    }
}

pub fn empty_report() -> Report {
    Report {
        header: Header::default(),
        theme: ReportTheme::Light,
        sources: vec!["D:\\Shoot".into()],
        generated_at: "2026-10-10 14:32".into(),
        luna_version: "0.0.0".into(),
        cameras: vec![],
        not_included: vec![],
        unreadable: vec![],
    }
}

/// Two cameras, three cards, a logo, custom fields and every kind of list entry.
pub fn sample_report() -> Report {
    Report {
        header: Header {
            production: Some("Night Shift".into()),
            company: Some("Lumen Rentals".into()),
            author: Some("Dana DIT".into()),
            logo: Some(Logo::from_bytes(LOGO_PNG.to_vec()).expect("fixture logo decodes")),
            custom: vec![CustomField {
                label: "Shoot day".into(),
                value: "12".into(),
            }],
        },
        cameras: vec![
            Camera {
                name: "A".into(),
                cards: vec![
                    Card {
                        reel: "A001".into(),
                        clips: vec![
                            full_clip("A001C001_261001_R1AB.mxf"),
                            full_clip("A001C002_261001_R1AB.mxf"),
                        ],
                    },
                    Card {
                        reel: "A002".into(),
                        clips: vec![full_clip("A002C001_261001_R1AB.mxf")],
                    },
                ],
            },
            Camera {
                name: "B".into(),
                cards: vec![Card {
                    reel: "B001".into(),
                    clips: vec![bare_clip("B001C001.mov")],
                }],
            },
        ],
        not_included: vec!["A001/notes.txt".into()],
        unreadable: vec![Unreadable {
            path: "C001".into(),
            kind: UnreadableKind::Folder,
            reason: "Access denied".into(),
        }],
        ..empty_report()
    }
}

/// Text that is markup in at least one output format.
pub const HOSTILE: &str = "<b>A&B</b> \"q\", *x* #eval $y$ _z_ @ref\nline2";

pub fn hostile_report() -> Report {
    let mut r = sample_report();
    r.header.production = Some(HOSTILE.into());
    r.header.custom = vec![CustomField {
        label: HOSTILE.into(),
        value: HOSTILE.into(),
    }];
    r.cameras[0].name = HOSTILE.into();
    r.cameras[0].cards[0].clips[0].fields.file_name = HOSTILE.into();
    r
}

/// Hebrew and accented Latin in every text slot a user or a file can fill.
pub fn non_ascii_report() -> Report {
    let mut r = sample_report();
    r.header.production = Some("הפקת לילה".into());
    r.header.author = Some("Zoë Ångström".into());
    r.cameras[0].cards[0].clips[0].fields.file_name = "צילום_001_Café.mov".into();
    r
}
