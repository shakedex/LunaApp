//! Luna's camera report: one data model and the three writers that render it.
//!
//! The writers return bytes. Where they are written, and when, is the caller's job.

mod csv;
mod error;
mod fonts;
mod html;
mod logo;
mod model;
mod pdf;
mod view;

pub use csv::write_csv;
pub use error::ReportError;
pub use html::write_html;
pub use logo::{Logo, LogoError, LogoFormat};
pub use model::{
    Camera, Card, Clip, ClipFields, CustomField, FIELD_LABELS, Header, IdentitySource, Jpeg,
    MISSING, Report, ReportTheme, Thumbnail, Totals, Unreadable, UnreadableKind, format_bytes,
};
pub use pdf::{PdfInspection, TextRun, inspect_pdf, write_pdf};
