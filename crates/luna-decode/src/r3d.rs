//! RED R3D through the closed R3D add-on, loaded at run time.
//!
//! Luna's MIT core never links RED's SDK: R3D support is a separate library
//! (`luna_r3d` / `r3d`, built from the private `r3d-sdk` crate) that exports
//! the small C ABI in `r3d-sdk/include/luna_r3d.h`. We look for it in
//! Luna's bundled-library folder (`util::lib_dir`), or at the path in
//! `LUNA_R3D_ADDON` for development.
//! Its SDK folder (holding `REDR3D-x64.dll` / `REDR3D.dylib`) is the add-on's
//! own folder unless `LUNA_R3D_SDK_DIR` says otherwise.
//!
//! Frames are interleaved RGB16, REDWideGamutRGB / Log3G10, as shot.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use libloading::Library;
use luna_formats::{ClipFiles, ColourHint, Gamut, Probe, Transfer};

use crate::util;
use crate::{DecodeError, Decoded, Frame};

/// The ABI version this core speaks (`LUNA_R3D_ABI_VERSION`).
const ABI_VERSION: u32 = 1;

pub(crate) const NOT_INSTALLED: &str = "R3D support is a separate add-on and isn't installed";

const OK: i32 = 0;
const E_FRAME_RANGE: i32 = 5;

/// Decode scales, finest first (`enum luna_r3d_scale`).
const SCALES: [(u32, u32); 5] = [(0, 1), (1, 2), (2, 4), (3, 8), (4, 16)];

pub(crate) const HINT: ColourHint = ColourHint {
    transfer: Some(Transfer::Log3G10),
    gamut: Some(Gamut::RedWideGamutRgb),
};

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Info {
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub video_fps: f32,
    pub video_tracks: u32,
    pub rmd_present: u8,
    pub timecode_fps: f32,
    pub start_absolute_timecode: [c_char; 16],
    pub end_absolute_timecode: [c_char; 16],
    pub start_edge_timecode: [c_char; 16],
    pub end_edge_timecode: [c_char; 16],
}

impl Info {
    pub(crate) fn text(field: &[c_char; 16]) -> Option<String> {
        let bytes: Vec<u8> = field
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect();
        let s = String::from_utf8_lossy(&bytes).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// One metadata value as the add-on reports it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Int(u32),
    Float(f32),
    Text(String),
}

type MetaCb = extern "C" fn(*mut c_void, *const c_char, u32, u32, f32, *const c_char);

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type InitFn = unsafe extern "C" fn(*const c_char) -> i32;
type OpenFn = unsafe extern "C" fn(*const c_char, *mut *mut c_void) -> i32;
type CloseFn = unsafe extern "C" fn(*mut c_void);
type InfoFn = unsafe extern "C" fn(*const c_void, *mut Info) -> i32;
type MetadataFn = unsafe extern "C" fn(*const c_void, Option<MetaCb>, *mut c_void) -> i32;
type DecodeSizeFn = unsafe extern "C" fn(*const c_void, u32, *mut u32, *mut u32) -> i32;
type DecodeFn = unsafe extern "C" fn(*const c_void, u64, u32, *mut u16, usize) -> i32;
type LastErrorFn = unsafe extern "C" fn(*mut c_char, usize) -> usize;

/// The loaded add-on. The library stays loaded for the life of the process,
/// so the function pointers stay valid.
pub(crate) struct Addon {
    _lib: Library,
    open: OpenFn,
    close: CloseFn,
    info: InfoFn,
    metadata: MetadataFn,
    decode_size: DecodeSizeFn,
    decode: DecodeFn,
    last_error: LastErrorFn,
}

/// The add-on, loaded and initialised once per process.
pub(crate) fn addon() -> Result<&'static Addon, DecodeError> {
    static ADDON: OnceLock<Result<Addon, DecodeError>> = OnceLock::new();
    ADDON
        .get_or_init(|| util::guard("R3D", load))
        .as_ref()
        .map_err(Clone::clone)
}

fn addon_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["luna_r3d.dll", "r3d.dll"]
    } else if cfg!(target_os = "macos") {
        &["libluna_r3d.dylib", "libr3d.dylib"]
    } else {
        &["libluna_r3d.so", "libr3d.so"]
    }
}

/// Where the add-on may be, in order.
fn addon_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("LUNA_R3D_ADDON") {
        out.push(PathBuf::from(p));
    }
    if let Some(lib) = util::lib_dir() {
        for name in addon_names() {
            out.push(lib.join(name));
        }
    }
    out
}

