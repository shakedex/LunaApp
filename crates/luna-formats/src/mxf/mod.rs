//! SMPTE MXF (ST 377-1) reader: partitions, header metadata, index tables,
//! and the vendor metadata Luna reports (ARRI, Sony, Canon).
//!
//! Only KLV headers and metadata are read; essence is located, never loaded.

mod arri;
mod display;
mod index;
pub(crate) mod klv;
pub(crate) mod labels;
mod metadata;
mod partition;
mod rdd18;
mod reader;
mod sidecar;
mod structure;

use std::io::{Read, Seek};

use self::klv::{Ul, is_essence_element, is_partition, read_klv};
use self::partition::{Kind, Partition, essence_start};
use self::reader::Reader;
use self::structure::Structure;
use crate::trace::Sources;
use crate::types::{
    ClipFiles, ClipMeta, Codec, ColourHint, FormatError, FrameIndex, Gamut, PictureLayout, Probe,
    Rational, SidecarKind, Timecode, Transfer,
};

/// Reads an MXF clip and its sidecars.
pub(crate) fn probe(clip: &ClipFiles) -> Result<Probe, FormatError> {
    let file = std::fs::File::open(&clip.main)?;
    let mut sidecars = Vec::new();
    let mut unreadable = Vec::new();
    for (kind, path) in &clip.sidecars {
        if matches!(kind, SidecarKind::SonyXml | SidecarKind::CanonXml) {
            match read_text(path) {
                Ok(t) => sidecars.push((*kind, t)),
                Err(e) => {
                    tracing::warn!(
                        kind = ?kind,
                        path = %path.display(),
                        error = %e,
                        "sidecar unreadable: the file can't be opened or read"
                    );
                    unreadable.push(format!(
                        "The sidecar {} could not be read: {e}",
                        path.display()
                    ));
                }
            }
        }
    }
    let mut p = probe_source(file, &sidecars)?;
    p.warnings.extend(unreadable);
    Ok(p)
}

fn read_text(path: &std::path::Path) -> Result<String, FormatError> {
    let f = std::fs::File::open(path)?;
    let mut r = Reader::new(f)?;
    let n = r.len();
    let v = r.read_vec(0, n, sidecar::MAX_XML)?;
    Ok(String::from_utf8_lossy(&v).into_owned())
}

/// Bytes of an ARRIRAW frame before the bit-packed image: ALEXA 35 frames
/// start with 0xFF padding (`frame_len − W·H·bits/8`). `None` when the frame
/// is smaller than the image.
pub fn arriraw_image_offset(frame_len: u64, width: u32, height: u32, bits: u8) -> Option<u64> {
    let image = (width as u64 * height as u64 * bits as u64).div_ceil(8);
    frame_len.checked_sub(image)
}

