//! HTML: one self-contained file. Inline CSS, images as data URIs, no scripts.

use askama::Template;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::error::ReportError;
use crate::fonts::FACES;
use crate::model::Report;
use crate::view::{self, ImageRef, View};

#[derive(Template)]
#[template(path = "report.html")]
struct HtmlReport {
    v: View,
    /// `@font-face` rules carrying the report's typeface.
    font_faces: String,
}

pub fn write_html(report: &Report) -> Result<Vec<u8>, ReportError> {
    let v = view::build(report, &mut |image| match image {
        ImageRef::Logo(format, bytes) => data_uri(format.mime(), bytes),
        ImageRef::Thumb(bytes) => data_uri("image/jpeg", bytes),
    });
    HtmlReport {
        v,
        font_faces: font_faces(),
    }
    .render()
    .map(String::into_bytes)
    .map_err(|e| ReportError::Html(e.to_string()))
}

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn font_faces() -> String {
    FACES
        .iter()
        .map(|f| {
            format!(
                "@font-face {{ font-family: \"{}\"; font-weight: {}; src: url({}) format(\"truetype\"); }}
",
                f.family,
                f.weight,
                data_uri("font/ttf", f.data)
            )
        })
        .collect()
}
