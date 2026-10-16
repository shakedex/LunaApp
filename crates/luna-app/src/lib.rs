//! Luna's desktop window: the one-flow main window and its Settings view.
//!
//! Everything that can be tested without a window lives in plain modules
//! (`flow`, `view`, `settings_edit`, `settings_store`, `backend`); `ui` binds
//! them to the Slint components in `ui/`.

pub mod backend;
pub mod core_backend;
pub mod flow;
pub mod log;
pub mod moon;
pub mod settings_edit;
pub mod settings_store;
pub mod ui;
pub mod view;
