//! The run handed over at launch (`Luna report <folders…> [options]`).

use std::path::{Path, PathBuf};

use luna_core::cli::ReportArgs;
use luna_core::run::RunRequest;
use luna_core::settings::Settings;

/// A launch's command line. Its first run starts as the window opens. After
/// *Back*, *Create report* on the same folder runs it again over the settings
/// as they are then, so whatever was changed in the window since is used,
/// unless the command line set that same option.
#[derive(Debug, Clone)]
pub struct LaunchRun(ReportArgs);

impl LaunchRun {
    pub fn new(args: ReportArgs) -> Self {
        Self(args)
    }

    /// The first folder, the one the window shows.
    pub fn source(&self) -> Option<&Path> {
        self.0.sources.first().map(PathBuf::as_path)
    }

    /// The run for `source` with the command line laid over `settings`;
    /// `None` when `source` isn't this launch's folder.
    pub fn request(
        &self,
        source: &Path,
        settings: &Settings,
    ) -> Option<Result<RunRequest, String>> {
        (self.source() == Some(source)).then(|| self.0.clone().into_request(settings))
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use luna_core::cli::{Cli, Command};

    use super::*;

    fn launch(args: &[&str]) -> LaunchRun {
        let mut v = vec!["luna", "report"];
        v.extend_from_slice(args);
        match Cli::try_parse_from(v).unwrap().command {
            Command::Report(a) => LaunchRun::new(a),
            _ => unreachable!(),
        }
    }

    fn edited() -> Settings {
        Settings {
            production: "Typed in the window".into(),
            custom: vec![("Unit".into(), "Second Unit".into())],
            copy_to_source: true,
            ..Settings::default()
        }
    }

    #[test]
    fn a_rerun_uses_what_was_changed_in_the_window() {
        let run = launch(&["/clips"]);
        let r = run
            .request(Path::new("/clips"), &edited())
            .unwrap()
            .unwrap();
        assert_eq!(r.settings.production, "Typed in the window");
        assert_eq!(r.settings.custom, [("Unit".into(), "Second Unit".into())]);
        assert!(r.settings.copy_to_source);
    }

    #[test]
    fn options_given_on_the_command_line_still_win() {
        let run = launch(&[
            "/clips",
            "--production",
            "From the command line",
            "--field",
            "Unit=Main",
            "--output",
            "/reports",
        ]);
        let r = run
            .request(Path::new("/clips"), &edited())
            .unwrap()
            .unwrap();
        assert_eq!(r.settings.production, "From the command line");
        assert_eq!(r.settings.custom, [("Unit".into(), "Main".into())]);
        assert_eq!(r.settings.output_dir, PathBuf::from("/reports"));
        assert!(r.settings.copy_to_source);
    }

    #[test]
    fn keeps_the_folders_cameras_and_name() {
        let run = launch(&["/a", "/b", "--camera", "A", "--name", "Day 3"]);
        let r = run
            .request(Path::new("/a"), &Settings::default())
            .unwrap()
            .unwrap();
        assert_eq!(r.sources, [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(r.cameras, ["A"]);
        assert_eq!(r.name.as_deref(), Some("Day 3"));
    }

    #[test]
    fn another_folder_is_not_this_launch() {
        let run = launch(&["/clips"]);
        assert!(run.request(Path::new("/other"), &edited()).is_none());
    }

    #[test]
    fn an_unreadable_logo_is_an_error() {
        let run = launch(&["/clips", "--logo", "/no/such/logo.png"]);
        let err = run
            .request(Path::new("/clips"), &Settings::default())
            .unwrap()
            .unwrap_err();
        assert!(
            err.starts_with("The logo /no/such/logo.png can't be read"),
            "{err}"
        );
    }
}
