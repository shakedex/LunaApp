//! QuickTime `.mov` and ISO-BMFF `.mp4`: clean-room from ISO/IEC 14496-12 and
//! Apple's QuickTime File Format specification, plus vendor metadata learned
//! from camera files.
//!
//! The file is read by seeking: top-level atom headers, the whole `moov`
//! (bounded), and a handful of small samples (the first timecode sample and
//! the first vendor-metadata sample). Coded frames in `mdat` are never read.

mod apple;
mod arri;
mod atom;
mod bytes;
mod canon;
mod display;
mod dji;
mod entry;
mod exif;
mod meta;
mod panasonic;
mod track;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use crate::trace::{Sources, present};
use crate::types::{
    Audio, ByteRange, ClipFiles, ClipMeta, FormatError, FrameIndex, PictureLayout, Probe, Timecode,
};
use atom::{Atoms, child};
use meta::Items;
use track::Track;

/// Random-access bytes: the file on disk, or a buffer in tests.
pub(crate) trait Source {
    fn len(&self) -> u64;
    /// Fills `buf` from `offset`, or fails if the source is shorter.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()>;
}

impl Source for &[u8] {
    fn len(&self) -> u64 {
        <[u8]>::len(self) as u64
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        let start = usize::try_from(offset).ok();
        let src = start
            .and_then(|s| s.checked_add(buf.len()).map(|e| (s, e)))
            .and_then(|(s, e)| self.get(s..e))
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::UnexpectedEof))?;
        buf.copy_from_slice(src);
        Ok(())
    }
}

struct FileSource {
    file: RefCell<File>,
    len: u64,
}

impl Source for FileSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        let mut f = self.file.borrow_mut();
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(buf)
    }
}

/// Vendor metadata blocks, as the run log names them.
const ARRI: &str = "moov/meta com.arri.camera.* keys";
const PANA: &str = "Panasonic metadata.xml key / udta PANA Exif";
const CANON_UUID: &str = "Canon moov/uuid Exif";
const CNDM: &str = "Canon cndm track (RDD 18)";
const DJMD: &str = "DJI djmd track";

/// A `moov` larger than this is not something a camera wrote.
const MAX_MOOV: u64 = 256 << 20;
/// Largest vendor-metadata sample read from `mdat`.
const MAX_META_SAMPLE: u64 = 4 << 20;

pub(crate) fn probe(clip: &ClipFiles) -> Result<Probe, FormatError> {
    let file = File::open(&clip.main)?;
    let len = file.metadata()?.len();
    let src = FileSource {
        file: RefCell::new(file),
        len,
    };
    probe_source(&src)
}

