//! Report details and the output location, stored as TOML in the platform
//! config folder (`%APPDATA%\Luna`, `~/Library/Application Support/Luna`).

use std::io;
use std::path::{Path, PathBuf};

use luna_report::Logo;
pub use luna_report::ReportTheme;
pub use luna_worker::ThumbnailColour;
use serde::{Deserialize, Serialize};

const FILE: &str = "settings.toml";
const LOGO_STEM: &str = "logo";
const LOGO_EXTENSIONS: [&str; 3] = ["png", "jpg", "svg"];

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub report_theme: ReportTheme,
    /// Also writes each report to `Luna` at the root of the source drive.
    pub copy_to_source: bool,
    /// Prints clip warnings and unreadable files in the report.
    pub show_problems: bool,
    pub colour: ThumbnailColour,
    /// The `.cube` file for [`ThumbnailColour::Lut`], where the user keeps it.
    pub lut: Option<PathBuf>,
    /// Use the LUT a clip carries, when it has one.
    pub camera_look: bool,
    /// LUTs chosen before, newest first, at most [`RECENT_LUTS`].
    pub recent_luts: Vec<PathBuf>,
}

/// How many LUTs Settings remembers for quick picking.
pub const RECENT_LUTS: usize = 5;

impl Default for Settings {
    fn default() -> Self {
        Self {
            production: String::new(),
            company: String::new(),
            author: String::new(),
            logo: None,
            custom: Vec::new(),
            output_dir: default_output_dir(),
            report_theme: ReportTheme::Light,
            copy_to_source: false,
            show_problems: false,
            colour: ThumbnailColour::Rec709,
            lut: None,
            camera_look: false,
            recent_luts: Vec::new(),
        }
    }
}

/// `Documents/Luna Reports`, falling back to the home folder, then the
/// current folder, when the platform has no Documents folder.
pub fn default_output_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_default()
        .join("Luna Reports")
}

impl Settings {
    /// Settings from the platform config folder; defaults when there are none.
    pub fn load() -> Settings {
        SettingsStore::platform()
            .map(|s| s.load())
            .unwrap_or_default()
    }

    /// Writes to the platform config folder.
    pub fn save(&self) -> io::Result<()> {
        platform_store()?.save(self)
    }

    /// Validates `source` as a logo and copies it into the platform config
    /// folder. The error is a sentence for the UI.
    pub fn set_logo(&mut self, source: &Path) -> Result<(), String> {
        platform_store()
            .map_err(|e| e.to_string())?
            .set_logo(self, source)
    }

    /// Checks that `path` is a usable `.cube` LUT and keeps a reference to it
    /// where it is. The error is a sentence for the UI.
    pub fn set_lut(&mut self, path: &Path) -> Result<(), String> {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("The LUT {name} can't be read: {e}"))?;
        luna_color::Lut::parse_cube(&text)
            .map_err(|e| format!("The LUT {name} can't be used: {e}"))?;
        self.lut = Some(path.to_path_buf());
        self.recent_luts.retain(|p| p != path);
        self.recent_luts.insert(0, path.to_path_buf());
        self.recent_luts.truncate(RECENT_LUTS);
        Ok(())
    }

    /// Removes the logo from the platform config folder.
    pub fn clear_logo(&mut self) {
        match SettingsStore::platform() {
            Some(store) => store.clear_logo(self),
            None => self.logo = None,
        }
    }
}

fn platform_store() -> io::Result<SettingsStore> {
    SettingsStore::platform()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config folder on this system"))
}

/// A config folder holding `settings.toml` and the logo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsStore {
    dir: PathBuf,
}

/// The file as stored. Every field is optional so older or hand-edited files load.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct SettingsFile {
    production: String,
    company: String,
    author: String,
    /// A file name inside the config folder, or an absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    logo: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dir: Option<PathBuf>,
    custom: Vec<CustomEntry>,
    report_theme: ReportTheme,
    copy_to_source: bool,
    show_problems: bool,
    colour: ThumbnailColour,
    #[serde(skip_serializing_if = "Option::is_none")]
    lut: Option<PathBuf>,
    camera_look: bool,
    recent_luts: Vec<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct CustomEntry {
    label: String,
    value: String,
}

