// No console window for the release app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;

use luna_app::backend::{FakeBackend, RunBackend};
use luna_app::settings_store::{MemorySettingsStore, SettingsStore};

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--worker") {
        // integrator: --worker entry. Call `luna_worker::worker_main()` here
        // once it exists; until then a worker process exits immediately.
        return ExitCode::SUCCESS;
    }

    // integrator: wire the real luna-core backend here and keep the fake one
    // behind `LUNA_FAKE_BACKEND=1`. There is no real backend yet, so the fake
    // runs either way. `LUNA_FAKE_OUTCOME=fail` makes fake runs fail.
    let backend: Arc<dyn RunBackend> = Arc::new(FakeBackend::from_env());

    // integrator: replace with the luna_core::settings-backed store.
    let store: Rc<dyn SettingsStore> = Rc::new(MemorySettingsStore::default());

    match luna_app::ui::run(backend, store) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Luna couldn't open its window: {e}");
            ExitCode::FAILURE
        }
    }
}