pub(crate) fn probe_source(src: &dyn Source) -> Result<Probe, FormatError> {
    let top = atom::scan_top_level(src)?;
    let crx = *b"crx ";
    if top.major_brand == Some(crx) || top.compatible.contains(&crx) {
        return Err(FormatError::Unsupported(
            "Canon Cinema RAW Light is read by its own decoder".into(),
        ));
    }
    let mut warnings = top.warnings;
    let Some((moov_at, moov_len)) = top.moov else {
        return Err(FormatError::Corrupt(
            "no movie header (moov); the recording may not have been closed".into(),
        ));
    };
    if moov_len > MAX_MOOV {
        return Err(FormatError::Unsupported(format!(
            "The movie header is {moov_len} bytes, larger than any camera writes"
        )));
    }
    let mut moov = vec![0u8; moov_len as usize];
    src.read_at(moov_at, &mut moov)?;
    let movie = Movie::parse(&moov, &mut warnings);
    let media = Media {
        file_len: src.len(),
        moov: (moov_at, moov_len),
    };

    let video = movie
        .tracks
        .iter()
        .find(|t| &t.handler == b"vide" && !t.entry.is_empty())
        .ok_or_else(|| FormatError::Unsupported("The file has no video track".into()))?;
    for table in &video.damaged {
        tracing::warn!(
            track = video.id,
            table,
            "damaged table: present in the video track but unreadable"
        );
        warnings.push(format!("The video track's '{table}' table is damaged."));
    }

    let codec = entry::codec(&video.entry_format);
    let mut ve = entry::video_entry(&video.entry).unwrap_or_default();
    let codec_config = ve.config.take();
    if ve.fields == Some(2) {
        tracing::warn!(
            fields = 2,
            "interlaced: the sample entry's fiel atom says two fields"
        );
        warnings.push("The picture is interlaced (two fields per frame).".into());
    }
    let (width, height) = ve.clean.unwrap_or((ve.width, ve.height));
    let layout = PictureLayout {
        stored_width: ve.width,
        stored_height: ve.height,
        raw_bits: None,
        pixel_aspect: ve.pixel_aspect,
        rotation: video.rotation,
    };
    if video.rotation != 0 {
        tracing::debug!(
            track = video.id,
            degrees = video.rotation,
            "display turn from tkhd matrix"
        );
    }

    let frame_count = video.sizes.count() as u64;
    let frame_rate = frame_rate(video, &mut warnings);
    edit_warnings(video, &mut warnings);
    let frames = frame_index(video, media, &mut warnings);

    let mut meta = ClipMeta {
        frame_rate,
        frame_count: (frame_count > 0).then_some(frame_count),
        width: (width > 0).then_some(width),
        height: (height > 0).then_some(height),
        ..ClipMeta::default()
    };
    let mut sources = Sources::default();
    sources.set(
        "frame_rate",
        format!(
            "video track {} stts, mdhd timescale {}",
            video.id, video.timescale
        ),
    );
    sources.set("frame_count", format!("video track {} stsz", video.id));
    let dims = if ve.clean.is_some() {
        "video sample entry clap"
    } else {
        "video sample entry width/height"
    };
    sources.set("width", dims);
    sources.set("height", dims);

    meta.audio = audio(&movie);
    sources.set("audio", "sound track sample entries");

    let tc = timecode(&movie, video, src, media, &mut warnings);
    meta.start_timecode = tc.as_ref().and_then(|t| t.0);
    sources.set("start_timecode", "tmcd track first sample");

    // Vendor metadata first: it is the most specific. Generic QuickTime
    // metadata and the movie header only fill what vendors leave empty.
    let has_track = |f: &[u8; 4]| movie.tracks.iter().any(|t| &t.entry_format == f);
    let blocks = [
        (ARRI.to_string(), movie.items.has_prefix("com.arri.camera.")),
        (
            PANA.to_string(),
            movie.items.text(panasonic::XML_KEY).is_some() || movie.pana.is_some(),
        ),
        (CANON_UUID.to_string(), movie.canon_uuid.is_some()),
        (CNDM.to_string(), has_track(b"cndm")),
        (DJMD.to_string(), has_track(b"djmd")),
    ];
    let mut vendors: Vec<(&str, ClipMeta)> = Vec::new();
    vendors.extend(arri::read(&movie.items).map(|m| (ARRI, m)));
    vendors.extend(panasonic::read(&movie.items, movie.pana.as_deref()).map(|m| (PANA, m)));
    if let Some(u) = &movie.canon_uuid {
        match canon::read_uuid(u) {
            Some(m) => vendors.push((CANON_UUID, m)),
            None => tracing::debug!("canon uuid: no Exif TIFF header inside the box"),
        }
    }
    if let Some(s) = meta_sample(&movie, b"cndm", src, media, &mut warnings) {
        vendors.push((CNDM, canon::read_rdd18(&s)));
    }
    if let Some(s) = meta_sample(&movie, b"djmd", src, media, &mut warnings) {
        match dji::read(&s) {
            Some(m) => vendors.push((DJMD, m)),
            None => tracing::debug!("djmd: no model starting \"DJI\" at field 2.2.1.4"),
        }
    }
    let vendor_colour = vendors
        .iter()
        .find(|(_, v)| v.colour.transfer.is_some() || v.colour.gamut.is_some())
        .map(|(label, v)| (*label, v.colour));
    for (label, v) in vendors {
        let before = present(&meta);
        fill(&mut meta, v);
        sources.gained(&before, &meta, |_| label.to_string());
    }
    let (g, keys) = generic(&movie);
    let before = present(&meta);
    fill(&mut meta, g);
    sources.gained(&before, &meta, |f| {
        keys.get(f).unwrap_or("QuickTime metadata").to_string()
    });
    let (a, keys) = apple::read(&movie.items, &video.items);
    let before = present(&meta);
    fill(&mut meta, a);
    sources.gained(&before, &meta, |f| {
        keys.get(f).unwrap_or("QuickTime metadata").to_string()
    });
    if meta.embedded_reel.is_none() {
        meta.embedded_reel = tc.and_then(|t| t.1);
        sources.set("embedded_reel", "tmcd sample entry name");
    }
    let colour_source = vendor_colour.map_or("video sample entry colr", |(l, _)| l);
    sources.set("transfer", colour_source);
    sources.set("gamut", colour_source);
    meta.colour = vendor_colour.map_or(ve.colour, |(_, c)| c);
    let base = |f: &str| match f {
        "frame_rate" => "video track stts",
        "frame_count" => "video track stsz",
        "width" | "height" => "video sample entry",
        "audio" => "sound track sample entries",
        "start_timecode" => "tmcd track",
        "embedded_reel" => "tmcd sample entry name",
        "recording_date" => "keys com.apple.quicktime.creationdate, \u{a9}day; mvhd creation_time",
        "camera_make" => "keys com.apple.quicktime.make, \u{a9}mak, manu",
        "camera_model" => "keys com.apple.quicktime.model, \u{a9}mod, modl",
        "transfer" | "gamut" => "video sample entry colr",
        _ => "",
    };
    sources.log(&meta, base, &blocks);

    Ok(Probe {
        codec,
        meta,
        frames,
        layout,
        codec_config,
        warnings,
        embedded_lut: None,
    })
}

