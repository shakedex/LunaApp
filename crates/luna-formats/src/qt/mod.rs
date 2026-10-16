//! QuickTime `.mov` and ISO-BMFF `.mp4`: clean-room from ISO/IEC 14496-12 and
//! Apple's QuickTime File Format specification, plus vendor metadata learned
//! from camera files.
//!
//! The file is read by seeking: top-level atom headers, the whole `moov`
//! (bounded), and a handful of small samples (the first timecode sample and
//! the first vendor-metadata sample). Coded frames in `mdat` are never read.

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
        warnings.push(format!("The video track's '{table}' table is damaged."));
    }

    let codec = entry::codec(&video.entry_format);
    let mut ve = entry::video_entry(&video.entry).unwrap_or_default();
    let codec_config = ve.config.take();
    if ve.fields == Some(2) {
        warnings.push("The picture is interlaced (two fields per frame).".into());
    }
    let (width, height) = ve.clean.unwrap_or((ve.width, ve.height));
    let layout = PictureLayout {
        stored_width: ve.width,
        stored_height: ve.height,
        raw_bits: None,
        pixel_aspect: ve.pixel_aspect,
    };

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

    meta.audio = audio(&movie);

    let tc = timecode(&movie, video, src, media, &mut warnings);
    meta.start_timecode = tc.as_ref().and_then(|t| t.0);

    // Vendor metadata first: it is the most specific. Generic QuickTime
    // metadata and the movie header only fill what vendors leave empty.
    let mut vendors: Vec<ClipMeta> = Vec::new();
    vendors.extend(arri::read(&movie.items));
    vendors.extend(panasonic::read(&movie.items, movie.pana.as_deref()));
    if let Some(u) = &movie.canon_uuid {
        vendors.extend(canon::read_uuid(u));
    }
    if let Some(s) = meta_sample(&movie, b"cndm", src, media, &mut warnings) {
        vendors.push(canon::read_rdd18(&s));
    }
    if let Some(s) = meta_sample(&movie, b"djmd", src, media, &mut warnings) {
        vendors.extend(dji::read(&s));
    }
    let vendor_colour = vendors
        .iter()
        .map(|v| v.colour)
        .find(|c| c.transfer.is_some() || c.gamut.is_some());
    for v in vendors {
        fill(&mut meta, v);
    }
    fill(&mut meta, generic(&movie));
    if meta.embedded_reel.is_none() {
        meta.embedded_reel = tc.and_then(|t| t.1);
    }
    meta.colour = vendor_colour.unwrap_or(ve.colour);

    Ok(Probe {
        codec,
        meta,
        frames,
        layout,
        codec_config,
        warnings,
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
        if atoms.damaged() {
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
    let runs: Vec<_> = t.stts.iter().filter(|&&(n, _)| n > 0).collect();
    let (&&(_, delta), rest) = runs.split_first()?;
    // A single odd-length last sample is common and doesn't make a clip VFR.
    let constant = rest
        .iter()
        .enumerate()
        .all(|(i, &&(n, d))| d == delta || (i == rest.len() - 1 && n == 1));
    if !constant || delta == 0 {
        warnings.push("The frame rate varies within the clip.".into());
        return None;
    }
    Some(entry::reduce(t.timescale, delta))
}

fn edit_warnings(t: &Track, warnings: &mut Vec<String>) {
    let plain = match t.edits.as_slice() {
        [] => true,
        [e] => e.rate_is_one && (e.media_time == 0 || e.media_time == track::first_cts(t)),
        _ => false,
    };
    if !plain {
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
        warnings.push(
            "The frame index can't be built: it lists more frames than the file has bytes.".into(),
        );
        return FrameIndex::Ranges(Vec::new());
    }
    let ranges = match track::sample_ranges(t) {
        Ok(r) => r,
        Err(e) => {
            warnings.push(format!("The frame index can't be built: {e}."));
            return FrameIndex::Ranges(Vec::new());
        }
    };
    let ordered: Vec<ByteRange> = match track::display_order(t, ranges.len()) {
        Some(order) => order.into_iter().map(|i| ranges[i]).collect(),
        None => ranges,
    };
    let total = ordered.len();
    let inside: Vec<ByteRange> = ordered.into_iter().filter(|r| media.holds(*r)).collect();
    if inside.len() < total {
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
        warnings.push("The timecode track is a counter, not a time of day.".into());
        return Some((None, name));
    }
    let start = track::first_sample(tmcd)
        .filter(|r| r.len >= 4 && media.holds(*r))
        .and_then(|r| {
            let mut b = [0u8; 4];
            src.read_at(r.offset, &mut b)
                .ok()
                .map(|_| u32::from_be_bytes(b))
        });
    let (Some(frames), Some(base)) = (start, entry.base()) else {
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

/// Make, model and date from QuickTime/ISO generic metadata and `mvhd`.
fn generic(movie: &Movie) -> ClipMeta {
    let it = &movie.items;
    let date = it
        .text("com.apple.quicktime.creationdate")
        .or_else(|| it.text("©day"))
        .and_then(|s| display::iso_local(&s))
        .or_else(|| display::qt_time(movie.creation_time));
    ClipMeta {
        camera_make: it
            .text("com.apple.quicktime.make")
            .or_else(|| it.text("©mak"))
            .or_else(|| it.text("manu")),
        camera_model: it
            .text("com.apple.quicktime.model")
            .or_else(|| it.text("©mod"))
            .or_else(|| it.text("modl")),
        recording_date: date,
        ..ClipMeta::default()
    }
}
