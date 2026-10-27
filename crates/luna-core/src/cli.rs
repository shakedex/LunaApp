//! The command line shared by `luna-cli` and `Luna.exe`: `report`, `inspect`
//! and `mcp`. Values given here apply to one run and are never saved.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use luna_report::Logo;

use crate::run::RunRequest;
use crate::settings::{ReportTheme, Settings};

#[derive(Debug, Parser)]
#[command(name = "luna", version, about = "Camera reports from camera originals")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

// Parsed once per process; boxing the report arguments buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Make a report from one or more folders.
    Report(ReportArgs),
    /// List the cameras, cards and clips in one or more folders.
    Inspect(InspectArgs),
    /// Serve Luna's tools to an AI agent over MCP (stdio).
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Default, Args)]
pub struct ReportArgs {
    /// Folders to scan, recursively.
    #[arg(required = true)]
    pub sources: Vec<PathBuf>,
    /// Only this camera (repeatable): a letter such as A, or a camera name from inspect.
    #[arg(long = "camera", value_name = "NAME")]
    pub cameras: Vec<String>,
    #[arg(long)]
    pub production: Option<String>,
    #[arg(long)]
    pub company: Option<String>,
    #[arg(long)]
    pub author: Option<String>,
    /// PNG, JPEG or SVG.
    #[arg(long)]
    pub logo: Option<PathBuf>,
    /// A header field as LABEL=VALUE (repeatable), after the saved fields. A
    /// saved field with the same label (any case) takes this value instead.
    #[arg(long = "field", value_name = "LABEL=VALUE", value_parser = parse_field)]
    pub fields: Vec<(String, String)>,
    /// Leave out the saved header fields.
    #[arg(long)]
    pub no_saved_fields: bool,
    /// Folder the report folder is written into.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Report folder name (default: production name and run date).
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, value_enum)]
    pub theme: Option<Theme>,
    /// Print clip warnings and unreadable files in the report.
    #[arg(long)]
    pub problems: bool,
    /// Print the result as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct InspectArgs {
    /// Folders to scan, recursively.
    #[arg(required = true)]
    pub sources: Vec<PathBuf>,
    /// Print the result as JSON.
    #[arg(long)]
    pub json: bool,
}

/// `LABEL=VALUE`, split on the first `=`; the label can't be empty.
pub fn parse_field(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((label, value)) if !label.trim().is_empty() => {
            Ok((label.trim().to_owned(), value.trim().to_owned()))
        }
        _ => Err(format!("\"{s}\" isn't LABEL=VALUE")),
    }
}