/// The probe of an already-open MXF source, with sidecar texts.
pub(crate) fn probe_source<R: Read + Seek>(
    src: R,
    sidecars: &[(SidecarKind, String)],
) -> Result<Probe, FormatError> {
    let mut r = Reader::new(src)?;
    let mut warnings = Vec::new();
    let parts = partition::find_partitions(&mut r)?;
    if !parts.complete {
        let kinds: Vec<String> = parts
            .list
            .iter()
            .take(5)
            .map(|p| format!("{:?}@{}", p.kind, p.offset))
            .collect();
        tracing::warn!(
            partitions = parts.list.len(),
            first = %kinds.join(", "),
            file_len = r.len(),
            "no footer: no random index pack or footer partition reached"
        );
        warnings.push("The file has no footer or partition index; it may be incomplete.".into());
    }

    // Header metadata: the most complete copy, falling back to others.
    let mut cands: Vec<&Partition> = parts
        .list
        .iter()
        .filter(|p| p.header_byte_count > 0)
        .collect();
    cands.sort_by_key(|p| std::cmp::Reverse((p.metadata_rank(), p.offset)));
    let mut md = None;
    let mut last_err = None;
    for p in &cands {
        match metadata::read_metadata(&mut r, p) {
            Ok(m) => {
                md = Some(m);
                break;
            }
            Err(e) => {
                tracing::debug!(
                    partition = p.offset,
                    header_byte_count = p.header_byte_count,
                    error = %e,
                    "header metadata copy unreadable, trying the next"
                );
                last_err = Some(e)
            }
        }
    }
    let md = md.ok_or_else(|| {
        last_err.unwrap_or_else(|| FormatError::Corrupt("no header metadata".into()))
    })?;
    if let Some(at) = md.truncated {
        tracing::warn!(
            stopped_at = at,
            sets = md.sets.len(),
            file_len = r.len(),
            "header metadata cut short: an item is damaged or runs past the file end"
        );
        warnings.push("The header metadata is cut short; some values may be missing.".into());
    }
    let st = structure::resolve(&md);

    let segments = match index::read_segments(&mut r, &parts.list) {
        Ok(s) => s,
        Err(e) => {
            let indexed: Vec<String> = parts
                .list
                .iter()
                .filter(|p| p.index_byte_count > 0)
                .take(5)
                .map(|p| format!("{}+{}", p.offset, p.index_byte_count))
                .collect();
            tracing::warn!(
                error = %e,
                partitions_with_index = %indexed.join(", "),
                "index unreadable: an index segment can't be read"
            );
            warnings.push(format!("The index table could not be read: {e}"));
            Vec::new()
        }
    };
    let target = index::Target {
        track_number: st.picture_track_number,
        body_sid: st.body_sid,
        duration: st.duration.filter(|&d| d > 0).or_else(|| {
            st.picture
                .as_ref()
                .and_then(|p| p.container_duration)
                .filter(|&d| d > 0)
                .map(|d| d as u64)
        }),
    };
    let frames = index::frames(&mut r, &parts.list, &segments, &target);
    warnings.extend(frames.warnings.iter().cloned());

    let pic = st.picture.clone().unwrap_or_default();
    let codec = labels::codec(
        pic.coding.as_ref(),
        pic.container.as_ref(),
        frames.first_key.as_ref(),
    );

    let first_unit = first_unit_data(&mut r, &parts.list, &st, &frames);

    let mut meta = generic_meta(&st);
    let mut sources = generic_sources(&st);
    if let Some(n) = target.duration {
        meta.frame_count = Some(n);
        let from = if st.duration.is_some_and(|d| d > 0) {
            "picture track Duration"
        } else {
            "picture descriptor ContainerDuration"
        };
        sources.set("frame_count", from);
    } else if !frames.truncated && !frames.clip_wrapped && !frames.ranges.is_empty() {
        meta.frame_count = Some(frames.ranges.len() as u64);
        sources.set("frame_count", "picture elements counted in the essence");
    }
    if frames.truncated {
        let have = frames.ranges.len();
        tracing::warn!(
            present = have,
            expected = ?meta.frame_count,
            duration = ?target.duration,
            last_frame_end = ?frames.ranges.last().map(|f| f.offset + f.len),
            file_len = r.len(),
            "ends early: the last indexed frame runs past the file end"
        );
        match meta.frame_count {
            Some(n) => warnings.push(format!(
                "The file ends early: {have} of {n} frames are present."
            )),
            None => {
                warnings.push(format!("The file ends early after {have} frames."));
                // A recording cut off (power loss, card pulled) never got its
                // durations written: the complete frames are the clip.
                if !frames.clip_wrapped && have > 0 {
                    meta.frame_count = Some(have as u64);
                    sources.set("frame_count", "complete picture elements before the cut");
                }
            }
        }
    }

    let vendor = vendor_of(&st);
    tracing::debug!(
        company = ?st.ident.company,
        product = ?st.ident.product,
        make = ?vendor,
        "mxf identification"
    );
    let mut v = Vendor::default();
    let mut blocks: Vec<(String, bool)> = Vec::new();
    match vendor {
        Make::Arri => arri_meta(&md, &first_unit, &mut v, &mut sources, &mut blocks),
        Make::Sony | Make::Canon | Make::Other => {}
    }
    let acq = first_unit
        .iter()
        .filter(|(k, _)| k.0[12] == 0x17)
        .map(|(_, val)| rdd18::parse(&rdd18::anc_payload(val)))
        .find(|a| *a != rdd18::Acquisition::default());
    blocks.push((RDD18.into(), acq.is_some()));
    if let Some(acq) = acq {
        let before = v.values();
        rdd18_meta(&acq, &mut v);
        v.credit(&before, RDD18, &mut sources);
    }
    if sidecars.is_empty() {
        blocks.push(("XML sidecar".into(), false));
    }
    for (kind, text) in sidecars {
        let before = v.values();
        match kind {
            SidecarKind::SonyXml => match sidecar::parse_sony(text) {
                Some(x) => {
                    sony_xml_meta(&x, &mut v);
                    v.credit(&before, SONY_XML, &mut sources);
                    blocks.push((SONY_XML.into(), true));
                }
                None => {
                    blocks.push((format!("{SONY_XML} (unparsed)"), false));
                    sidecar_unparsed("Sony", text);
                    warnings.push("The Sony XML sidecar could not be read.".into())
                }
            },
            SidecarKind::CanonXml => match sidecar::parse_canon(text) {
                Some(x) => {
                    canon_xml_meta(&x, &mut v);
                    v.credit(&before, CANON_XML, &mut sources);
                    blocks.push((CANON_XML.into(), true));
                }
                None => {
                    blocks.push((format!("{CANON_XML} (unparsed)"), false));
                    sidecar_unparsed("Canon", text);
                    warnings.push("The Canon XML sidecar could not be read.".into())
                }
            },
            _ => {}
        }
    }
    merge(&mut meta, v, vendor, &st, &mut sources);
    let base = |f: &str| match f {
        "frame_rate" => "picture track EditRate",
        "frame_count" => "picture track Duration, descriptor ContainerDuration, essence",
        "width" | "height" => "picture descriptor",
        "audio" => "sound descriptors",
        "start_timecode" => "timecode track",
        "camera_make" => "Identification CompanyName",
        "camera_model" if vendor == Make::Sony => {
            "vendor blocks (Sony's Identification names the recorder)"
        }
        "camera_model" => "Identification ProductName",
        "recording_date" => "material package CreationDate, Identification ModificationDate",
        "transfer" | "gamut" => "picture descriptor",
        _ => "",
    };
    sources.log(&meta, base, &blocks);

    // Picture layout.
    let mut layout = PictureLayout {
        stored_width: pic.stored_width.unwrap_or(0),
        stored_height: pic.stored_height.unwrap_or(0),
        raw_bits: None,
        pixel_aspect: pixel_aspect(&pic),
        rotation: 0,
    };
    if codec == Codec::ArriRaw {
        layout.raw_bits = frames.ranges.first().and_then(|f| {
            arriraw_bits(
                f.len,
                layout.stored_width,
                layout.stored_height,
                pic.image_start_offset,
            )
        });
        if layout.raw_bits.is_none() {
            let px = layout.stored_width as u64 * layout.stored_height as u64;
            tracing::warn!(
                frame_len = ?frames.ranges.first().map(|f| f.len),
                width = layout.stored_width,
                height = layout.stored_height,
                image_start_offset = ?pic.image_start_offset,
                bytes_12bit = (px * 12).div_ceil(8),
                bytes_13bit = (px * 13).div_ceil(8),
                "arriraw bits: frame size matches neither 12- nor 13-bit packing"
            );
            warnings
                .push("The ARRIRAW bit depth could not be worked out from the frame size.".into());
        }
    }

    let frames_index = if frames.clip_wrapped {
        tracing::warn!(
            element_len = ?frames.ranges.first().map(|f| f.len),
            duration = ?target.duration,
            "clip-wrapped: one picture element holds every frame"
        );
        warnings
            .push("The picture is clip-wrapped; frame positions are left to the decoder.".into());
        FrameIndex::DecoderOwned
    } else if frames.ranges.is_empty() {
        tracing::warn!(
            track_number = ?target.track_number,
            body_sid = ?target.body_sid,
            partitions = parts.list.len(),
            segments = segments.len(),
            "no picture frames: no essence element matched the picture track"
        );
        warnings.push("No picture frames were found.".into());
        FrameIndex::DecoderOwned
    } else {
        FrameIndex::Ranges(frames.ranges)
    };

    Ok(Probe {
        codec,
        meta,
        frames: frames_index,
        layout,
        codec_config: None,
        warnings,
    })
}

