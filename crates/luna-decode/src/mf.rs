//! H.264 / HEVC in MOV/MP4 through Windows Media Foundation's own decoders.
//!
//! One `IMFSourceReader` per call, with the decoder's native output (no video
//! processor): NV12 for 8-bit 4:2:0, P010 for 10-bit 4:2:0, and Y210/P210
//! when the HEVC decoder offers 10-bit 4:2:2. For each position we seek to its
//! time (the reader lands on the key frame before it) and read forward until
//! the frame at or after that time.
//!
//! Microsoft's H.264 decoder has no High 4:2:2 / High 10 support; a stream no
//! decoder on this computer takes is reported as unsupported.

use std::mem::ManuallyDrop;

use luna_formats::{ClipFiles, Codec, ColourHint, Gamut, Probe, Transfer};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Variant::VT_I8;
use windows::core::{GUID, Interface, PCWSTR};

use crate::util;
use crate::{DecodeError, Decoded, Frame, YcbcrMatrix};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
/// Frames to read forward from a key frame before giving up on a position.
const MAX_FORWARD: u32 = 1200;

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let _com = match Com::init() {
        Ok(c) => c,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let _mf = match MfStartup::init() {
        Ok(m) => m,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let reader = match Reader::open(clip, probe) {
        Ok(r) => r,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let count = probe
        .meta
        .frame_count
        .or_else(|| reader.duration_frames())
        .unwrap_or(u64::MAX);
    let mut reader = reader;
    util::per_frame("Media Foundation", positions, count, |i| reader.frame_at(i))
}

/// COM on this thread (multithreaded apartment), undone on drop.
struct Com {
    uninit: bool,
}

impl Com {
    fn init() -> Result<Com, DecodeError> {
        // SAFETY: plain COM initialisation for this thread.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            // Someone already made this thread single-threaded; MF works there too.
            return Ok(Com { uninit: false });
        }
        hr.ok()
            .map_err(|e| DecodeError::Failed(format!("Could not start COM: {e}")))?;
        Ok(Com { uninit: true })
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.uninit {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() }
        }
    }
}

struct MfStartup;

impl MfStartup {
    fn init() -> Result<MfStartup, DecodeError> {
        // SAFETY: balanced by MFShutdown on drop.
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }
            .map_err(|e| DecodeError::Failed(format!("Could not start Media Foundation: {e}")))?;
        Ok(MfStartup)
    }
}

impl Drop for MfStartup {
    fn drop(&mut self) {
        // SAFETY: balances MFStartup.
        let _ = unsafe { MFShutdown() };
    }
}

/// What the reader outputs after negotiation.
#[derive(Debug, Clone, Copy)]
struct Output {
    format: Format,
    /// Coded frame size (buffer layout).
    width: u32,
    height: u32,
    /// Display area within it.
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    stride: Option<i32>,
    matrix: YcbcrMatrix,
    full_range: bool,
}

struct Reader {
    reader: IMFSourceReader,
    out: Output,
    /// 100 ns units per frame.
    frame_100ns: f64,
    hint: ColourHint,
    label: String,
    decoded_any: bool,
    start: Option<i64>,
    /// Set once the decoder has failed for good; later positions get the same
    /// error instead of calling into a reader in its error state.
    broken: Option<DecodeError>,
}