impl ReportArgs {
    /// The run these arguments ask for: `saved` with the arguments laid over
    /// it. `saved` itself is left as it is.
    pub fn into_request(self, saved: &Settings) -> Result<RunRequest, String> {
        let mut settings = saved.clone();
        let set = |field: &mut String, value: Option<String>| {
            if let Some(v) = value {
                *field = v;
            }
        };
        set(&mut settings.production, self.production);
        set(&mut settings.company, self.company);
        set(&mut settings.author, self.author);
        if let Some(logo) = self.logo {
            let bytes = std::fs::read(&logo)
                .map_err(|e| format!("The logo {} can't be read: {e}", logo.display()))?;
            Logo::from_bytes(bytes)
                .map_err(|e| format!("The logo {} can't be used: {e}", logo.display()))?;
            settings.logo = Some(logo);
        }
        if self.no_saved_fields {
            settings.custom.clear();
        }
        for (label, value) in self.fields {
            let same = |(l, _): &&mut (String, String)| l.trim().eq_ignore_ascii_case(&label);
            match settings.custom.iter_mut().find(same) {
                Some(saved) => saved.1 = value,
                None => settings.custom.push((label, value)),
            }
        }
        if let Some(out) = self.output {
            settings.output_dir = out;
        }
        if let Some(theme) = self.theme {
            settings.report_theme = match theme {
                Theme::Light => ReportTheme::Light,
                Theme::Dark => ReportTheme::Dark,
            };
        }
        settings.show_problems |= self.problems;
        Ok(RunRequest {
            sources: self.sources,
            cameras: self.cameras,
            settings,
            name: self.name.filter(|n| !n.trim().is_empty()),
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn args(extra: &[&str]) -> ReportArgs {
        let mut v = vec!["luna", "report", "E:/A"];
        v.extend_from_slice(extra);
        match Cli::try_parse_from(v).unwrap().command {
            Command::Report(a) => a,
            _ => unreachable!(),
        }
    }

    fn saved() -> Settings {
        Settings {
            production: "Old".into(),
            author: "Dana".into(),
            custom: vec![("Day".into(), "1".into())],
            ..Settings::default()
        }
    }

    #[test]
    fn overrides_apply_and_rest_comes_from_saved() {
        let s = saved();
        let req = args(&["--production", "New"]).into_request(&s).unwrap();
        assert_eq!(req.settings.production, "New");
        assert_eq!(req.settings.author, "Dana");
        assert_eq!(s.production, "Old", "saved settings untouched");
    }

    #[test]
    fn fields_follow_saved_fields_in_order() {
        let req = args(&["--field", "Unit=2nd", "--field", "Cam=A"])
            .into_request(&saved())
            .unwrap();
        let pairs = |r: &RunRequest| {
            r.settings
                .custom
                .iter()
                .map(|(l, v)| format!("{l}={v}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(pairs(&req), ["Day=1", "Unit=2nd", "Cam=A"]);
        let req = args(&[
            "--field",
            "Unit=2nd",
            "--field",
            "Cam=A",
            "--no-saved-fields",
        ])
        .into_request(&saved())
        .unwrap();
        assert_eq!(pairs(&req), ["Unit=2nd", "Cam=A"]);
    }

    #[test]
    fn a_field_replaces_the_saved_field_of_the_same_label() {
        let req = args(&["--field", "DAY=01", "--field", "Unit=2nd"])
            .into_request(&saved())
            .unwrap();
        assert_eq!(
            req.settings.custom,
            [
                ("Day".to_owned(), "01".to_owned()),
                ("Unit".to_owned(), "2nd".to_owned())
            ]
        );
    }

    #[test]
    fn field_value_may_contain_equals() {
        assert_eq!(
            parse_field("Note=a=b"),
            Ok(("Note".to_string(), "a=b".to_string()))
        );
        assert!(parse_field("=x").is_err());
        assert!(parse_field("nolabel").is_err());
    }

    #[test]
    fn missing_logo_is_an_error() {
        let err = args(&["--logo", "nope.png"])
            .into_request(&saved())
            .unwrap_err();
        assert!(err.contains("nope.png"), "{err}");
    }

    #[test]
    fn parses_full_command_line() {
        let cli = Cli::try_parse_from([
            "luna",
            "report",
            "E:/A",
            "F:/B",
            "--camera",
            "A",
            "--theme",
            "dark",
            "--json",
            "--name",
            "A Cam",
            "--output",
            "D:/Out",
            "--problems",
        ])
        .unwrap();
        let Command::Report(a) = cli.command else {
            panic!("report expected")
        };
        assert_eq!(a.sources.len(), 2);
        assert_eq!(a.cameras, ["A"]);
        assert_eq!(a.theme, Some(Theme::Dark));
        assert!(a.json);
        let req = a.into_request(&Settings::default()).unwrap();
        assert_eq!(req.settings.report_theme, ReportTheme::Dark);
        assert!(req.settings.show_problems);
        assert_eq!(req.name.as_deref(), Some("A Cam"));
        assert_eq!(req.settings.output_dir, std::path::PathBuf::from("D:/Out"));
        assert_eq!(req.cameras, ["A"]);
    }

    #[test]
    fn report_needs_a_source() {
        assert!(Cli::try_parse_from(["luna", "report"]).is_err());
    }
}