/// Where samples may lie: inside the file and outside the movie header.
#[derive(Clone, Copy)]
struct Media {
    file_len: u64,
    /// File offset and length of the `moov` payload.
    moov: (u64, u64),
}

impl Media {
    fn holds(&self, r: ByteRange) -> bool {
        let Some(end) = r.offset.checked_add(r.len) else {
            return false;
        };
        let (m, l) = self.moov;
        end <= self.file_len && (end <= m || r.offset >= m.saturating_add(l))
    }
}

/// Copies every field of `from` into `into` where `into` has none.
pub(crate) fn fill(into: &mut ClipMeta, from: ClipMeta) {
    macro_rules! f {
        ($($name:ident),*) => { $( if into.$name.is_none() { into.$name = from.$name; } )* };
    }
    f!(
        recording_date,
        start_timecode,
        frame_rate,
        frame_count,
        width,
        height,
        camera_make,
        camera_model,
        camera_serial,
        lens,
        focal_length,
        stop,
        iso,
        shutter,
        white_balance,
        nd,
        sensor_fps,
        audio,
        embedded_reel,
        embedded_camera_id
    );
    let (to, slate) = (&mut into.slate, &from.slate);
    to.scene = to.scene.take().or_else(|| slate.scene.clone());
    to.shot = to.shot.take().or_else(|| slate.shot.clone());
    to.take = to.take.take().or_else(|| slate.take.clone());
    if into.colour.transfer.is_none() && into.colour.gamut.is_none() {
        into.colour = from.colour;
    }
}

/// What `moov` holds, parsed once.
#[derive(Default)]
struct Movie {
    creation_time: u64,
    tracks: Vec<Track>,
    items: Items,
    pana: Option<Vec<u8>>,
    canon_uuid: Option<Vec<u8>>,
}

impl Movie {
    fn parse(moov: &[u8], warnings: &mut Vec<String>) -> Movie {
        let mut m = Movie::default();
        let mut atoms = Atoms::new(moov);
        for a in atoms.by_ref() {
            match &a.kind {
                b"mvhd" => {
                    let mut r = bytes::Reader::new(a.body);
                    m.creation_time = match r.u8() {
                        Some(1) => r.skip(3).and_then(|_| r.u64()),
                        Some(_) => r.skip(3).and_then(|_| r.u32()).map(u64::from),
                        None => None,
                    }
                    .unwrap_or(0);
                }
                b"trak" => m.tracks.push(track::parse_track(a.body)),
                b"meta" => meta::read_meta(a.body, &mut m.items),
                b"udta" => {
                    meta::read_udta(a.body, &mut m.items);
                    if let Some(p) = child(a.body, b"PANA") {
                        m.pana = Some(p.to_vec());
                    }
                }
                b"uuid" if a.body.get(..16) == Some(&canon::UUID[..]) => {
                    m.canon_uuid = Some(a.body[16..].to_vec());
                }
                _ => {}
            }
        }
        if let Some(at) = atoms.damaged_at() {
            tracing::warn!(
                moov_offset = at,
                moov_len = moov.len(),
                "moov damaged: an atom header inside moov doesn't fit"
            );
            warnings.push(
                "The movie header is damaged; some tracks or metadata may be missing.".into(),
            );
        }
        m
    }
}