impl Reader {
    fn open(clip: &ClipFiles, probe: &Probe) -> Result<Reader, DecodeError> {
        let wide: Vec<u16> = clip
            .main
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let fail = |what: &str, e: windows::core::Error| {
            DecodeError::Failed(format!("Media Foundation could not {what}: {e}"))
        };
        // SAFETY: all calls below are plain COM calls on live interfaces.
        unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 1).map_err(|e| fail("start", e))?;
            let attrs = attrs.ok_or_else(|| DecodeError::Failed("no attributes".into()))?;
            // Native decoder output only; no colour conversion by MF.
            attrs
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 0)
                .map_err(|e| fail("start", e))?;
            let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attrs)
                .map_err(|e| fail("open the file", e))?;
            reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .map_err(|e| fail("select the video", e))?;
            reader
                .SetStreamSelection(VIDEO, true)
                .map_err(|e| fail("select the video", e))?;
            let native = reader
                .GetNativeMediaType(VIDEO, 0)
                .map_err(|e| fail("read the video format", e))?;
            let label = profile_label(&probe.codec, &native);
            let mut negotiated = false;
            for subtype in [MFVideoFormat_P010, MFVideoFormat_NV12] {
                let mt = MFCreateMediaType().map_err(|e| fail("start", e))?;
                mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                    .map_err(|e| fail("start", e))?;
                mt.SetGUID(&MF_MT_SUBTYPE, &subtype)
                    .map_err(|e| fail("start", e))?;
                if reader.SetCurrentMediaType(VIDEO, None, &mt).is_ok() {
                    negotiated = true;
                    break;
                }
            }
            if !negotiated {
                return Err(unsupported(&label));
            }
            let out = output(&reader)?;
            let fps = probe
                .meta
                .frame_rate
                .map(|r| r.as_f64())
                .filter(|f| *f > 0.0)
                .or_else(|| ratio_attr(&native, &MF_MT_FRAME_RATE))
                .unwrap_or(25.0);
            Ok(Reader {
                reader,
                out,
                frame_100ns: 1e7 / fps,
                hint: hint_for(probe, &native),
                label,
                decoded_any: false,
                start: None,
                broken: None,
            })
        }
    }

    /// Clip length in frames from the presentation duration, when the probe
    /// didn't say.
    fn duration_frames(&self) -> Option<u64> {
        let d = self.duration_100ns()?;
        let n = (d as f64 / self.frame_100ns).round() as u64;
        (n > 0).then_some(n)
    }

    /// Presentation duration in 100 ns units.
    fn duration_100ns(&self) -> Option<i64> {
        // SAFETY: plain COM call.
        let pv = unsafe {
            self.reader
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
        }
        .ok()?;
        // SAFETY: MF_PD_DURATION is a VT_UI8.
        let d = unsafe { pv.Anonymous.Anonymous.Anonymous.uhVal };
        i64::try_from(d).ok().filter(|d| *d > 0)
    }

    /// One `ReadSample`: (flags, timestamp, sample). A failure before any frame
    /// has decoded means no decoder takes this stream.
    fn read_one(&mut self, frame: u64) -> Result<(u32, i64, Option<IMFSample>), DecodeError> {
        if let Some(e) = &self.broken {
            return Err(e.clone());
        }
        let mut flags = 0u32;
        let mut ts = 0i64;
        let mut sample: Option<IMFSample> = None;
        // SAFETY: out pointers are valid locals.
        let read = unsafe {
            self.reader.ReadSample(
                VIDEO,
                0,
                None,
                Some(&mut flags),
                Some(&mut ts),
                Some(&mut sample),
            )
        };
        let errored = read.is_err() || flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0;
        if errored {
            let e = if self.decoded_any {
                DecodeError::Failed(format!(
                    "Could not decode frame {frame}{}",
                    read.err().map(|e| format!(": {e}")).unwrap_or_default()
                ))
            } else {
                unsupported(&self.label)
            };
            self.broken = Some(e.clone());
            return Err(e);
        }
        if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
            // SAFETY: plain COM call.
            self.out = unsafe { output(&self.reader) }?;
        }
        if sample.is_some() {
            self.decoded_any = true;
        }
        Ok((flags, ts, sample))
    }

    /// Presentation time of the first frame. MP4s with B-frames often start
    /// after zero (composition offset), so frame times count from here.
    fn start(&mut self) -> Result<i64, DecodeError> {
        if let Some(t) = self.start {
            return Ok(t);
        }
        let mut t0 = 0;
        for _ in 0..MAX_FORWARD {
            let (flags, ts, sample) = self.read_one(0)?;
            if sample.is_some() {
                t0 = ts;
                break;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
        }
        self.start = Some(t0);
        Ok(t0)
    }

    fn frame_at(&mut self, frame: u64) -> Result<Frame, DecodeError> {
        let start = self.start()?;
        let target = start + (frame as f64 * self.frame_100ns).round() as i64;
        let half = (self.frame_100ns / 2.0) as i64;
        // Seeking at or past the duration is refused; land inside the clip and
        // read forward (the last frame is returned when the target is past it).
        let seek = match self.duration_100ns() {
            Some(d) => target.min(d - self.frame_100ns.round() as i64).max(0),
            None => target,
        };
        let pos = propvariant_i64(seek);
        // SAFETY: the PROPVARIANT is a VT_I8 that lives across the call.
        unsafe { self.reader.SetCurrentPosition(&GUID::zeroed(), &pos) }
            .map_err(|e| DecodeError::Failed(format!("Could not seek to frame {frame}: {e}")))?;
        let mut last: Option<IMFSample> = None;
        for _ in 0..MAX_FORWARD {
            let (flags, ts, sample) = self.read_one(frame)?;
            if let Some(s) = sample {
                if ts + half >= target {
                    return self.to_frame(&s);
                }
                last = Some(s);
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
        }
        // Past the end (or a bad index): the last frame we saw.
        match last {
            Some(s) => self.to_frame(&s),
            None => Err(DecodeError::Failed(format!(
                "Frame {frame} could not be found"
            ))),
        }
    }

    fn to_frame(&self, sample: &IMFSample) -> Result<Frame, DecodeError> {
        let o = self.out;
        let fail = |e: windows::core::Error| {
            DecodeError::Failed(format!("Could not read the decoded frame: {e}"))
        };
        // SAFETY: the buffer is locked while we copy and unlocked after.
        unsafe {
            let buf = sample.ConvertToContiguousBuffer().map_err(fail)?;
            let min_pitch = o.format.min_pitch(o.width);
            if let Ok(b2) = buf.cast::<IMF2DBuffer>() {
                let mut scan0 = std::ptr::null_mut();
                let mut pitch = 0i32;
                b2.Lock2D(&mut scan0, &mut pitch).map_err(fail)?;
                let r = if pitch <= 0 || (pitch as usize) < min_pitch || scan0.is_null() {
                    Err(DecodeError::Failed(
                        "Bottom-up or empty decoded frame".into(),
                    ))
                } else {
                    let len = o.format.buffer_len(pitch as usize, o.height);
                    let bytes = std::slice::from_raw_parts(scan0 as *const u8, len);
                    Ok(copy_planes(bytes, pitch as usize, &o))
                };
                let _ = b2.Unlock2D();
                return r.map(|image| Frame {
                    image,
                    hint: self.hint,
                });
            }
            let mut ptr = std::ptr::null_mut();
            let mut cur = 0u32;
            buf.Lock(&mut ptr, None, Some(&mut cur)).map_err(fail)?;
            let pitch = o
                .stride
                .filter(|s| *s > 0 && *s as usize >= min_pitch)
                .map(|s| s as usize)
                .unwrap_or(min_pitch);
            let need = o.format.buffer_len(pitch, o.height);
            let r = if ptr.is_null() || (cur as usize) < need {
                Err(DecodeError::Failed("The decoded frame is short".into()))
            } else {
                let bytes = std::slice::from_raw_parts(ptr as *const u8, need);
                Ok(copy_planes(bytes, pitch, &o))
            };
            let _ = buf.Unlock();
            r.map(|image| Frame {
                image,
                hint: self.hint,
            })
        }
    }
}

/// The decoder output layouts we read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// 8-bit 4:2:0: Y plane, then interleaved CbCr at half height.
    Nv12,
    /// 10-bit 4:2:0 as NV12 with 16-bit samples (value in the top 10 bits).
    P010,
    /// 10-bit 4:2:2: Y plane, then interleaved CbCr at full height.
    P210,
    /// 10-bit 4:2:2 packed: Y0 Cb Y1 Cr per pixel pair, 16-bit samples.
    Y210,
}