fn load() -> Result<Addon, DecodeError> {
    let Some(path) = addon_candidates().into_iter().find(|p| p.is_file()) else {
        return Err(DecodeError::Unsupported(NOT_INSTALLED.into()));
    };
    let broken = |e: &dyn std::fmt::Display| {
        DecodeError::Unsupported(format!("The R3D add-on could not be loaded: {e}"))
    };
    // SAFETY: loading the add-on runs its initialisers. It is our own library,
    // found only in Luna's bundled-library folder (or named by the developer).
    let lib = unsafe { Library::new(&path) }.map_err(|e| broken(&e))?;
    // SAFETY: each symbol is declared with the exact signature from luna_r3d.h.
    unsafe {
        let abi: AbiVersionFn = *lib
            .get::<AbiVersionFn>(b"luna_r3d_abi_version\0")
            .map_err(|e| broken(&e))?;
        let version = abi();
        if version != ABI_VERSION {
            return Err(DecodeError::Unsupported(format!(
                "The installed R3D add-on is the wrong version (ABI {version}, Luna needs {ABI_VERSION})"
            )));
        }
        macro_rules! sym {
            ($name:literal, $ty:ty) => {
                *lib.get::<$ty>(concat!($name, "\0").as_bytes())
                    .map_err(|e| broken(&e))?
            };
        }
        let init: InitFn = sym!("luna_r3d_init", InitFn);
        let addon = Addon {
            open: sym!("luna_r3d_open", OpenFn),
            close: sym!("luna_r3d_close", CloseFn),
            info: sym!("luna_r3d_info_get", InfoFn),
            metadata: sym!("luna_r3d_metadata", MetadataFn),
            decode_size: sym!("luna_r3d_decode_size", DecodeSizeFn),
            decode: sym!("luna_r3d_decode", DecodeFn),
            last_error: sym!("luna_r3d_last_error", LastErrorFn),
            _lib: lib,
        };
        let sdk_dir = sdk_dir(&path);
        let c_dir = c_path(&sdk_dir)?;
        if init(c_dir.as_ptr()) != OK {
            return Err(DecodeError::Unsupported(format!(
                "The R3D add-on could not start RED's decoder: {}",
                addon.error()
            )));
        }
        Ok(addon)
    }
}

