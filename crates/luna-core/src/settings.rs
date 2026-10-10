//! Report details and the output location, stored as TOML in the platform
//! config folder (`%APPDATA%\Luna`, `~/Library/Application Support/Luna`).

use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    pub production: String,
    pub company: String,
    pub author: String,
    /// The logo as copied into the config folder; `None` when there is none.
    pub logo: Option<PathBuf>,
    /// Label/value pairs printed in the report header, in this order.
    pub custom: Vec<(String, String)>,
    /// Where per-run report folders are written. Default `Documents/Luna Reports`.
    pub output_dir: PathBuf,
}