impl Format {
    fn from_subtype(sub: &GUID) -> Option<Format> {
        // Y210 / P210 FourCCs in the standard video subtype GUID.
        const Y210: GUID = GUID::from_u128(0x30313259_0000_0010_8000_00aa00389b71);
        const P210: GUID = GUID::from_u128(0x30313250_0000_0010_8000_00aa00389b71);
        match *sub {
            s if s == MFVideoFormat_NV12 => Some(Format::Nv12),
            s if s == MFVideoFormat_P010 => Some(Format::P010),
            s if s == P210 => Some(Format::P210),
            s if s == Y210 => Some(Format::Y210),
            _ => None,
        }
    }

    fn bits(self) -> u8 {
        if self == Format::Nv12 { 8 } else { 10 }
    }

    fn min_pitch(self, width: u32) -> usize {
        let w = width as usize;
        match self {
            Format::Nv12 => w,
            Format::P010 | Format::P210 => 2 * w,
            Format::Y210 => 8 * w.div_ceil(2),
        }
    }

    fn buffer_len(self, pitch: usize, height: u32) -> usize {
        let h = height as usize;
        match self {
            Format::Nv12 | Format::P010 => pitch * (h + h.div_ceil(2)),
            Format::P210 => pitch * 2 * h,
            Format::Y210 => pitch * h,
        }
    }
}

