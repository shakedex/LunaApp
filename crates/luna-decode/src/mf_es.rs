//! HEVC in MXF (Sony XAVC H) through Windows Media Foundation's HEVC decoder.
//!
//! The source reader can't open MXF, so the decoder transform is driven
//! directly: each indexed frame (an Annex B access unit with its own VPS, SPS
//! and PPS) goes in as one sample, and a drain brings its picture out. Output
//! is the decoder's native layout (Y210/P210 for 10-bit 4:2:2, P010, NV12),
//! read with the same code as the MOV/MP4 path.
//!
//! Only intra-coded streams (XAVC H Intra) are decoded: a frame without an
//! IRAP picture is reported as unsupported rather than decoded from a guessed
//! reference.

use std::mem::ManuallyDrop;

use luna_formats::{ClipFiles, FrameIndex, Probe};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;

use crate::annexb::{has_irap, profile_label};
use crate::mf::{self, Com, MfStartup, Output};
use crate::util;
use crate::{DecodeError, Frame};

pub(crate) fn decode(
    clip: &ClipFiles,
    probe: &Probe,
    positions: &[u64],
    _min_width: u32,
) -> Vec<Result<Frame, DecodeError>> {
    let FrameIndex::Ranges(ranges) = &probe.frames else {
        return util::all_err(
            positions.len(),
            DecodeError::Failed("HEVC frames were not indexed".into()),
        );
    };
    let _com = match Com::init() {
        Ok(c) => c,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let _mf = match MfStartup::init() {
        Ok(m) => m,
        Err(e) => return util::all_err(positions.len(), e),
    };
    let mut label = "This HEVC profile".to_string();
    let mut decoder: Option<Decoder> = None;
    util::per_frame("HEVC", positions, ranges.len() as u64, |i| {
        let r = ranges[i as usize];
        let au = util::read_range(&clip.main, r.offset, r.len)?;
        if let Some(l) = profile_label(&au) {
            label = l;
        }
        if !has_irap(&au) {
            return Err(DecodeError::Unsupported(
                "Long-GOP HEVC in MXF isn't supported yet".into(),
            ));
        }
        if decoder.is_none() {
            decoder = Some(Decoder::open(probe, &label)?);
        }
        let d = decoder.as_mut().expect("opened above");
        let image = d.decode(&au, i)?;
        Ok(Frame {
            image,
            hint: probe.meta.colour,
        })
    })
}

struct Decoder {
    mft: IMFTransform,
    out: Output,
    label: String,
    decoded_any: bool,
}

fn fail(what: &str) -> impl Fn(windows::core::Error) -> DecodeError + '_ {
    move |e| DecodeError::Failed(format!("Media Foundation could not {what}: {e}"))
}

