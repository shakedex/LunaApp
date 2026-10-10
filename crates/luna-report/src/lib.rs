//! Luna's camera report: one data model and the three writers that render it.
//!
//! The writers return bytes. Where they are written, and when, is the caller's job.

mod logo;
mod model;

pub use logo::{Logo, LogoError, LogoFormat};
pub use model::{
    Camera, Card, Clip, ClipFields, CustomField, FIELD_LABELS, Header, IdentitySource, MISSING,
    Report, Thumbnail, Totals, Unreadable, UnreadableKind, format_bytes,
};