/// Metadata blocks, as the run log names them.
const ARRI_JSON: &str = "ARRI JSON documents";
const ARRI_HEADER: &str = "ARRI legacy camera header";
const ARRI_FRAME: &str = "ARRI frame metadata element (first frame)";
const RDD18: &str = "RDD 18 ANC (first frame)";
const SONY_XML: &str = "Sony XML sidecar";
const CANON_XML: &str = "Canon XML sidecar";

/// Logs how a sidecar that didn't parse begins.
fn sidecar_unparsed(vendor: &str, text: &str) {
    let head: String = text.trim_start().chars().take(80).collect();
    tracing::warn!(
        vendor,
        len = text.len(),
        head = ?head,
        "sidecar unparsed: not XML or not the expected root element"
    );
}

/// Where `generic_meta` took each field from.
fn generic_sources(st: &Structure) -> Sources {
    let mut s = Sources::default();
    s.set("frame_rate", "picture track EditRate");
    s.set("start_timecode", "timecode track StartTimecode");
    s.set("audio", "sound descriptors");
    if let Some(p) = &st.picture {
        let w = if p.display_width.is_some() {
            "DisplayWidth"
        } else {
            "StoredWidth"
        };
        let h = if p.display_height.is_some() {
            "DisplayHeight"
        } else {
            "StoredHeight"
        };
        s.set("width", format!("picture descriptor {w}"));
        s.set("height", format!("picture descriptor {h}"));
        s.set("transfer", "picture descriptor TransferCharacteristic");
        s.set("gamut", "picture descriptor ColorPrimaries");
    }
    s
}

