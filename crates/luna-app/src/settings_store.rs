//! Where the Settings window loads and saves from.
//!
//! The real store (TOML in the platform config folder, logo copied next to it)
//! lives in `luna_core::settings`; the integrator wires it in behind
//! [`SettingsStore`]. Until then [`MemorySettingsStore`] keeps everything in
//! memory for the session.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use luna_core::settings::Settings;

pub trait SettingsStore {
    fn load(&self) -> Settings;
    fn save(&self, settings: &Settings) -> Result<(), String>;
    /// Validates `file` as a logo and records it in `settings`. The error is
    /// shown inline under the logo.
    fn set_logo(&self, settings: &mut Settings, file: &Path) -> Result<(), String>;
    fn clear_logo(&self, settings: &mut Settings);
}

/// Settings kept in memory for the session.
#[derive(Debug)]
pub struct MemorySettingsStore {
    saved: Mutex<Settings>,
}

impl MemorySettingsStore {
    pub fn new(initial: Settings) -> Self {
        Self {
            saved: Mutex::new(initial),
        }
    }
}

impl Default for MemorySettingsStore {
    fn default() -> Self {
        Self::new(Settings {
            output_dir: default_output_dir(),
            ..Settings::default()
        })
    }
}

impl SettingsStore for MemorySettingsStore {
    fn load(&self) -> Settings {
        self.saved.lock().expect("settings lock").clone()
    }

    fn save(&self, settings: &Settings) -> Result<(), String> {
        *self.saved.lock().expect("settings lock") = settings.clone();
        Ok(())
    }

    fn set_logo(&self, settings: &mut Settings, file: &Path) -> Result<(), String> {
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "svg") {
            return Err("Use a PNG, JPEG or SVG file.".into());
        }
        let meta = std::fs::metadata(file).map_err(|e| format!("Can't read that file: {e}"))?;
        if meta.len() == 0 {
            return Err("That file is empty.".into());
        }
        settings.logo = Some(file.to_path_buf());
        Ok(())
    }

    fn clear_logo(&self, settings: &mut Settings) {
        settings.logo = None;
    }
}

/// `Documents/Luna Reports` in the user's home folder.
/// `Documents/Luna Reports`, from the platform's real Documents folder
/// (which may be redirected).
pub fn default_output_dir() -> PathBuf {
    luna_core::settings::default_output_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_load_back() {
        let store = MemorySettingsStore::default();
        let mut s = store.load();
        s.production = "Night Shoot".into();
        store.save(&s).unwrap();
        assert_eq!(store.load().production, "Night Shoot");
    }

    #[test]
    fn default_output_is_luna_reports_in_documents() {
        let dir = default_output_dir();
        assert!(dir.ends_with(Path::new("Documents").join("Luna Reports")));
        assert_eq!(MemorySettingsStore::default().load().output_dir, dir);
    }

    #[test]
    fn a_logo_must_be_png_jpeg_or_svg() {
        let store = MemorySettingsStore::default();
        let mut s = Settings::default();
        let err = store.set_logo(&mut s, Path::new("logo.gif")).unwrap_err();
        assert_eq!(err, "Use a PNG, JPEG or SVG file.");
        assert_eq!(s.logo, None);
    }

    #[test]
    fn an_existing_png_is_accepted_and_can_be_cleared() {
        let file = std::env::temp_dir().join(format!("luna-app-logo-{}.PNG", std::process::id()));
        std::fs::write(&file, b"not really a png").unwrap();
        let store = MemorySettingsStore::default();
        let mut s = Settings::default();
        store.set_logo(&mut s, &file).unwrap();
        assert_eq!(s.logo.as_deref(), Some(file.as_path()));
        store.clear_logo(&mut s);
        assert_eq!(s.logo, None);
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    fn a_missing_logo_file_is_rejected() {
        let store = MemorySettingsStore::default();
        let mut s = Settings::default();
        assert!(
            store
                .set_logo(&mut s, Path::new("Z:/nope/logo.png"))
                .is_err()
        );
    }
}
