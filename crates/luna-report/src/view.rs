//! The report flattened into display strings. HTML and PDF both render this, so
//! they can't disagree. Each writer only decides how an image is referenced.

use serde::Serialize;

use crate::logo::LogoFormat;
use crate::model::{CustomField, FIELD_LABELS, MISSING, Report, Thumbnail, Totals, format_bytes};

/// An image a writer must make referable: return the string the template uses as its source.
pub(crate) enum ImageRef<'a> {
    Logo(LogoFormat, &'a [u8]),
    Thumb(&'a [u8]),
}

#[derive(Serialize)]
pub(crate) struct View {
    pub header: HeaderView,
    pub source_root: String,
    pub generated_at: String,
    pub luna_version: String,
    pub totals: Totals,
    pub total_size: String,
    pub labels: Vec<&'static str>,
    pub cameras: Vec<CameraView>,
    pub not_included: Vec<String>,
    pub unreadable: Vec<UnreadableView>,
}

#[derive(Serialize)]
pub(crate) struct HeaderView {
    pub production: Option<String>,
    pub company: Option<String>,
    pub author: Option<String>,
    pub logo: Option<String>,
    pub custom: Vec<CustomField>,
}

#[derive(Serialize)]
pub(crate) struct CameraView {
    pub name: String,
    pub cards: Vec<CardView>,
}

#[derive(Serialize)]
pub(crate) struct CardView {
    pub reel: String,
    pub clips: Vec<ClipView>,
}

#[derive(Serialize)]
pub(crate) struct ClipView {
    /// In `labels` order, with [`MISSING`] for absent values.
    pub values: Vec<String>,
    pub thumbs: Vec<ThumbView>,
    pub identity: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct ThumbView {
    pub src: Option<String>,
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct UnreadableView {
    pub kind: &'static str,
    pub path: String,
    pub reason: String,
}

pub(crate) fn build(report: &Report, image_ref: &mut dyn FnMut(ImageRef) -> String) -> View {
    let h = &report.header;
    let logo = h
        .logo
        .as_ref()
        .map(|l| image_ref(ImageRef::Logo(l.format(), l.bytes())));
    let totals = report.totals();

    let cameras = report
        .cameras
        .iter()
        .map(|camera| CameraView {
            name: camera.name.clone(),
            cards: camera
                .cards
                .iter()
                .map(|card| CardView {
                    reel: card.reel.clone(),
                    clips: card
                        .clips
                        .iter()
                        .map(|clip| ClipView {
                            values: clip
                                .fields
                                .display_values()
                                .into_iter()
                                .map(|v| v.unwrap_or_else(|| MISSING.to_owned()))
                                .collect(),
                            thumbs: clip
                                .thumbnails
                                .iter()
                                .map(|t| match t {
                                    Thumbnail::Jpeg(bytes) => ThumbView {
                                        src: Some(image_ref(ImageRef::Thumb(bytes))),
                                        reason: None,
                                    },
                                    Thumbnail::Missing { reason } => ThumbView {
                                        src: None,
                                        reason: Some(reason.clone()),
                                    },
                                })
                                .collect(),
                            identity: format!(
                                "Camera from {}, reel from {}",
                                clip.camera_source.label(),
                                clip.reel_source.label()
                            ),
                            warnings: clip.warnings.clone(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    View {
        header: HeaderView {
            production: h.production.clone(),
            company: h.company.clone(),
            author: h.author.clone(),
            logo,
            custom: h.custom.clone(),
        },
        source_root: report.source_root.clone(),
        generated_at: report.generated_at.clone(),
        luna_version: report.luna_version.clone(),
        totals,
        total_size: format_bytes(totals.bytes),
        labels: FIELD_LABELS.to_vec(),
        cameras,
        not_included: report.not_included.clone(),
        unreadable: report
            .unreadable
            .iter()
            .map(|u| UnreadableView {
                kind: u.kind.label(),
                path: u.path.clone(),
                reason: u.reason.clone(),
            })
            .collect(),
    }
}