/// Constant frame duration from `stts` → timescale/duration, reduced.
fn frame_rate(t: &Track, warnings: &mut Vec<String>) -> Option<crate::types::Rational> {
    if t.timescale == 0 {
        return None;
    }
    if t.stts.iter().all(|&(n, _)| n == 0) {
        return None;
    }
    let Some(delta) = steady_delta(&t.stts) else {
        tracing::warn!(track = t.id, "vfr: {}", stts_summary(&t.stts, t.timescale));
        warnings.push("The frame rate varies within the clip.".into());
        return None;
    };
    if t.stts.iter().any(|&(n, d)| n > 0 && d != delta) {
        tracing::debug!(
            track = t.id,
            "timing jitter ignored: {}",
            stts_summary(&t.stts, t.timescale)
        );
    }
    Some(entry::reduce(t.timescale, delta))
}

/// The frame duration of a steady clip, from its `stts` runs: one duration
/// throughout, or one duration with a few frames a tick longer or shorter
/// (phones stamp frames at capture time, rounded to the timescale). A single
/// odd-length last sample is common and doesn't count. `None` when the rate
/// really varies.
fn steady_delta(stts: &[(u32, u32)]) -> Option<u32> {
    /// Frames this many ticks long or more can be a tick off and still steady.
    const MIN_TICKS: u32 = 20;
    /// At most this share of frames may be a tick off.
    const MAX_OFF: f64 = 0.02;
    let mut runs: Vec<_> = stts.iter().copied().filter(|&(n, _)| n > 0).collect();
    if runs.len() > 1 && runs.last().is_some_and(|&(n, _)| n == 1) {
        runs.pop();
    }
    let mut counts: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    for &(n, d) in &runs {
        *counts.entry(d).or_default() += u64::from(n);
    }
    let (&delta, &main) = counts.iter().max_by_key(|&(&d, &n)| (n, d))?;
    if delta == 0 {
        return None;
    }
    if counts.len() == 1 {
        return Some(delta);
    }
    let total: u64 = counts.values().sum();
    let near = counts.keys().all(|&d| d.abs_diff(delta) <= 1);
    let off = (total - main) as f64 / total as f64;
    (delta >= MIN_TICKS && near && off <= MAX_OFF).then_some(delta)
}

/// Timing evidence for a frame rate that varies: the `stts` run count, the
/// distinct deltas with their sample counts (top 5), the min and max delta
/// in ms, and the first 5 sample indices where the delta changes.
pub(crate) fn stts_summary(stts: &[(u32, u32)], timescale: u32) -> String {
    let mut counts: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let mut changes = Vec::new();
    let mut change_count = 0u64;
    let mut sample = 0u64;
    let mut prev = None;
    for &(n, d) in stts.iter().filter(|r| r.0 > 0) {
        if prev.is_some_and(|p| p != d) {
            change_count += 1;
            if changes.len() < 5 {
                changes.push(sample.to_string());
            }
        }
        prev = Some(d);
        *counts.entry(d).or_default() += u64::from(n);
        sample += u64::from(n);
    }
    let ms = |d: u32| {
        if timescale == 0 {
            format!("{d}ticks")
        } else {
            format!("{:.3}ms", f64::from(d) * 1000.0 / f64::from(timescale))
        }
    };
    let min = counts.keys().min().map_or("none".into(), |&d| ms(d));
    let max = counts.keys().max().map_or("none".into(), |&d| ms(d));
    let mut by_count: Vec<(u32, u64)> = counts.into_iter().collect();
    by_count.sort_by_key(|&(d, n)| (std::cmp::Reverse(n), d));
    let top: Vec<String> = by_count
        .iter()
        .take(5)
        .map(|(d, n)| format!("{d}x{n}"))
        .collect();
    let more = if change_count > changes.len() as u64 {
        ", ..."
    } else {
        ""
    };
    format!(
        "runs={} samples={sample} timescale={timescale} distinct={} deltas={} min={min} max={max} {change_count} changes at {}{more}",
        stts.len(),
        by_count.len(),
        top.join(" "),
        changes.join(", "),
    )
}

