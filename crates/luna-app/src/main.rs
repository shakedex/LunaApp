// No console window for the release app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;

use clap::Parser;
use luna_app::backend::{FakeBackend, RunBackend};
use luna_app::core_backend::{CoreBackend, PlatformSettingsStore};
use luna_app::settings_store::{MemorySettingsStore, SettingsStore};
use luna_core::cli::{Cli, Command};
use luna_core::run::{RunRequest, WorkerSpec};
use luna_log::Mode;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--worker") {
        luna_log::init(Mode::Worker);
        luna_worker::worker_main();
        return ExitCode::SUCCESS;
    }

    luna_log::init(Mode::App);
    tracing::info!(
        "Luna {} {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    // `LUNA_FAKE_BACKEND=1` simulates runs (UI work); otherwise the real pipeline.
    let fake = std::env::var_os("LUNA_FAKE_BACKEND").is_some();
    let backend: Arc<dyn RunBackend> = if fake {
        Arc::new(FakeBackend::from_env())
    } else {
        match WorkerSpec::current_exe() {
            Ok(spec) => Arc::new(CoreBackend::new(spec)),
            Err(e) => {
                tracing::error!("Luna couldn't find its own executable: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    let store: Rc<dyn SettingsStore> = match PlatformSettingsStore::new() {
        Some(s) if !fake => Rc::new(s),
        _ => Rc::new(MemorySettingsStore::default()),
    };

    let preset = preset(&*store);
    // Only the real app updates itself; a simulated run (UI work) doesn't.
    match luna_app::ui::run(backend, store, !fake, preset) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("Luna couldn't open its window: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `Luna.exe report <folders…> [options]`: the run to start as the window
/// opens. Errors become a sentence for the window; there is no console.
fn preset(store: &dyn SettingsStore) -> Option<Result<RunRequest, String>> {
    let first = std::env::args().nth(1)?;
    if !matches!(first.as_str(), "report" | "inspect" | "mcp") {
        return None;
    }
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // clap's first line, without its "error: " prefix.
            let text = e.to_string();
            let line = text.lines().next().unwrap_or_default();
            return Some(Err(line.trim_start_matches("error: ").to_owned()));
        }
    };
    Some(match cli.command {
        Command::Report(args) => args.into_request(&store.load()),
        Command::Inspect(_) | Command::Mcp => Err("Use luna-cli for inspect and mcp".into()),
    })
}
