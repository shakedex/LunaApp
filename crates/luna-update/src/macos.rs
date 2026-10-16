//! Sparkle (`Luna.app/Contents/Frameworks/Sparkle.framework`), through the
//! small Objective-C shim in `shim/luna_sparkle.m`. The feed URL, public key
//! and check schedule come from Info.plist (written by `cargo xtask package`).

use std::ffi::{CString, c_char, c_int};
use std::path::PathBuf;

use crate::{Event, emit};

unsafe extern "C" {
    fn luna_sparkle_start(framework_path: *const c_char, cb: extern "C" fn(c_int)) -> c_int;
    fn luna_sparkle_check();
}

pub(crate) struct Sparkle;

impl Sparkle {
    pub(crate) fn start() -> Result<Sparkle, String> {
        let path = framework_path().ok_or("Luna's folder is unknown")?;
        if !path.is_dir() {
            return Err(format!("no {}", path.display()));
        }
        let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "bad path")?;
        // SAFETY: the shim copies the path; `event` lives for the process.
        if unsafe { luna_sparkle_start(c.as_ptr(), event) } == 0 {
            return Err(format!("Sparkle could not start from {}", path.display()));
        }
        Ok(Sparkle)
    }

    pub(crate) fn check_now(&self) {
        // SAFETY: the controller exists once start succeeded.
        unsafe { luna_sparkle_check() };
    }
}

/// `<app>/Contents/Frameworks/Sparkle.framework`, from `Contents/MacOS/Luna`.
fn framework_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(
        exe.parent()?
            .parent()?
            .join("Frameworks")
            .join("Sparkle.framework"),
    )
}

extern "C" fn event(code: c_int) {
    match code {
        0 => emit(Event::Found),
        1 => emit(Event::NotFound),
        2 => emit(Event::Skipped),
        _ => {}
    }
}