/// RED's libraries sit next to the add-on unless `LUNA_R3D_SDK_DIR` says
/// otherwise (development builds keep them in the SDK download).
fn sdk_dir(addon: &Path) -> PathBuf {
    std::env::var_os("LUNA_R3D_SDK_DIR")
        .map(PathBuf::from)
        .or_else(|| addon.parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

fn c_path(p: &Path) -> Result<CString, DecodeError> {
    CString::new(util::path_str(p)?)
        .map_err(|_| DecodeError::Failed(format!("Path contains a NUL byte: {}", p.display())))
}

impl Addon {
    /// The calling thread's last add-on error message.
    fn error(&self) -> String {
        let mut buf = vec![0 as c_char; 512];
        // SAFETY: the buffer is writable for its full length.
        unsafe { (self.last_error)(buf.as_mut_ptr(), buf.len()) };
        // SAFETY: the add-on NUL-terminates within `len`.
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if s.is_empty() {
            "unknown error".into()
        } else {
            s
        }
    }

    pub(crate) fn open(&'static self, path: &Path) -> Result<Clip, DecodeError> {
        let c = c_path(path)?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: valid C string and out pointer.
        if unsafe { (self.open)(c.as_ptr(), &mut raw) } != OK || raw.is_null() {
            return Err(DecodeError::Failed(format!(
                "Could not open the R3D clip: {}",
                self.error()
            )));
        }
        Ok(Clip { addon: self, raw })
    }
}

/// An open clip; closed on drop.
pub(crate) struct Clip {
    addon: &'static Addon,
    raw: *mut c_void,
}

impl Drop for Clip {
    fn drop(&mut self) {
        // SAFETY: `raw` came from luna_r3d_open and no call on it is running.
        unsafe { (self.addon.close)(self.raw) }
    }
}

extern "C" fn collect(
    ctx: *mut c_void,
    key: *const c_char,
    kind: u32,
    i: u32,
    f: f32,
    s: *const c_char,
) {
    let _ = std::panic::catch_unwind(|| {
        if ctx.is_null() || key.is_null() {
            return;
        }
        // SAFETY: ctx is the Vec we passed; key/s are valid during the call.
        let out = unsafe { &mut *ctx.cast::<Vec<(String, Value)>>() };
        let key = unsafe { CStr::from_ptr(key) }
            .to_string_lossy()
            .into_owned();
        let value = match kind {
            1 => Value::Int(i),
            3 => Value::Float(f),
            _ if s.is_null() => Value::Text(String::new()),
            _ => Value::Text(unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()),
        };
        out.push((key, value));
    });
}

impl Clip {
    pub(crate) fn info(&self) -> Result<Info, DecodeError> {
        let mut info = Info::default();
        // SAFETY: valid clip and out pointer.
        if unsafe { (self.addon.info)(self.raw, &mut info) } != OK {
            return Err(DecodeError::Failed(self.addon.error()));
        }
        Ok(info)
    }

    pub(crate) fn metadata(&self) -> Result<Vec<(String, Value)>, DecodeError> {
        let mut out: Vec<(String, Value)> = Vec::new();
        let ctx = (&mut out as *mut Vec<(String, Value)>).cast();
        // SAFETY: the callback only touches `out` during this call.
        if unsafe { (self.addon.metadata)(self.raw, Some(collect), ctx) } != OK {
            return Err(DecodeError::Failed(self.addon.error()));
        }
        Ok(out)
    }

    fn decode_size(&self, scale: u32) -> Result<(u32, u32), DecodeError> {
        let (mut w, mut h) = (0, 0);
        // SAFETY: valid pointers.
        if unsafe { (self.addon.decode_size)(self.raw, scale, &mut w, &mut h) } != OK {
            return Err(DecodeError::Failed(self.addon.error()));
        }
        Ok((w, h))
    }

    /// The coarsest scale still at least `min_width` wide.
    fn scale_for(&self, min_width: u32) -> Result<(u32, u32, u32), DecodeError> {
        let mut best = None;
        for &(scale, _) in SCALES.iter() {
            let (w, h) = self.decode_size(scale)?;
            if best.is_none() || w >= min_width {
                best = Some((scale, w, h));
            }
        }
        best.ok_or_else(|| DecodeError::Failed("No R3D decode size".into()))
    }

    fn decode(&self, frame: u64, scale: u32, w: u32, h: u32) -> Result<Vec<u16>, DecodeError> {
        let len = w as usize * h as usize * 3;
        if len == 0 {
            return Err(DecodeError::Failed("R3D decode size is empty".into()));
        }
        let mut out = vec![0u16; len];
        // SAFETY: `out` holds exactly `len` u16 values.
        let rc = unsafe { (self.addon.decode)(self.raw, frame, scale, out.as_mut_ptr(), len) };
        match rc {
            OK => Ok(out),
            E_FRAME_RANGE => Err(DecodeError::Failed(format!(
                "Frame {frame} is past the end of the R3D clip"
            ))),
            _ => Err(DecodeError::Failed(format!(
                "Could not decode R3D frame {frame}: {}",
                self.addon.error()
            ))),
        }
    }
}

pub(crate) fn decode(
    clip: &ClipFiles,
    _probe: &Probe,
    positions: &[u64],
    min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let setup = addon().and_then(|a| {
        let c = a.open(&clip.main)?;
        let info = c.info()?;
        let scale = c.scale_for(min_width)?;
        Ok((c, info, scale))
    });
    let (c, info, (scale, w, h)) = match setup {
        Ok(s) => s,
        Err(e) => return util::all_err(positions.len(), e),
    };
    util::per_frame("R3D", positions, info.frame_count, |i| {
        let data = c.decode(i, scale, w, h)?;
        Ok(Frame {
            image: Decoded::Rgb16 {
                width: w,
                height: h,
                data,
                transfer: Transfer::Log3G10,
                gamut: Gamut::RedWideGamutRgb,
            },
            hint: HINT,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_text_fields() {
        let mut f = [0 as c_char; 16];
        for (d, s) in f.iter_mut().zip(b"01:02:03:04") {
            *d = *s as c_char;
        }
        assert_eq!(Info::text(&f).as_deref(), Some("01:02:03:04"));
        assert_eq!(Info::text(&[0; 16]), None);
        // no NUL at all: all 16 bytes
        let full = [b'1' as c_char; 16];
        assert_eq!(Info::text(&full).map(|s| s.len()), Some(16));
    }

    #[test]
    fn missing_addon_is_unsupported() {
        // Only meaningful when no add-on is configured for this test run.
        if std::env::var_os("LUNA_R3D_ADDON").is_some() {
            return;
        }
        match addon() {
            Err(DecodeError::Unsupported(m)) => assert_eq!(m, NOT_INSTALLED),
            Err(e) => panic!("unexpected error {e}"),
            Ok(_) => {} // an add-on is installed next to the test binary
        }
    }
}