/// Copies a decoded buffer into planar samples, cropped to the display area.
fn copy_planes(bytes: &[u8], pitch: usize, o: &Output) -> Decoded {
    let (cx, cy, w, h) = (
        o.crop_x as usize & !1,
        o.crop_y as usize & !1,
        o.crop_w as usize,
        o.crop_h as usize,
    );
    let f = o.format;
    let half_height = matches!(f, Format::Nv12 | Format::P010);
    let (cw, ch) = (w.div_ceil(2), if half_height { h.div_ceil(2) } else { h });
    let s16 = |row: &[u8], i: usize| u16::from_le_bytes([row[2 * i], row[2 * i + 1]]) >> 6;
    let sample = |row: &[u8], i: usize| -> u16 {
        if f == Format::Nv12 {
            row[i] as u16
        } else {
            s16(row, i)
        }
    };
    let mut y = Vec::with_capacity(w * h);
    let mut cb = Vec::with_capacity(cw * ch);
    let mut cr = Vec::with_capacity(cw * ch);
    if f == Format::Y210 {
        for r in cy..cy + h {
            let row = &bytes[r * pitch..(r + 1) * pitch];
            y.extend((cx..cx + w).map(|i| s16(row, 2 * i)));
            for p in cx / 2..cx / 2 + cw {
                cb.push(s16(row, 4 * p + 1));
                cr.push(s16(row, 4 * p + 3));
            }
        }
    } else {
        for r in cy..cy + h {
            let row = &bytes[r * pitch..(r + 1) * pitch];
            y.extend((cx..cx + w).map(|i| sample(row, i)));
        }
        let uv_base = pitch * o.height as usize;
        let first = if half_height { cy / 2 } else { cy };
        for r in first..first + ch {
            let row = &bytes[uv_base + r * pitch..uv_base + (r + 1) * pitch];
            for i in cx / 2..cx / 2 + cw {
                cb.push(sample(row, 2 * i));
                cr.push(sample(row, 2 * i + 1));
            }
        }
    }
    Decoded::Ycbcr {
        width: w as u32,
        height: h as u32,
        chroma_width: cw as u32,
        chroma_height: ch as u32,
        bits: f.bits(),
        y,
        cb,
        cr,
        matrix: o.matrix,
        full_range: o.full_range,
    }
}

/// Reads the negotiated output format.
unsafe fn output(reader: &IMFSourceReader) -> Result<Output, DecodeError> {
    let fail = |e: windows::core::Error| {
        DecodeError::Failed(format!("Media Foundation output format: {e}"))
    };
    unsafe {
        let mt = reader.GetCurrentMediaType(VIDEO).map_err(fail)?;
        let sub = mt.GetGUID(&MF_MT_SUBTYPE).map_err(fail)?;
        let Some(format) = Format::from_subtype(&sub) else {
            return Err(DecodeError::Failed(format!(
                "Media Foundation chose an unexpected output format ({sub:?})"
            )));
        };
        let size = mt.GetUINT64(&MF_MT_FRAME_SIZE).map_err(fail)?;
        let (width, height) = ((size >> 32) as u32, size as u32);
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(DecodeError::Failed(format!(
                "Media Foundation frame size {width}×{height}"
            )));
        }
        let (mut crop_x, mut crop_y, mut crop_w, mut crop_h) = (0, 0, width, height);
        let mut blob = [0u8; 16];
        let mut got = 0u32;
        if mt
            .GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut blob, Some(&mut got))
            .is_ok()
            && got == 16
        {
            let ox = i16::from_le_bytes([blob[2], blob[3]]).max(0) as u32;
            let oy = i16::from_le_bytes([blob[6], blob[7]]).max(0) as u32;
            let aw = i32::from_le_bytes([blob[8], blob[9], blob[10], blob[11]]).max(0) as u32;
            let ah = i32::from_le_bytes([blob[12], blob[13], blob[14], blob[15]]).max(0) as u32;
            if aw > 0 && ah > 0 && ox + aw <= width && oy + ah <= height {
                (crop_x, crop_y, crop_w, crop_h) = (ox, oy, aw, ah);
            }
        }
        let stride = mt.GetUINT32(&MF_MT_DEFAULT_STRIDE).ok().map(|s| s as i32);
        let matrix = match mt.GetUINT32(&MF_MT_YUV_MATRIX).ok().map(|m| m as i32) {
            Some(m) if m == MFVideoTransferMatrix_BT709.0 => YcbcrMatrix::Bt709,
            Some(m) if m == MFVideoTransferMatrix_BT601.0 => YcbcrMatrix::Bt601,
            Some(m)
                if m == MFVideoTransferMatrix_BT2020_10.0
                    || m == MFVideoTransferMatrix_BT2020_12.0 =>
            {
                YcbcrMatrix::Bt2020
            }
            _ if crop_h >= 720 => YcbcrMatrix::Bt709,
            _ => YcbcrMatrix::Bt601,
        };
        let full_range =
            mt.GetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE).ok() == Some(MFNominalRange_0_255.0 as u32);
        Ok(Output {
            format,
            width,
            height,
            crop_x,
            crop_y,
            crop_w,
            crop_h,
            stride,
            matrix,
            full_range,
        })
    }
}

