//! Headless run for development:
//! `cargo run -p luna-core --example report -- <source> [output dir] [--dark]`.
//! The same binary serves as its own worker (`--worker`), like the app does.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use luna_core::run::{Progress, WorkerSpec, run};
use luna_core::settings::Settings;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let dark = args.iter().any(|a| a == "--dark");
    args.retain(|a| a != "--dark");
    if args.first().map(String::as_str) == Some("--worker") {
        luna_worker::worker_main();
        return;
    }
    let Some(source) = args.first() else {
        eprintln!("usage: report <source folder> [output folder] [--dark]");
        std::process::exit(2);
    };
    let mut settings = Settings::load();
    if let Some(out) = args.get(1) {
        settings.output_dir = PathBuf::from(out);
    }
    if dark {
        settings.report_theme = luna_core::settings::ReportTheme::Dark;
    }
    let workers = WorkerSpec::current_exe().expect("current exe");
    let start = std::time::Instant::now();
    let progress = |p: Progress| match p {
        Progress::FilesFound(n) => eprintln!("{n} files found"),
        Progress::ClipsTotal(n) => eprintln!("{n} clips"),
        Progress::ClipDone {
            done,
            total,
            problems,
        } => eprintln!("{done}/{total} clips ({problems} with problems)"),
    };
    match run(
        source.as_ref(),
        &settings,
        &workers,
        &progress,
        &AtomicBool::new(false),
    ) {
        Ok(outcome) => {
            println!("{}", outcome.summary);
            println!("{}", outcome.files.pdf.display());
            println!("{}", outcome.files.html.display());
            println!("{}", outcome.files.csv.display());
            eprintln!("took {:.1} s", start.elapsed().as_secs_f64());
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