fn arriraw_bits(len: u64, w: u32, h: u32, start: Option<u32>) -> Option<u8> {
    if w == 0 || h == 0 {
        return None;
    }
    let px = w as u64 * h as u64;
    if let Some(s) = start {
        let body = len.checked_sub(s as u64)?;
        for bits in [12u8, 13] {
            if body == (px * bits as u64).div_ceil(8) {
                return Some(bits);
            }
        }
    }
    [13u8, 12]
        .into_iter()
        .find(|&b| arriraw_image_offset(len, w, h, b).is_some_and(|pad| pad < 65536))
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn pixel_aspect(p: &structure::PictureDescriptor) -> Option<Rational> {
    let (n, d) = p.aspect?;
    let w = p.display_width.or(p.stored_width)? as u64;
    let h = p.display_height.or(p.stored_height)? as u64;
    if n <= 0 || d <= 0 || w == 0 || h == 0 {
        return None;
    }
    let (num, den) = (n as u64 * h, d as u64 * w);
    let par = num as f64 / den as f64;
    if (par - 1.0).abs() < 0.01 {
        return Some(Rational { num: 1, den: 1 });
    }
    let g = gcd(num, den);
    let (mut a, mut b) = (num / g, den / g);
    while a > u32::MAX as u64 || b > u32::MAX as u64 {
        a /= 2;
        b /= 2;
    }
    (b != 0).then_some(Rational {
        num: a as u32,
        den: b as u32,
    })
}

/// Data elements (ANC, ARRI frame metadata) of the first content package.
fn first_unit_data<R: Read + Seek>(
    r: &mut Reader<R>,
    parts: &[Partition],
    st: &Structure,
    frames: &index::Frames,
) -> Vec<(Ul, Vec<u8>)> {
    let mut out = Vec::new();
    let Some(first) = frames.ranges.first() else {
        return out;
    };
    let sid = st.body_sid.unwrap_or(0);
    let Some(p) = parts
        .iter()
        .filter(|p| p.kind != Kind::Footer)
        .rfind(|p| p.offset < first.offset && (p.body_sid == sid || sid == 0) && p.body_sid != 0)
        .or_else(|| parts.first())
    else {
        return out;
    };
    let mut off = essence_start(r, p);
    let mut pictures = 0;
    for _ in 0..256 {
        let Ok(k) = read_klv(r, off) else { break };
        if k.end() > r.len() || (is_partition(&k.key) && off != p.offset) {
            break;
        }
        if index::is_picture(&k.key, st.picture_track_number) {
            pictures += 1;
            if pictures > 1 {
                break;
            }
        } else if is_essence_element(&k.key)
            && k.key.0[12] == 0x17
            && let Ok(v) = r.read_vec(k.value, k.len, 1 << 20)
        {
            out.push((k.key, v));
        }
        off = k.end();
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Make {
    Arri,
    Sony,
    Canon,
    Other,
}

fn vendor_of(st: &Structure) -> Make {
    match st
        .ident
        .company
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("arri") => Make::Arri,
        Some(s) if s.starts_with("sony") => Make::Sony,
        Some("canon") => Make::Canon,
        _ => Make::Other,
    }
}

fn generic_meta(st: &Structure) -> ClipMeta {
    let mut m = ClipMeta {
        frame_rate: st.edit_rate.map(|(n, d)| Rational {
            num: n as u32,
            den: d as u32,
        }),
        frame_count: st.duration.filter(|&d| d > 0),
        start_timecode: st.timecode.map(|(f, b, df)| Timecode {
            frames: f as u64,
            base: b as u32,
            drop_frame: df,
        }),
        audio: st.audio,
        ..Default::default()
    };
    if let Some(p) = &st.picture {
        m.width = p.display_width.or(p.stored_width).filter(|&w| w > 0);
        m.height = p.display_height.or(p.stored_height).filter(|&h| h > 0);
        m.colour = ColourHint {
            transfer: p.transfer.as_ref().and_then(labels::transfer),
            gamut: p.primaries.as_ref().and_then(labels::gamut),
        };
    }
    m
}

/// Vendor values, gathered before merging into ClipMeta.
#[derive(Debug, Default)]
struct Vendor {
    make: Option<String>,
    model: Option<String>,
    serial: Option<String>,
    lens: Option<String>,
    focal: Option<String>,
    stop: Option<String>,
    iso: Option<String>,
    shutter: Option<String>,
    white_balance: Option<String>,
    nd: Option<String>,
    sensor_fps: Option<Rational>,
    reel: Option<String>,
    camera_id: Option<String>,
    start_tc: Option<Timecode>,
    /// A sidecar's "HH:MM:SS:FF", settled against the frame rate when merging.
    start_tc_text: Option<String>,
    /// Local date-time "YYYY-MM-DD HH:MM:SS".
    date: Option<String>,
    /// A date known only in UTC.
    utc_date: Option<String>,
    transfer: Option<Transfer>,
    gamut: Option<Gamut>,
}

impl Vendor {
    /// Each value under the `ClipMeta` field it feeds (`start_tc_text` and
    /// `utc_date` under their own names), for crediting sources.
    fn values(&self) -> [(&'static str, String); 19] {
        fn d(v: &dyn std::fmt::Debug) -> String {
            format!("{v:?}")
        }
        [
            ("camera_make", d(&self.make)),
            ("camera_model", d(&self.model)),
            ("camera_serial", d(&self.serial)),
            ("lens", d(&self.lens)),
            ("focal_length", d(&self.focal)),
            ("stop", d(&self.stop)),
            ("iso", d(&self.iso)),
            ("shutter", d(&self.shutter)),
            ("white_balance", d(&self.white_balance)),
            ("nd", d(&self.nd)),
            ("sensor_fps", d(&self.sensor_fps)),
            ("embedded_reel", d(&self.reel)),
            ("embedded_camera_id", d(&self.camera_id)),
            ("start_timecode", d(&self.start_tc)),
            ("start_tc_text", d(&self.start_tc_text)),
            ("recording_date", d(&self.date)),
            ("utc_date", d(&self.utc_date)),
            ("transfer", d(&self.transfer)),
            ("gamut", d(&self.gamut)),
        ]
    }

    /// Credits `source` with every value that changed since `before`.
    fn credit(&self, before: &[(&'static str, String); 19], source: &str, sources: &mut Sources) {
        for ((field, old), (_, new)) in before.iter().zip(self.values()) {
            if *old != new {
                sources.set(field, source);
            }
        }
    }
}

fn set<T>(slot: &mut Option<T>, v: Option<T>) {
    if v.is_some() {
        *slot = v;
    }
}

fn arri_meta(
    md: &metadata::Metadata,
    unit: &[(Ul, Vec<u8>)],
    v: &mut Vendor,
    sources: &mut Sources,
    blocks: &mut Vec<(String, bool)>,
) {
    let docs = arri::json_documents(md);
    let mut names: Vec<&str> = docs.keys().map(String::as_str).collect();
    names.sort_unstable();
    blocks.push((
        format!("{ARRI_JSON} ({})", names.join(", ")),
        !names.is_empty(),
    ));
    let before = v.values();
    let j = |d: &str, f: &str| arri::json_str(&docs, d, f);
    set(
        &mut v.model,
        j("Camera Device", "cameraModel")
            .map(|m| m.strip_prefix("ARRI ").unwrap_or(&m).to_string()),
    );
    set(&mut v.serial, j("Camera Device", "cameraSerialNumber"));
    set(&mut v.lens, j("Lens Device", "lensModel"));
    set(&mut v.reel, j("Slate Info", "reelName"));
    set(&mut v.camera_id, j("Slate Info", "cameraIndex"));
    v.credit(&before, ARRI_JSON, sources);

    let legacy = arri::legacy_header(md);
    blocks.push((ARRI_HEADER.into(), legacy.is_some()));
    if let Some(l) = legacy {
        let before = v.values();
        set(
            &mut v.model,
            l.camera_model
                .map(|m| m.strip_prefix("ARRI ").unwrap_or(&m).to_string()),
        );
        set(&mut v.serial, l.camera_serial.map(|s| s.to_string()));
        set(&mut v.lens, l.lens_model);
        set(&mut v.reel, l.reel_name);
        set(&mut v.camera_id, l.camera_index);
        set(
            &mut v.nd,
            l.nd_1000
                .and_then(|n| display::nd_density(n as f64 / 1000.0)),
        );
        set(
            &mut v.sensor_fps,
            l.sensor_fps_1000.and_then(|f| Rational::reduced(f, 1000)),
        );
        set(&mut v.iso, l.exposure_index.map(|e| e.to_string()));
        set(
            &mut v.white_balance,
            l.white_balance.and_then(display::kelvin),
        );
        set(
            &mut v.shutter,
            l.shutter_angle_1000
                .and_then(|a| display::angle(a as f64 / 1000.0)),
        );
        set(
            &mut v.focal,
            l.focal_um.and_then(|u| display::focal(u as f64 / 1000.0)),
        );
        v.credit(&before, ARRI_HEADER, sources);
    }

    // ARRI frame metadata element: …0d01030117010f00
    let element = unit.iter().find(|(k, _)| k.0[12..15] == [0x17, 0x01, 0x0f]);
    blocks.push((ARRI_FRAME.into(), element.is_some()));
    if let Some((_, val)) = element {
        let before = v.values();
        let f = arri::frame_meta(val);
        set(&mut v.iso, f.exposure_index.map(|e| e.to_string()));
        set(
            &mut v.white_balance,
            f.white_balance.and_then(|k| display::kelvin(k as u32)),
        );
        set(&mut v.nd, f.nd.and_then(|n| display::nd_density(n as f64)));
        set(
            &mut v.focal,
            f.focal_um.and_then(|u| display::focal(u as f64 / 1000.0)),
        );
        set(
            &mut v.stop,
            f.iris_t1000
                .and_then(|t| display::t_stop(t as f64 / 1000.0)),
        );
        set(
            &mut v.sensor_fps,
            f.sensor_fps.and_then(|(n, d)| Rational::reduced(n, d)),
        );
        if let Some((n, d)) = f.exposure_time {
            let fps = f.sensor_fps.map(|(a, b)| a as f64 / b as f64);
            set(&mut v.shutter, display::shutter(n as f64 / d as f64, fps));
        }
        v.credit(&before, ARRI_FRAME, sources);
    }
}

fn sony_model_name(code: &str) -> String {
    match code {
        "MPC-3610" => "VENICE".into(),
        "MPC-3626" | "MPC-3628" => "VENICE 2".into(),
        "MPC-2610" => "BURANO".into(),
        other => other.to_string(),
    }
}

fn rdd18_meta(a: &rdd18::Acquisition, v: &mut Vendor) {
    // CameraAttributes: "<model> <serial> Version<x>".
    if let Some(attrs) = &a.camera_attributes {
        let mut it = attrs.split_whitespace();
        set(&mut v.model, it.next().map(sony_model_name));
        set(
            &mut v.serial,
            it.next()
                .filter(|s| !s.starts_with("Version"))
                .map(str::to_string),
        );
    }
    set(
        &mut v.model,
        a.vendor_model
            .as_deref()
            .map(|m| m.strip_prefix("Canon ").unwrap_or(m).to_string()),
    );
    set(&mut v.serial, a.vendor_serial.clone());
    set(
        &mut v.lens,
        a.lens_attributes
            .clone()
            .filter(|l| !l.eq_ignore_ascii_case("unknown")),
    );
    set(&mut v.focal, a.focal_mm.and_then(display::focal));
    set(
        &mut v.stop,
        a.iris_t
            .and_then(display::t_stop)
            .or_else(|| a.iris_f.and_then(display::f_stop)),
    );
    set(
        &mut v.iso,
        a.exposure_index.or(a.iso).map(|e| e.to_string()),
    );
    set(
        &mut v.white_balance,
        a.white_balance.and_then(|k| display::kelvin(k as u32)),
    );
    set(
        &mut v.nd,
        a.nd_setting.and_then(display::nd_from_transmittance),
    );
    let shutter = a.shutter_angle.and_then(display::angle).or_else(|| {
        let (n, d) = a.shutter_time?;
        let fps = a.capture_fps.map(|(x, y)| x as f64 / y as f64);
        display::shutter(n as f64 / d as f64, fps)
    });
    set(&mut v.shutter, shutter);
    set(
        &mut v.sensor_fps,
        a.capture_fps.and_then(|(n, d)| Rational::reduced(n, d)),
    );
    set(
        &mut v.transfer,
        a.capture_gamma.as_ref().and_then(labels::transfer),
    );
    if v.transfer.is_none() {
        v.transfer = a.vendor_transfer.as_ref().and_then(labels::transfer);
    }
    if let Some(g) = a.look_primaries.as_ref().and_then(labels::gamut) {
        v.gamut = Some(g);
    }
    if v.gamut.is_none() {
        v.gamut = a.vendor_primaries.as_ref().and_then(labels::gamut);
    }
    if let Some(d) = a.vendor_date {
        v.date = Some(display::stamp(d));
    }
}

fn sony_transfer(s: &str) -> Option<Transfer> {
    let s = s.to_ascii_lowercase();
    if s.starts_with("s-log3") {
        Some(Transfer::SLog3)
    } else if s == "scene-linear" || s == "linear" {
        Some(Transfer::Linear)
    } else if s.starts_with("rec709") || s == "itu709" {
        Some(Transfer::Rec709)
    } else if s.contains("hlg") {
        Some(Transfer::Hlg)
    } else {
        None
    }
}

fn sony_gamut(s: &str) -> Option<Gamut> {
    match s.to_ascii_lowercase().as_str() {
        "s-gamut3-cine" | "s-gamut3.cine" => Some(Gamut::SGamut3Cine),
        "s-gamut3" => Some(Gamut::SGamut3),
        "rec709" | "itu709" => Some(Gamut::Rec709),
        "rec2020" | "itu2020" => Some(Gamut::Rec2020),
        _ => None,
    }
}

fn sony_xml_meta(x: &sidecar::SonyXml, v: &mut Vendor) {
    set(&mut v.make, x.manufacturer.clone());
    set(&mut v.model, x.model.as_deref().map(sony_model_name));
    set(&mut v.serial, x.serial.clone());
    if let Some(attrs) = &x.camera_attributes {
        let mut it = attrs.split_whitespace();
        if x.model.is_none() {
            set(&mut v.model, it.next().map(sony_model_name));
        } else {
            it.next();
        }
        if x.serial.is_none() {
            set(
                &mut v.serial,
                it.next()
                    .filter(|s| !s.starts_with("Version"))
                    .map(str::to_string),
            );
        }
    }
    set(
        &mut v.lens,
        x.lens
            .clone()
            .filter(|l| !l.eq_ignore_ascii_case("unknown")),
    );
    set(&mut v.focal, x.focal_mm.and_then(display::focal));
    set(
        &mut v.stop,
        x.iris_t
            .and_then(display::t_stop)
            .or_else(|| x.iris_f.and_then(display::f_stop)),
    );
    set(
        &mut v.iso,
        x.exposure_index.clone().or_else(|| x.iso.clone()),
    );
    set(
        &mut v.white_balance,
        x.white_balance
            .as_deref()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .and_then(display::kelvin),
    );
    set(
        &mut v.nd,
        x.nd.as_deref()
            .and_then(|s| s.trim().parse::<u16>().ok())
            .and_then(display::nd_from_transmittance),
    );
    set(&mut v.shutter, x.shutter_angle.and_then(display::angle));
    if v.start_tc.is_none() {
        v.start_tc = x.start_tc;
    }
    set(
        &mut v.transfer,
        x.capture_gamma.as_deref().and_then(sony_transfer),
    );
    set(
        &mut v.gamut,
        x.capture_primaries
            .as_deref()
            .or(x.look_primaries.as_deref())
            .and_then(sony_gamut),
    );
    if let Some((s, local)) = x.creation.as_deref().and_then(sidecar::iso_local) {
        if local {
            v.date = Some(s);
        } else {
            v.utc_date = Some(s);
        }
    }
}

fn canon_xml_meta(x: &sidecar::CanonXml, v: &mut Vendor) {
    set(&mut v.model, x.model.clone());
    set(&mut v.serial, x.serial.clone());
    set(&mut v.lens, x.lens.clone());
    if let Some((s, local)) = x.date_time.as_deref().and_then(sidecar::iso_local) {
        if local {
            v.date = Some(s);
        } else {
            v.utc_date = Some(s);
        }
    }
    set(&mut v.start_tc_text, x.start_tc.clone());
}

/// "HH:MM:SS:FF" (";" before FF for drop frame) at the clip's counting base.
fn parse_tc_text(s: &str, base: u32) -> Option<Timecode> {
    let drop = s.contains(';');
    let p: Vec<u64> = s
        .split([':', ';'])
        .map(|x| x.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [hh, mm, ss, ff] = p[..] else { return None };
    (base > 0 && ff < base as u64 && ss < 60 && mm < 60 && hh < 24)
        .then(|| sidecar::timecode_from_hmsf(hh, mm, ss, ff, base, drop))
}

fn normalise_make(s: &str) -> String {
    match s.to_ascii_lowercase().as_str() {
        "arri" => "ARRI".into(),
        "sony" => "Sony".into(),
        "canon" => "Canon".into(),
        _ => s.to_string(),
    }
}

fn merge(m: &mut ClipMeta, v: Vendor, make: Make, st: &Structure, sources: &mut Sources) {
    let ident = &st.ident;
    if v.make.is_none() {
        sources.set("camera_make", "Identification CompanyName");
    }
    if v.model.is_none() {
        sources.set("camera_model", "Identification ProductName");
    }
    m.camera_make = v
        .make
        .as_deref()
        .or(ident.company.as_deref())
        .map(normalise_make);
    // Sony's Identification names the recorder or media ("AXS", "Mem"), not the camera.
    let ident_model = match make {
        Make::Sony => None,
        _ => ident.product.clone(),
    };
    m.camera_model = v.model.or(ident_model);
    m.camera_serial = v.serial;
    m.lens = v.lens;
    m.focal_length = v.focal;
    m.stop = v.stop;
    m.iso = v.iso;
    m.shutter = v.shutter;
    m.white_balance = v.white_balance;
    m.nd = v.nd;
    m.sensor_fps = v.sensor_fps;
    m.embedded_reel = v.reel;
    m.embedded_camera_id = v.camera_id;
    if m.start_timecode.is_some() {
        sources.set("start_timecode", "timecode track StartTimecode");
    } else {
        m.start_timecode = v.start_tc;
    }
    if m.start_timecode.is_none() {
        if let Some(s) = sources.get("start_tc_text").map(str::to_string) {
            sources.set("start_timecode", s);
        }
        let base = m.frame_rate.map(|r| r.as_f64().round() as u32).unwrap_or(0);
        m.start_timecode = v
            .start_tc_text
            .as_deref()
            .and_then(|t| parse_tc_text(t, base));
    }
    if m.colour.transfer.is_some() {
        sources.set("transfer", "picture descriptor TransferCharacteristic");
    } else {
        m.colour.transfer = v.transfer;
    }
    if m.colour.gamut.is_some() {
        sources.set("gamut", "picture descriptor ColorPrimaries");
    } else {
        m.colour.gamut = v.gamut;
    }
    if make == Make::Arri {
        let a35 = m
            .camera_model
            .as_deref()
            .is_some_and(|s| s.contains("ALEXA 35"));
        if m.colour.transfer.is_none() {
            sources.set("transfer", "ARRI default for the camera model");
            m.colour.transfer = Some(if a35 {
                Transfer::LogC4
            } else {
                Transfer::LogC3
            });
        }
        if m.colour.gamut.is_none() {
            sources.set("gamut", "ARRI default for the camera model");
            m.colour.gamut = Some(if a35 {
                Gamut::ArriWideGamut4
            } else {
                Gamut::ArriWideGamut3
            });
        }
    }

    let file_stamp = st.created.or(ident.modified).map(display::stamp);
    let stamp_source = if st.created.is_some() {
        "material package CreationDate"
    } else {
        "Identification ModificationDate"
    };
    match (&v.date, make, &v.utc_date) {
        (Some(_), _, _) => {}
        (None, Make::Sony, Some(_)) => {
            let s = sources.get("utc_date").unwrap_or("vendor").to_string();
            sources.set("recording_date", s);
        }
        _ => sources.set("recording_date", stamp_source),
    }
    m.recording_date = match (v.date, make) {
        (Some(d), _) => Some(d),
        // Sony's MXF writers stamp UTC (it matches the XML's "Z" times): show
        // it as UTC rather than pretend it is local time.
        (None, Make::Sony) => v.utc_date.or(file_stamp).map(|s| format!("{s} UTC")),
        (None, _) => file_stamp,
    };
}

#[cfg(test)]
mod tests;