fn ratio_attr(mt: &IMFMediaType, key: &GUID) -> Option<f64> {
    // SAFETY: plain COM call.
    let v = unsafe { mt.GetUINT64(key) }.ok()?;
    let (n, d) = ((v >> 32) as u32, v as u32);
    (n > 0 && d > 0).then(|| n as f64 / d as f64)
}

/// The clip's colour hint, filled from the stream's own signalling.
fn hint_for(probe: &Probe, native: &IMFMediaType) -> ColourHint {
    let mut hint = probe.meta.colour;
    // SAFETY: plain COM calls.
    unsafe {
        if hint.transfer.is_none()
            && let Ok(t) = native.GetUINT32(&MF_MT_TRANSFER_FUNCTION)
        {
            let t = t as i32;
            hint.transfer = if t == MFVideoTransFunc_709.0 || t == MFVideoTransFunc_2020.0 {
                Some(Transfer::Rec709)
            } else if t == MFVideoTransFunc_2084.0 {
                Some(Transfer::Pq)
            } else if t == MFVideoTransFunc_HLG.0 {
                Some(Transfer::Hlg)
            } else {
                None
            };
        }
        if hint.gamut.is_none()
            && let Ok(p) = native.GetUINT32(&MF_MT_VIDEO_PRIMARIES)
        {
            let p = p as i32;
            hint.gamut = if p == MFVideoPrimaries_BT709.0 {
                Some(Gamut::Rec709)
            } else if p == MFVideoPrimaries_BT2020.0 {
                Some(Gamut::Rec2020)
            } else {
                None
            };
        }
    }
    hint
}

/// "H.264 High 4:2:2" etc. from the stream's profile, for messages.
fn profile_label(codec: &Codec, native: &IMFMediaType) -> String {
    // SAFETY: plain COM calls.
    let sub = unsafe { native.GetGUID(&MF_MT_SUBTYPE) }.ok();
    let profile = unsafe { native.GetUINT32(&MF_MT_VIDEO_PROFILE) }.ok();
    let is_hevc = matches!(codec, Codec::Hevc)
        || sub == Some(MFVideoFormat_HEVC)
        || sub == Some(MFVideoFormat_HEVC_ES);
    let name = if is_hevc { "HEVC" } else { "H.264" };
    let p = match (is_hevc, profile) {
        (false, Some(66)) => Some("Baseline"),
        (false, Some(77)) => Some("Main"),
        (false, Some(100)) => Some("High"),
        (false, Some(110)) => Some("High 10"),
        (false, Some(122)) => Some("High 4:2:2"),
        (false, Some(244)) => Some("High 4:4:4"),
        (true, Some(1)) => Some("Main"),
        (true, Some(2)) => Some("Main 10"),
        (true, Some(4)) => Some("4:2:2 (Range Extensions)"),
        _ => None,
    };
    match p {
        Some(p) => format!("{name} {p}"),
        None if is_hevc => "This HEVC profile".into(),
        None => "This H.264 profile".into(),
    }
}

fn unsupported(label: &str) -> DecodeError {
    DecodeError::Unsupported(format!(
        "{label} isn't supported by this computer's decoders"
    ))
}