impl Decoder {
    fn open(probe: &Probe, label: &str) -> Result<Decoder, DecodeError> {
        let (w, h) = (
            probe.meta.width.unwrap_or(0),
            probe.meta.height.unwrap_or(0),
        );
        // SAFETY: plain COM calls; the activate array is freed after use.
        unsafe {
            let input = MFT_REGISTER_TYPE_INFO {
                guidMajorType: MFMediaType_Video,
                guidSubtype: MFVideoFormat_HEVC,
            };
            let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count = 0u32;
            MFTEnumEx(
                MFT_CATEGORY_VIDEO_DECODER,
                MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_LOCALMFT | MFT_ENUM_FLAG_SORTANDFILTER,
                Some(&input),
                None,
                &mut list,
                &mut count,
            )
            .map_err(fail("look for an HEVC decoder"))?;
            let activates: Vec<Option<IMFActivate>> = if list.is_null() {
                Vec::new()
            } else {
                let v = std::slice::from_raw_parts_mut(list, count as usize)
                    .iter_mut()
                    .map(Option::take)
                    .collect();
                CoTaskMemFree(Some(list as *const _));
                v
            };
            let Some(act) = activates.into_iter().flatten().next() else {
                return Err(mf::unsupported(label));
            };
            let mft: IMFTransform = act
                .ActivateObject()
                .map_err(fail("start the HEVC decoder"))?;
            let mt = MFCreateMediaType().map_err(fail("start"))?;
            mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(fail("start"))?;
            mt.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_HEVC)
                .map_err(fail("start"))?;
            if w > 0 && h > 0 {
                mt.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)
                    .map_err(fail("start"))?;
            }
            mt.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .map_err(fail("start"))?;
            if mft.SetInputType(0, &mt, 0).is_err() {
                return Err(mf::unsupported(label));
            }
            let out = negotiate(&mft, label)?;
            let _ = mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0);
            let _ = mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0);
            Ok(Decoder {
                mft,
                out,
                label: label.to_string(),
                decoded_any: false,
            })
        }
    }

    /// Decodes one self-contained access unit.
    fn decode(&mut self, au: &[u8], frame: u64) -> Result<crate::Decoded, DecodeError> {
        // SAFETY: plain COM calls; the input buffer is unlocked before use.
        unsafe {
            let buf = MFCreateMemoryBuffer(au.len() as u32).map_err(fail("start"))?;
            let mut ptr = std::ptr::null_mut();
            buf.Lock(&mut ptr, None, None).map_err(fail("start"))?;
            std::ptr::copy_nonoverlapping(au.as_ptr(), ptr, au.len());
            buf.Unlock().map_err(fail("start"))?;
            buf.SetCurrentLength(au.len() as u32)
                .map_err(fail("start"))?;
            let sample = MFCreateSample().map_err(fail("start"))?;
            sample.AddBuffer(&buf).map_err(fail("start"))?;
            let _ = sample.SetSampleTime(frame as i64 * 400_000);
            if let Err(e) = self.mft.ProcessInput(0, &sample, 0) {
                return Err(self.failure(frame, e));
            }
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0);
            let mut image = None;
            // Read until the drain is done; keep the first picture.
            for _ in 0..8 {
                match self.output()? {
                    Some(s) => {
                        if image.is_none() {
                            image = Some(mf::sample_image(&s, &self.out)?);
                        }
                    }
                    None => break,
                }
            }
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            match image {
                Some(i) => {
                    self.decoded_any = true;
                    Ok(i)
                }
                None if self.decoded_any => Err(DecodeError::Failed(format!(
                    "The decoder returned no picture for frame {frame}"
                ))),
                None => Err(mf::unsupported(&self.label)),
            }
        }
    }

    /// One `ProcessOutput`: a decoded sample, or `None` when the decoder needs
    /// more input. Format changes are renegotiated and retried.
    unsafe fn output(&mut self) -> Result<Option<IMFSample>, DecodeError> {
        for _ in 0..4 {
            // SAFETY: plain COM calls; the output buffer struct's COM fields are
            // taken back out (and released) after the call.
            unsafe {
                let info = self
                    .mft
                    .GetOutputStreamInfo(0)
                    .map_err(fail("read the decoder's output"))?;
                let provides = info.dwFlags
                    & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0
                        | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32
                    != 0;
                let given = if provides {
                    None
                } else {
                    let s = MFCreateSample().map_err(fail("start"))?;
                    let b = MFCreateMemoryBuffer(info.cbSize.max(1)).map_err(fail("start"))?;
                    s.AddBuffer(&b).map_err(fail("start"))?;
                    Some(s)
                };
                let mut db = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(given),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut status = 0u32;
                let r = self.mft.ProcessOutput(0, &mut db, &mut status);
                let sample = ManuallyDrop::into_inner(std::mem::replace(
                    &mut db[0].pSample,
                    ManuallyDrop::new(None),
                ));
                drop(ManuallyDrop::into_inner(std::mem::replace(
                    &mut db[0].pEvents,
                    ManuallyDrop::new(None),
                )));
                match r {
                    Ok(()) => return Ok(sample),
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(None),
                    Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                        self.out = negotiate(&self.mft, &self.label)?;
                    }
                    Err(e) => return Err(self.failure(0, e)),
                }
            }
        }
        Err(DecodeError::Failed(
            "The decoder kept changing its output format".into(),
        ))
    }

    fn failure(&self, frame: u64, e: windows::core::Error) -> DecodeError {
        if self.decoded_any {
            DecodeError::Failed(format!("Could not decode frame {frame}: {e}"))
        } else {
            mf::unsupported(&self.label)
        }
    }
}

/// Picks the decoder's first output layout we read, and reads its geometry.
fn negotiate(mft: &IMFTransform, label: &str) -> Result<Output, DecodeError> {
    // SAFETY: plain COM calls.
    unsafe {
        for i in 0..32 {
            let Ok(mt) = mft.GetOutputAvailableType(0, i) else {
                break;
            };
            let Ok(sub) = mt.GetGUID(&MF_MT_SUBTYPE) else {
                continue;
            };
            if mf::Format::from_subtype(&sub).is_some() && mft.SetOutputType(0, &mt, 0).is_ok() {
                let cur = mft
                    .GetOutputCurrentType(0)
                    .map_err(fail("read the decoder's output format"))?;
                return mf::output_of(&cur);
            }
        }
    }
    Err(mf::unsupported(label))
}
