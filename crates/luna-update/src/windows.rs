//! WinSparkle (`lib\WinSparkle.dll`), through its C API (`winsparkle.h`).

use std::ffi::{CString, c_char, c_int};
use std::path::PathBuf;

use libloading::Library;

use crate::{CHECK_INTERVAL, Event, emit};

type VoidFn = unsafe extern "C" fn();
type IntFn = unsafe extern "C" fn() -> c_int;
type SetCallback = unsafe extern "C" fn(VoidFn);
type SetIntCallback = unsafe extern "C" fn(IntFn);

pub(crate) struct WinSparkle {
    lib: Library,
}

impl WinSparkle {
    /// Loads WinSparkle from Luna's bundled-library folder.
    pub(crate) fn load() -> Result<WinSparkle, String> {
        let path = dll_path().ok_or("Luna's folder is unknown")?;
        if !path.is_file() {
            return Err(format!("no {}", path.display()));
        }
        // SAFETY: WinSparkle's initialisers only set up its own state. The
        // DLL comes from Luna's own install folder.
        let lib = unsafe { Library::new(&path) }
            .map_err(|e| format!("WinSparkle.dll could not be loaded: {e}"))?;
        Ok(WinSparkle { lib })
    }

    /// Configures WinSparkle and starts its automatic checks.
    pub(crate) fn start(&self, feed: &str, public_key: &str, version: &str) -> Result<(), String> {
        let feed = CString::new(feed).map_err(|_| "bad feed URL")?;
        let key = CString::new(public_key).map_err(|_| "bad public key")?;
        let (company, app, version) = (wide("Shaked Lipszyc"), wide("Luna"), wide(version));
        // SAFETY: each symbol is declared with its exact winsparkle.h
        // signature; strings outlive the calls (WinSparkle copies them).
        unsafe {
            self.sym::<unsafe extern "C" fn(*const c_char)>(b"win_sparkle_set_appcast_url\0")?(
                feed.as_ptr(),
            );
            let set_key = self.sym::<unsafe extern "C" fn(*const c_char) -> c_int>(
                b"win_sparkle_set_eddsa_public_key\0",
            )?;
            if set_key(key.as_ptr()) == 0 {
                return Err("WinSparkle rejected the update public key".into());
            }
            self.sym::<unsafe extern "C" fn(*const u16, *const u16, *const u16)>(
                b"win_sparkle_set_app_details\0",
            )?(company.as_ptr(), app.as_ptr(), version.as_ptr());
            // Setting this before init also skips WinSparkle's first-run
            // "check automatically?" question.
            self.sym::<unsafe extern "C" fn(c_int)>(
                b"win_sparkle_set_automatic_check_for_updates\0",
            )?(1);
            self.sym::<unsafe extern "C" fn(c_int)>(b"win_sparkle_set_update_check_interval\0")?(
                CHECK_INTERVAL as c_int,
            );
            let set = |name: &[u8], f: VoidFn| -> Result<(), String> {
                self.sym::<SetCallback>(name).map(|s| s(f))
            };
            set(b"win_sparkle_set_did_find_update_callback\0", found)?;
            set(b"win_sparkle_set_did_not_find_update_callback\0", not_found)?;
            set(b"win_sparkle_set_update_skipped_callback\0", skipped)?;
            set(b"win_sparkle_set_shutdown_request_callback\0", shutdown)?;
            self.sym::<SetIntCallback>(b"win_sparkle_set_can_shutdown_callback\0")?(can_shutdown);
            self.sym::<VoidFn>(b"win_sparkle_init\0")?();
        }
        Ok(())
    }

    /// A user-initiated check with WinSparkle's UI.
    pub(crate) fn check_now(&self) {
        // SAFETY: matches winsparkle.h; safe to call after init.
        if let Ok(f) = unsafe { self.sym::<VoidFn>(b"win_sparkle_check_update_with_ui\0") } {
            unsafe { f() };
        }
    }

    /// SAFETY: `T` must be the symbol's exact function type.
    unsafe fn sym<T: Copy>(&self, name: &[u8]) -> Result<T, String> {
        unsafe { self.lib.get::<T>(name) }
            .map(|s| *s)
            .map_err(|e| format!("WinSparkle.dll is missing a function: {e}"))
    }
}

impl Drop for WinSparkle {
    fn drop(&mut self) {
        // SAFETY: matches winsparkle.h; cleanup is safe even before init.
        if let Ok(f) = unsafe { self.sym::<VoidFn>(b"win_sparkle_cleanup\0") } {
            unsafe { f() };
        }
    }
}

fn dll_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("lib").join("WinSparkle.dll"))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "C" fn found() {
    emit(Event::Found);
}

unsafe extern "C" fn not_found() {
    emit(Event::NotFound);
}

unsafe extern "C" fn skipped() {
    emit(Event::Skipped);
}

unsafe extern "C" fn shutdown() {
    emit(Event::ShutdownRequested);
}

/// Luna can always quit for an update; a run in progress is cancelled.
unsafe extern "C" fn can_shutdown() -> c_int {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_strings_end_in_nul() {
        assert_eq!(wide("Lu"), vec![b'L' as u16, b'u' as u16, 0]);
    }
}