fn propvariant_i64(v: i64) -> PROPVARIANT {
    let mut inner = PROPVARIANT_0_0 {
        vt: VT_I8,
        ..Default::default()
    };
    inner.Anonymous.hVal = v;
    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(inner),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(format: Format, w: u32, h: u32) -> Output {
        Output {
            format,
            width: w,
            height: h,
            crop_x: 0,
            crop_y: 0,
            crop_w: w,
            crop_h: h,
            stride: None,
            matrix: YcbcrMatrix::Bt709,
            full_range: false,
        }
    }

    fn planes(d: &Decoded) -> (u32, u32, u32, u32, &[u16], &[u16], &[u16]) {
        let Decoded::Ycbcr {
            width,
            height,
            chroma_width,
            chroma_height,
            y,
            cb,
            cr,
            ..
        } = d
        else {
            panic!("not ycbcr")
        };
        (*width, *height, *chroma_width, *chroma_height, y, cb, cr)
    }

    #[test]
    fn nv12_with_padding_and_crop() {
        // 4x4 coded, pitch 6, display 4x2 at the top
        let pitch = 6;
        let mut b = vec![0u8; pitch * 6];
        for r in 0..4 {
            for c in 0..4 {
                b[r * pitch + c] = (r * 10 + c) as u8;
            }
        }
        for r in 0..2 {
            for c in 0..2 {
                b[4 * pitch + r * pitch + 2 * c] = 100 + (r * 2 + c) as u8;
                b[4 * pitch + r * pitch + 2 * c + 1] = 200 + (r * 2 + c) as u8;
            }
        }
        let mut o = out(Format::Nv12, 4, 4);
        o.crop_h = 2;
        let d = copy_planes(&b, pitch, &o);
        let (w, h, cw, ch, y, cb, cr) = planes(&d);
        assert_eq!((w, h, cw, ch), (4, 2, 2, 1));
        assert_eq!(y, &[0, 1, 2, 3, 10, 11, 12, 13]);
        assert_eq!(cb, &[100, 101]);
        assert_eq!(cr, &[200, 201]);
    }

    #[test]
    fn p010_takes_the_top_ten_bits() {
        let (w, h) = (2u32, 2u32);
        let pitch = 4;
        let mut b = vec![0u8; Format::P010.buffer_len(pitch, h)];
        let put = |b: &mut Vec<u8>, at: usize, v: u16| {
            b[at..at + 2].copy_from_slice(&(v << 6).to_le_bytes());
        };
        put(&mut b, 0, 1023);
        put(&mut b, 2, 64);
        put(&mut b, 4, 512);
        put(&mut b, 6, 940);
        put(&mut b, 8, 300);
        put(&mut b, 10, 700);
        let d = copy_planes(&b, pitch, &out(Format::P010, w, h));
        let (_, _, cw, ch, y, cb, cr) = planes(&d);
        assert_eq!((cw, ch), (1, 1));
        assert_eq!(y, &[1023, 64, 512, 940]);
        assert_eq!((cb, cr), (&[300u16][..], &[700u16][..]));
    }

    #[test]
    fn y210_is_packed_422() {
        // one row, 2 pixels: Y0 Cb Y1 Cr
        let mut b = Vec::new();
        for v in [100u16, 200, 300, 400] {
            b.extend_from_slice(&(v << 6).to_le_bytes());
        }
        let d = copy_planes(&b, 8, &out(Format::Y210, 2, 1));
        let (_, _, cw, ch, y, cb, cr) = planes(&d);
        assert_eq!((cw, ch), (1, 1));
        assert_eq!(y, &[100, 300]);
        assert_eq!((cb, cr), (&[200u16][..], &[400u16][..]));
    }

    #[test]
    fn p210_has_full_height_chroma() {
        let pitch = 4;
        let mut b = vec![0u8; Format::P210.buffer_len(pitch, 2)];
        for (i, v) in [1u16, 2, 3, 4, 50, 60, 70, 80].into_iter().enumerate() {
            b[2 * i..2 * i + 2].copy_from_slice(&(v << 6).to_le_bytes());
        }
        let d = copy_planes(&b, pitch, &out(Format::P210, 2, 2));
        let (_, _, cw, ch, y, cb, cr) = planes(&d);
        assert_eq!((cw, ch), (1, 2));
        assert_eq!(y, &[1, 2, 3, 4]);
        assert_eq!(cb, &[50, 70]);
        assert_eq!(cr, &[60, 80]);
    }

    #[test]
    fn subtypes() {
        assert_eq!(
            Format::from_subtype(&MFVideoFormat_NV12),
            Some(Format::Nv12)
        );
        assert_eq!(
            Format::from_subtype(&MFVideoFormat_P010),
            Some(Format::P010)
        );
        assert_eq!(Format::from_subtype(&MFVideoFormat_H264), None);
    }
}