fn edit_warnings(t: &Track, warnings: &mut Vec<String>) {
    let plain = match t.edits.as_slice() {
        [] => true,
        [e] => e.rate_is_one && (e.media_time == 0 || e.media_time == track::first_cts(t)),
        _ => false,
    };
    if !plain {
        let edits: Vec<String> = t
            .edits
            .iter()
            .take(5)
            .map(|e| format!("media_time={} rate_one={}", e.media_time, e.rate_is_one))
            .collect();
        tracing::warn!(
            count = t.edits.len(),
            first_cts = track::first_cts(t),
            edits = %edits.join("; "),
            "edit list: not a plain start"
        );
        warnings.push(
            "The file has an edit list (a trimmed or delayed start); frame count and \
             frame index cover every recorded frame, not the edited range."
                .into(),
        );
    }
}

fn frame_index(t: &Track, media: Media, warnings: &mut Vec<String>) -> FrameIndex {
    // Every coded frame takes at least one byte of the file.
    if u64::from(t.sizes.count()) > media.file_len {
        tracing::warn!(
            samples = t.sizes.count(),
            file_len = media.file_len,
            "frame index: more samples than file bytes"
        );
        warnings.push(
            "The frame index can't be built: it lists more frames than the file has bytes.".into(),
        );
        return FrameIndex::Ranges(Vec::new());
    }
    let ranges = match track::sample_ranges(t) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                samples = t.sizes.count(),
                stsc = t.stsc.len(),
                chunks = t.chunk_offsets.len(),
                error = %e,
                "frame index: sample tables don't agree"
            );
            warnings.push(format!("The frame index can't be built: {e}."));
            return FrameIndex::Ranges(Vec::new());
        }
    };
    let ordered: Vec<ByteRange> = match track::display_order(t, ranges.len()) {
        Some(order) => order.into_iter().map(|i| ranges[i]).collect(),
        None => ranges,
    };
    let total = ordered.len();
    let mut inside = Vec::with_capacity(total);
    let mut outside = Vec::new();
    for (i, r) in ordered.into_iter().enumerate() {
        if media.holds(r) {
            inside.push(r);
        } else if outside.len() < 5 {
            outside.push(format!("#{i} at {} len {}", r.offset, r.len));
        }
    }
    if inside.len() < total {
        tracing::warn!(
            outside = total - inside.len(),
            total,
            file_len = media.file_len,
            moov_at = media.moov.0,
            moov_len = media.moov.1,
            first = %outside.join(", "),
            "frames outside media: sample ranges past the file end or inside moov"
        );
        warnings.push(format!(
            "{} of {} frames lie outside the file's media data; it may be truncated or damaged.",
            total - inside.len(),
            total
        ));
    }
    FrameIndex::Ranges(inside)
}

