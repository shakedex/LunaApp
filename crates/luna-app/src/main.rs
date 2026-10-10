// No console window for the release app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;

use luna_app::backend::{FakeBackend, RunBackend};
use luna_app::core_backend::{CoreBackend, PlatformSettingsStore};
use luna_app::settings_store::{MemorySettingsStore, SettingsStore};
use luna_core::run::WorkerSpec;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--worker") {
        luna_worker::worker_main();
        return ExitCode::SUCCESS;
    }

    // `LUNA_FAKE_BACKEND=1` simulates runs (UI work); otherwise the real pipeline.
    let fake = std::env::var_os("LUNA_FAKE_BACKEND").is_some();
    let backend: Arc<dyn RunBackend> = if fake {
        Arc::new(FakeBackend::from_env())
    } else {
        match WorkerSpec::current_exe() {
            Ok(spec) => Arc::new(CoreBackend::new(spec)),
            Err(e) => {
                eprintln!("Luna couldn't find its own executable: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    let store: Rc<dyn SettingsStore> = match PlatformSettingsStore::new() {
        Some(s) if !fake => Rc::new(s),
        _ => Rc::new(MemorySettingsStore::default()),
    };

    match luna_app::ui::run(backend, store) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Luna couldn't open its window: {e}");
            ExitCode::FAILURE
        }
    }
}