impl SettingsStore {
    /// `<config dir>/Luna`; `None` when the platform has no config folder.
    pub fn platform() -> Option<Self> {
        dirs::config_dir().map(|d| Self::at(d.join("Luna")))
    }

    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    /// Never fails: a missing or unreadable file gives defaults, and a corrupt
    /// one is kept as `settings.toml.bak` before defaults are returned.
    pub fn load(&self) -> Settings {
        let path = self.file();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Settings::default();
        };
        let file: SettingsFile = match toml::from_str(&text) {
            Ok(f) => f,
            Err(_) => {
                let _ = std::fs::rename(&path, self.dir.join(format!("{FILE}.bak")));
                return Settings::default();
            }
        };
        let logo = file.logo.map(|l| self.dir.join(l)).filter(|l| l.is_file());
        Settings {
            production: file.production,
            company: file.company,
            author: file.author,
            logo,
            custom: file
                .custom
                .into_iter()
                .map(|c| (c.label, c.value))
                .collect(),
            output_dir: file
                .output_dir
                .filter(|d| !d.as_os_str().is_empty())
                .unwrap_or_else(default_output_dir),
            report_theme: file.report_theme,
            copy_to_source: file.copy_to_source,
            show_problems: file.show_problems,
            colour: file.colour,
            lut: file.lut,
            camera_look: file.camera_look,
            recent_luts: file
                .recent_luts
                .into_iter()
                .filter(|p| p.is_file())
                .collect(),
        }
    }

    /// Writes atomically (temp file, then rename), creating the folder.
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let logo = settings
            .logo
            .as_ref()
            .map(|l| match l.strip_prefix(&self.dir) {
                Ok(inside) => inside.to_path_buf(),
                Err(_) => l.clone(),
            });
        let file = SettingsFile {
            production: settings.production.clone(),
            company: settings.company.clone(),
            author: settings.author.clone(),
            logo,
            output_dir: Some(settings.output_dir.clone()),
            custom: settings
                .custom
                .iter()
                .map(|(label, value)| CustomEntry {
                    label: label.clone(),
                    value: value.clone(),
                })
                .collect(),
            report_theme: settings.report_theme,
            copy_to_source: settings.copy_to_source,
            show_problems: settings.show_problems,
            colour: settings.colour,
            lut: settings.lut.clone(),
            camera_look: settings.camera_look,
            recent_luts: settings.recent_luts.clone(),
        };
        let text = toml::to_string(&file).map_err(io::Error::other)?;
        let tmp = self.dir.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, self.file())
    }

    /// Validates `source` the way the report will place it, then copies it in
    /// as `logo.<ext>`, replacing any earlier logo. On failure `settings` is
    /// unchanged and the error is a sentence for the UI.
    pub fn set_logo(&self, settings: &mut Settings, source: &Path) -> Result<(), String> {
        let bytes =
            std::fs::read(source).map_err(|e| format!("the logo file can't be read: {e}"))?;
        let logo = Logo::from_bytes(bytes).map_err(|e| e.to_string())?;
        let dest = self
            .dir
            .join(format!("{LOGO_STEM}.{}", logo.format().extension()));
        let copy = || -> io::Result<()> {
            std::fs::create_dir_all(&self.dir)?;
            let tmp = self.dir.join(format!("{LOGO_STEM}.tmp"));
            std::fs::write(&tmp, logo.bytes())?;
            self.remove_logos();
            std::fs::rename(&tmp, &dest)
        };
        copy().map_err(|e| format!("the logo couldn't be saved: {e}"))?;
        settings.logo = Some(dest);
        Ok(())
    }

    pub fn clear_logo(&self, settings: &mut Settings) {
        self.remove_logos();
        settings.logo = None;
    }

    fn remove_logos(&self) {
        for ext in LOGO_EXTENSIONS {
            let _ = std::fs::remove_file(self.dir.join(format!("{LOGO_STEM}.{ext}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = include_bytes!("../../luna-report/tests/fixtures/logo.png");
    const JPEG: &[u8] = include_bytes!("../../luna-report/tests/fixtures/thumb.jpg");

    fn store() -> (tempfile::TempDir, SettingsStore) {
        let t = tempfile::tempdir().unwrap();
        let s = SettingsStore::at(t.path().join("Luna"));
        (t, s)
    }

    fn sample() -> Settings {
        Settings {
            production: "Night Swim".into(),
            company: "Moon & Co".into(),
            author: "Noa".into(),
            logo: None,
            custom: vec![
                ("DIT".into(), "Noa".into()),
                ("Day".into(), "3 \"wet\"".into()),
            ],
            output_dir: PathBuf::from("/reports/here"),
            report_theme: ReportTheme::Dark,
            copy_to_source: true,
            show_problems: true,
            colour: ThumbnailColour::Lut,
            lut: Some(PathBuf::from("/luts/show.cube")),
            camera_look: true,
            recent_luts: vec![],
        }
    }

    #[test]
    fn set_lut_rejects_a_bad_cube() {
        let t = tempfile::tempdir().unwrap();
        let bad = t.path().join("bad.cube");
        std::fs::write(
            &bad,
            "LUT_3D_SIZE 2
0 0 0
",
        )
        .unwrap();
        let mut s = Settings::default();
        let err = s.set_lut(&bad).unwrap_err();
        assert!(err.contains("bad.cube"), "{err}");
        assert_eq!(s.lut, None);
    }

    fn cube(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(
            &p,
            "LUT_1D_SIZE 2
0 0 0
1 1 1
",
        )
        .unwrap();
        p
    }

    #[test]
    fn set_lut_remembers_recent_luts_newest_first() {
        let t = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        let names = ["a.cube", "b.cube", "c.cube", "d.cube", "e.cube", "f.cube"];
        let paths: Vec<_> = names.iter().map(|n| cube(t.path(), n)).collect();
        for p in &paths {
            s.set_lut(p).unwrap();
        }
        s.set_lut(&paths[2]).unwrap();
        let want: Vec<_> = [2, 5, 4, 3, 1].iter().map(|&i| paths[i].clone()).collect();
        assert_eq!(s.recent_luts, want);
    }

    #[test]
    fn recent_luts_round_trip_and_skip_missing_files() {
        let (t, store) = store();
        let kept = cube(t.path(), "kept.cube");
        let s = Settings {
            recent_luts: vec![kept.clone(), t.path().join("gone.cube")],
            ..Settings::default()
        };
        store.save(&s).unwrap();
        assert_eq!(store.load().recent_luts, vec![kept]);
    }

    #[test]
    fn set_lut_keeps_a_good_cube() {
        let t = tempfile::tempdir().unwrap();
        let good = t.path().join("show.cube");
        std::fs::write(
            &good,
            "LUT_1D_SIZE 2
0 0 0
1 1 1
",
        )
        .unwrap();
        let mut s = Settings::default();
        s.set_lut(&good).unwrap();
        assert_eq!(s.lut.as_deref(), Some(good.as_path()));
    }

    #[test]
    fn a_file_without_colour_loads_rec709() {
        let (_t, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(
            store.file(),
            "production = \"X\"
",
        )
        .unwrap();
        let s = store.load();
        assert_eq!(s.colour, ThumbnailColour::Rec709);
        assert_eq!(s.lut, None);
        assert!(!s.camera_look);
    }

    #[test]
    fn a_file_without_a_theme_loads_light() {
        let (_t, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(
            store.file(),
            "production = \"X\"
",
        )
        .unwrap();
        assert_eq!(store.load().report_theme, ReportTheme::Light);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_t, store) = store();
        let s = store.load();
        assert_eq!(s, Settings::default());
        assert!(s.output_dir.ends_with("Luna Reports"));
    }

    #[test]
    fn save_then_load_round_trips() {
        let (_t, store) = store();
        let s = sample();
        store.save(&s).unwrap();
        assert!(store.file().is_file());
        assert!(!store.dir().join("settings.toml.tmp").exists());
        assert_eq!(store.load(), s);
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let (_t, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(store.file(), "production = \"Only this\"\n").unwrap();
        let s = store.load();
        assert_eq!(s.production, "Only this");
        assert_eq!(s.output_dir, default_output_dir());
        assert!(s.custom.is_empty());
    }

    #[test]
    fn corrupt_file_gives_defaults_and_keeps_a_backup() {
        let (_t, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(store.file(), "production = [broken").unwrap();
        assert_eq!(store.load(), Settings::default());
        let bak = store.dir().join("settings.toml.bak");
        assert_eq!(
            std::fs::read_to_string(bak).unwrap(),
            "production = [broken"
        );
        // The next save starts clean.
        store.save(&sample()).unwrap();
        assert_eq!(store.load(), sample());
    }

    #[test]
    fn set_logo_copies_into_the_config_folder() {
        let (t, store) = store();
        let src = t.path().join("brand.png");
        std::fs::write(&src, PNG).unwrap();
        let mut s = sample();
        store.set_logo(&mut s, &src).unwrap();
        let dest = store.dir().join("logo.png");
        assert_eq!(s.logo.as_deref(), Some(dest.as_path()));
        assert_eq!(std::fs::read(&dest).unwrap(), PNG);

        store.save(&s).unwrap();
        let text = std::fs::read_to_string(store.file()).unwrap();
        assert!(text.contains("logo = \"logo.png\""), "{text}");
        assert_eq!(store.load(), s);

        // A new logo of another format replaces the old one.
        let jpg = t.path().join("brand.jpeg");
        std::fs::write(&jpg, JPEG).unwrap();
        store.set_logo(&mut s, &jpg).unwrap();
        assert_eq!(s.logo, Some(store.dir().join("logo.jpg")));
        assert!(!dest.exists());
    }

    #[test]
    fn set_logo_rejects_what_the_report_cannot_place() {
        let (t, store) = store();
        let src = t.path().join("notes.txt");
        std::fs::write(&src, "hello").unwrap();
        let mut s = sample();
        let err = store.set_logo(&mut s, &src).unwrap_err();
        assert_eq!(err, "the logo must be a PNG, JPEG or SVG file");
        assert_eq!(s, sample());
        assert!(!store.dir().exists());

        let err = store
            .set_logo(&mut s, &t.path().join("missing.png"))
            .unwrap_err();
        assert!(err.starts_with("the logo file can't be read"), "{err}");
    }

    #[test]
    fn clear_logo_removes_the_copy() {
        let (t, store) = store();
        let src = t.path().join("brand.png");
        std::fs::write(&src, PNG).unwrap();
        let mut s = sample();
        store.set_logo(&mut s, &src).unwrap();
        store.clear_logo(&mut s);
        assert_eq!(s.logo, None);
        assert!(!store.dir().join("logo.png").exists());
        assert!(src.exists(), "the user's original stays");
    }

    #[test]
    fn a_vanished_logo_loads_as_none() {
        let (t, store) = store();
        let src = t.path().join("brand.png");
        std::fs::write(&src, PNG).unwrap();
        let mut s = sample();
        store.set_logo(&mut s, &src).unwrap();
        store.save(&s).unwrap();
        std::fs::remove_file(store.dir().join("logo.png")).unwrap();
        assert_eq!(store.load().logo, None);
    }

    #[test]
    fn platform_folder_is_named_luna() {
        if let Some(store) = SettingsStore::platform() {
            assert!(store.dir().ends_with("Luna"));
            assert!(store.file().ends_with("Luna/settings.toml"));
        }
    }
}
