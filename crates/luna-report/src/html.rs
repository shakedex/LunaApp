//! HTML: one self-contained file. Inline CSS, images as data URIs, no scripts.

use askama::Template;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::error::ReportError;
use crate::model::Report;
use crate::view::{self, ImageRef, View};

#[derive(Template)]
#[template(path = "report.html")]
struct HtmlReport {
    v: View,
}

pub fn write_html(report: &Report) -> Result<Vec<u8>, ReportError> {
    let v = view::build(report, &mut |image| match image {
        ImageRef::Logo(format, bytes) => data_uri(format.mime(), bytes),
        ImageRef::Thumb(bytes) => data_uri("image/jpeg", bytes),
    });
    HtmlReport { v }
        .render()
        .map(String::into_bytes)
        .map_err(|e| ReportError::Html(e.to_string()))
}

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}