/// Start timecode and the timecode track's tape name.
fn timecode(
    movie: &Movie,
    video: &Track,
    src: &dyn Source,
    media: Media,
    warnings: &mut Vec<String>,
) -> Option<(Option<Timecode>, Option<String>)> {
    let tmcd = video
        .tmcd_refs
        .iter()
        .find_map(|id| {
            movie
                .tracks
                .iter()
                .find(|t| t.id == *id && &t.entry_format == b"tmcd")
        })
        .or_else(|| movie.tracks.iter().find(|t| &t.entry_format == b"tmcd"))?;
    let entry = entry::timecode_entry(&tmcd.entry)?;
    let name = entry.name.clone();
    if entry.counter {
        tracing::warn!(
            track = tmcd.id,
            timescale = entry.timescale,
            frame_duration = entry.frame_duration,
            fps = entry.frames_per_second,
            "timecode counter: tmcd flags have the counter bit (0x8) set"
        );
        warnings.push("The timecode track is a counter, not a time of day.".into());
        return Some((None, name));
    }
    let first = track::first_sample(tmcd);
    let start = first
        .filter(|r| r.len >= 4 && media.holds(*r))
        .and_then(|r| {
            let mut b = [0u8; 4];
            src.read_at(r.offset, &mut b)
                .ok()
                .map(|_| u32::from_be_bytes(b))
        });
    let (Some(frames), Some(base)) = (start, entry.base()) else {
        tracing::warn!(
            track = tmcd.id,
            first_sample = ?first.map(|r| (r.offset, r.len)),
            in_media = first.is_some_and(|r| media.holds(r)),
            file_len = media.file_len,
            fps = entry.frames_per_second,
            timescale = entry.timescale,
            frame_duration = entry.frame_duration,
            "start timecode: no readable 4-byte first sample or no counting base"
        );
        warnings.push("The start timecode can't be read.".into());
        return Some((None, name));
    };
    Some((
        Some(Timecode {
            frames: frames as u64,
            base,
            drop_frame: entry.drop_frame,
        }),
        name,
    ))
}

/// The first sample of the first track whose sample entry is `format`.
fn meta_sample(
    movie: &Movie,
    format: &[u8; 4],
    src: &dyn Source,
    media: Media,
    warnings: &mut Vec<String>,
) -> Option<Vec<u8>> {
    let t = movie.tracks.iter().find(|t| &t.entry_format == format)?;
    let r = track::first_sample(t).filter(|r| r.len > 0 && r.len <= MAX_META_SAMPLE)?;
    let mut buf = vec![0u8; r.len as usize];
    match media.holds(r).then(|| src.read_at(r.offset, &mut buf)) {
        Some(Ok(())) => Some(buf),
        _ => {
            tracing::warn!(
                track = t.id,
                format = %String::from_utf8_lossy(format),
                offset = r.offset,
                len = r.len,
                file_len = media.file_len,
                in_media = media.holds(r),
                "metadata sample: first sample can't be read"
            );
            warnings.push(format!(
                "The camera metadata track ('{}') can't be read.",
                String::from_utf8_lossy(format)
            ));
            None
        }
    }
}

/// Every sound track's channels added up; rate and depth from the first.
/// A movie with no sound track has no sound. `None` when a sound track's
/// description can't be read, since the total would then be a guess.
fn audio(movie: &Movie) -> Option<Audio> {
    let mut total = Audio::default();
    for t in movie.tracks.iter().filter(|t| &t.handler == b"soun") {
        let e = entry::sound_entry(&t.entry_format, &t.entry)?;
        if total.channels == 0 {
            total.sample_rate = e.sample_rate;
            total.bits = e.bits;
        }
        total.channels += e.channels;
    }
    Some(total)
}

/// Make, model and date from QuickTime/ISO generic metadata and `mvhd`, and
/// the key each came from.
fn generic(movie: &Movie) -> (ClipMeta, Sources) {
    let it = &movie.items;
    let first = |keys: &[&'static str]| keys.iter().find_map(|&k| it.text(k).map(|v| (k, v)));
    let mut keys = Sources::default();
    let date = first(&["com.apple.quicktime.creationdate", "\u{a9}day"])
        .and_then(|(k, s)| display::iso_local(&s).map(|d| (k, d)))
        .or_else(|| display::qt_time(movie.creation_time).map(|d| ("mvhd creation_time", d)));
    let make = first(&["com.apple.quicktime.make", "\u{a9}mak", "manu"]);
    let model = first(&["com.apple.quicktime.model", "\u{a9}mod", "modl"]);
    let mut take = |field, v: Option<(&str, String)>| {
        v.map(|(k, v)| {
            let source = if k.starts_with("mvhd") {
                k.to_string()
            } else {
                format!("QuickTime key {k}")
            };
            keys.set(field, source);
            v
        })
    };
    let m = ClipMeta {
        camera_make: take("camera_make", make),
        camera_model: take("camera_model", model),
        recording_date: take("recording_date", date),
        ..ClipMeta::default()
    };
    (m, keys)
}
