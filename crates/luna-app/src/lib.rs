//! Luna's desktop window: the one-flow main window, Settings and Credits.
//!
//! Everything that can be tested without a window lives in plain modules
//! (`flow`, `view`, `settings_edit`, `settings_store`, `backend`); `ui` binds
//! them to the Slint components in `ui/`.

pub mod backend;
pub mod flow;
pub mod settings_edit;
pub mod settings_store;
pub mod view;
