use std::fmt;

/// A writer failed. The message is meant for a log, not for the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportError {
    Html(String),
    Pdf(String),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Html(msg) => write!(f, "HTML report failed: {msg}"),
            Self::Pdf(msg) => write!(f, "PDF report failed: {msg}"),
        }
    }
}

impl std::error::Error for ReportError {}
