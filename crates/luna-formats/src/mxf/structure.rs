//! The structural model (ST 377-1 §9–10): identification, packages, tracks,
//! timecode, and the picture essence descriptor, resolved from local sets.

use super::klv::Ul;
use crate::types::Audio;

use super::metadata::{INSTANCE_UID, LocalSet, Metadata, Prop, prop, timestamp};

// Identification
const COMPANY_NAME: Prop = prop(0x3c01, "060e2b34010101020520070102010000");
const PRODUCT_NAME: Prop = prop(0x3c02, "060e2b34010101020520070103010000");
const MODIFICATION_DATE: Prop = prop(0x3c06, "060e2b34010101020702011002030000");
// Packages
const PACKAGE_UID: Prop = prop(0x4401, "060e2b34010101010101151000000000");
const TRACKS: Prop = prop(0x4403, "060e2b34010101020601010406050000");
const PACKAGE_CREATION_DATE: Prop = prop(0x4405, "060e2b34010101020702011001030000");
const DESCRIPTOR: Prop = prop(0x4701, "060e2b34010101020601010402030000");
// Tracks
const TRACK_ID: Prop = prop(0x4801, "060e2b34010101020107010100000000");
const TRACK_NUMBER: Prop = prop(0x4804, "060e2b34010101020104010300000000");
const EDIT_RATE: Prop = prop(0x4b01, "060e2b34010101020530040500000000");
const SEQUENCE: Prop = prop(0x4803, "060e2b34010101020601010402040000");
// Sequences and components
const DATA_DEFINITION: Prop = prop(0x0201, "060e2b34010101020407010000000000");
const DURATION: Prop = prop(0x0202, "060e2b34010101020702020101030000");
const COMPONENTS: Prop = prop(0x1001, "060e2b34010101020601010406090000");
const SOURCE_PACKAGE_ID: Prop = prop(0x1101, "060e2b34010101020601010301000000");
const START_TIMECODE: Prop = prop(0x1501, "060e2b34010101020702010301050000");
const ROUNDED_TC_BASE: Prop = prop(0x1502, "060e2b34010101020404010102060000");
const DROP_FRAME: Prop = prop(0x1503, "060e2b34010101010404010105000000");
// Essence container data
const LINKED_PACKAGE_UID: Prop = prop(0x2701, "060e2b34010101020601010601000000");
const INDEX_SID: Prop = prop(0x3f06, "060e2b34010101040103040500000000");
const BODY_SID: Prop = prop(0x3f07, "060e2b34010101040103040400000000");
// Descriptors
const SUB_DESCRIPTORS: Prop = prop(0x3f01, "060e2b340101010406010104060b0000");
const LINKED_TRACK_ID: Prop = prop(0x3006, "060e2b34010101050601010305000000");
const SAMPLE_RATE: Prop = prop(0x3001, "060e2b34010101010406010100000000");
const CONTAINER_DURATION: Prop = prop(0x3002, "060e2b34010101010406010200000000");
const ESSENCE_CONTAINER: Prop = prop(0x3004, "060e2b34010101020601010401020000");
const PICTURE_CODING: Prop = prop(0x3201, "060e2b34010101020401060100000000");
const STORED_HEIGHT: Prop = prop(0x3202, "060e2b34010101010401050201000000");
const STORED_WIDTH: Prop = prop(0x3203, "060e2b34010101010401050202000000");
const DISPLAY_HEIGHT: Prop = prop(0x3208, "060e2b3401010101040105010b000000");
const DISPLAY_WIDTH: Prop = prop(0x3209, "060e2b3401010101040105010c000000");
const FRAME_LAYOUT: Prop = prop(0x320c, "060e2b34010101010401030104000000");
const ASPECT_RATIO: Prop = prop(0x320e, "060e2b34010101010401010101000000");
const TRANSFER: Prop = prop(0x3210, "060e2b34010101020401020101010200");
const PRIMARIES: Prop = prop(0x3219, "060e2b34010101090401020101060100");
const IMAGE_START_OFFSET: Prop = prop(0x3213, "060e2b34010101020418010200000000");
const COMPONENT_DEPTH: Prop = prop(0x3301, "060e2b3401010102040105030a000000");
const AUDIO_SAMPLING_RATE: Prop = prop(0x3d03, "060e2b34010101050402030101010000");
const CHANNEL_COUNT: Prop = prop(0x3d07, "060e2b34010101050402010104000000");
const QUANTIZATION_BITS: Prop = prop(0x3d01, "060e2b34010101040402030304000000");

const DD_PICTURE: [u8; 13] = [
    0x06, 0x0e, 0x2b, 0x34, 0x04, 0x01, 0x01, 0x01, 0x01, 0x03, 0x02, 0x02, 0x01,
];
const DD_TIMECODE: [u8; 13] = [
    0x06, 0x0e, 0x2b, 0x34, 0x04, 0x01, 0x01, 0x01, 0x01, 0x03, 0x02, 0x01, 0x01,
];
const DD_SOUND: [u8; 13] = [
    0x06, 0x0e, 0x2b, 0x34, 0x04, 0x01, 0x01, 0x01, 0x01, 0x03, 0x02, 0x02, 0x02,
];
const DD_DATA: [u8; 13] = [
    0x06, 0x0e, 0x2b, 0x34, 0x04, 0x01, 0x01, 0x01, 0x01, 0x03, 0x02, 0x02, 0x03,
];

pub(crate) type Stamp = (u16, u8, u8, u8, u8, u8);

#[derive(Debug, Default, Clone)]
pub(crate) struct Identification {
    pub company: Option<String>,
    pub product: Option<String>,
    pub modified: Option<Stamp>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct PictureDescriptor {
    pub stored_width: Option<u32>,
    pub stored_height: Option<u32>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub aspect: Option<(i32, i32)>,
    pub coding: Option<Ul>,
    pub container: Option<Ul>,
    pub transfer: Option<Ul>,
    pub primaries: Option<Ul>,
    pub frame_layout: Option<u8>,
    pub component_depth: Option<u32>,
    pub image_start_offset: Option<u32>,
    pub container_duration: Option<i64>,
    pub sample_rate: Option<(i32, i32)>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Structure {
    pub ident: Identification,
    pub created: Option<Stamp>,
    pub edit_rate: Option<(i32, i32)>,
    /// Picture track duration in edit units, when the file states it.
    pub duration: Option<u64>,
    /// (start frames, rounded base, drop frame)
    pub timecode: Option<(i64, u16, bool)>,
    /// TrackNumber of the file package's picture track (= last 4 key bytes of
    /// its essence elements).
    pub picture_track_number: Option<u32>,
    /// TrackNumbers of the file package's data tracks (ANC, ARRI frame metadata).
    pub data_track_numbers: Vec<u32>,
    pub picture: Option<PictureDescriptor>,
    /// The file package's sound, from its sound descriptors.
    pub audio: Option<Audio>,
    pub body_sid: Option<u32>,
    pub index_sid: Option<u32>,
}

fn sequence_of<'a>(md: &'a Metadata, track: &LocalSet) -> Option<&'a LocalSet> {
    md.by_uid(&track.uid(SEQUENCE)?)
}

fn data_def(md: &Metadata, seq: &LocalSet) -> Option<Ul> {
    seq.ul(DATA_DEFINITION).or_else(|| {
        let first = seq.refs(COMPONENTS).into_iter().next()?;
        md.by_uid(&first)?.ul(DATA_DEFINITION)
    })
}

fn components<'a>(md: &'a Metadata, seq: &'a LocalSet) -> Vec<&'a LocalSet> {
    // A track's "sequence" may itself be a single component.
    let refs = seq.refs(COMPONENTS);
    if refs.is_empty() {
        return vec![seq];
    }
    refs.iter().filter_map(|u| md.by_uid(u)).collect()
}

fn tracks<'a>(md: &'a Metadata, package: &LocalSet) -> Vec<&'a LocalSet> {
    package
        .refs(TRACKS)
        .iter()
        .filter_map(|u| md.by_uid(u))
        .collect()
}

fn timecode_of(md: &Metadata, package: &LocalSet) -> Option<(i64, u16, bool)> {
    for t in tracks(md, package) {
        let Some(seq) = sequence_of(md, t) else {
            continue;
        };
        for c in components(md, seq) {
            if c.kind() == Some(0x14) || c.has(START_TIMECODE) {
                let start = c.i64(START_TIMECODE)?;
                let base = c.u16(ROUNDED_TC_BASE)?;
                // A timecode at or past 24 h is not one.
                if start < 0 || base == 0 || base > 1000 || start >= base as i64 * 86_400 {
                    return None;
                }
                return Some((start, base, c.u8(DROP_FRAME).unwrap_or(0) != 0));
            }
        }
        let _ = data_def(md, seq).filter(|d| d.starts(&DD_TIMECODE));
    }
    None
}

fn is_sound_descriptor(s: &LocalSet) -> bool {
    s.has(CHANNEL_COUNT) || s.has(AUDIO_SAMPLING_RATE)
}

/// Channels added up over the sound descriptors; rate and depth from the first.
/// `None` when a sound descriptor doesn't state its channel count, or the
/// package has sound tracks but no sound descriptors to describe them.
fn audio_of(sounds: &[&LocalSet], sound_tracks: usize) -> Option<Audio> {
    if sounds.is_empty() {
        return (sound_tracks == 0).then_some(Audio::default());
    }
    let first = sounds[0];
    Some(Audio {
        channels: sounds
            .iter()
            .map(|s| s.u32(CHANNEL_COUNT))
            .sum::<Option<u32>>()?,
        sample_rate: first
            .rational(AUDIO_SAMPLING_RATE)
            .filter(|&(n, d)| n > 0 && d > 0)
            .map(|(n, d)| (n as f64 / d as f64).round() as u32),
        bits: first
            .u32(QUANTIZATION_BITS)
            .filter(|b| (8..=64).contains(b)),
    })
}

fn is_picture_descriptor(s: &LocalSet) -> bool {
    s.has(STORED_WIDTH) || s.has(PICTURE_CODING) || s.has(DISPLAY_WIDTH)
}

fn picture_descriptor(s: &LocalSet) -> PictureDescriptor {
    PictureDescriptor {
        stored_width: s.u32(STORED_WIDTH),
        stored_height: s.u32(STORED_HEIGHT),
        display_width: s.u32(DISPLAY_WIDTH),
        display_height: s.u32(DISPLAY_HEIGHT),
        aspect: s.rational(ASPECT_RATIO),
        coding: s.ul(PICTURE_CODING),
        container: s.ul(ESSENCE_CONTAINER),
        transfer: s.ul(TRANSFER),
        primaries: s.ul(PRIMARIES),
        frame_layout: s.u8(FRAME_LAYOUT),
        component_depth: s.u32(COMPONENT_DEPTH),
        image_start_offset: s.u32(IMAGE_START_OFFSET),
        container_duration: s.i64(CONTAINER_DURATION),
        sample_rate: s.rational(SAMPLE_RATE),
    }
}

pub(crate) fn resolve(md: &Metadata) -> Structure {
    let mut st = Structure::default();

    if let Some(id) = md.of_kind(0x30).last() {
        st.ident = Identification {
            company: id.utf16(COMPANY_NAME),
            product: id.utf16(PRODUCT_NAME),
            modified: id.get(MODIFICATION_DATE).and_then(timestamp),
        };
    }

    let material = md.of_kind(0x36).next();
    let mut source_uid: Option<Vec<u8>> = None;
    if let Some(mp) = material {
        st.created = mp.get(PACKAGE_CREATION_DATE).and_then(timestamp);
        st.timecode = timecode_of(md, mp);
        for t in tracks(md, mp) {
            let Some(seq) = sequence_of(md, t) else {
                continue;
            };
            if !data_def(md, seq).is_some_and(|d| d.starts(&DD_PICTURE)) {
                continue;
            }
            st.edit_rate = t.rational(EDIT_RATE).filter(|&(n, d)| n > 0 && d > 0);
            let dur = seq
                .i64(DURATION)
                .or_else(|| components(md, seq).iter().map(|c| c.i64(DURATION)).sum());
            st.duration = dur.filter(|&d| d >= 0).map(|d| d as u64);
            source_uid = components(md, seq)
                .iter()
                .find_map(|c| c.get(SOURCE_PACKAGE_ID).map(|v| v.to_vec()));
            break;
        }
    }

    // The file (source) package carrying essence: the one the material
    // package points at, else the first with a descriptor.
    let packages: Vec<&LocalSet> = md.of_kind(0x37).collect();
    let file_pkg = packages
        .iter()
        .find(|p| {
            source_uid
                .as_deref()
                .is_some_and(|u| p.get(PACKAGE_UID) == Some(u))
                && p.has(DESCRIPTOR)
        })
        .or_else(|| packages.iter().find(|p| p.has(DESCRIPTOR)))
        .copied();

    if let Some(fp) = file_pkg {
        if st.timecode.is_none() {
            st.timecode = timecode_of(md, fp);
        }
        let mut picture_track_id = None;
        let mut sound_tracks = 0;
        for t in tracks(md, fp) {
            let Some(seq) = sequence_of(md, t) else {
                continue;
            };
            let dd = data_def(md, seq);
            if dd.is_some_and(|d| d.starts(&DD_PICTURE)) && st.picture_track_number.is_none() {
                st.picture_track_number = t.u32(TRACK_NUMBER);
                picture_track_id = t.u32(TRACK_ID);
                if st.edit_rate.is_none() {
                    st.edit_rate = t.rational(EDIT_RATE).filter(|&(n, d)| n > 0 && d > 0);
                }
                if st.duration.is_none() {
                    st.duration = seq.i64(DURATION).filter(|&d| d >= 0).map(|d| d as u64);
                }
            } else if dd.is_some_and(|d| d.starts(&DD_SOUND)) {
                sound_tracks += 1;
            } else if dd.is_some_and(|d| d.starts(&DD_DATA))
                && let Some(n) = t.u32(TRACK_NUMBER)
            {
                st.data_track_numbers.push(n);
            }
        }
        if let Some(d) = fp.uid(DESCRIPTOR).and_then(|u| md.by_uid(&u)) {
            let candidates: Vec<&LocalSet> = if d.has(SUB_DESCRIPTORS) {
                d.refs(SUB_DESCRIPTORS)
                    .iter()
                    .filter_map(|u| md.by_uid(u))
                    .collect()
            } else {
                vec![d]
            };
            let sounds: Vec<&LocalSet> = candidates
                .iter()
                .copied()
                .filter(|s| is_sound_descriptor(s))
                .collect();
            st.audio = audio_of(&sounds, sound_tracks);
            let pics: Vec<&&LocalSet> = candidates
                .iter()
                .filter(|s| is_picture_descriptor(s))
                .collect();
            let chosen = pics
                .iter()
                .find(|s| picture_track_id.is_some() && s.u32(LINKED_TRACK_ID) == picture_track_id)
                .or_else(|| pics.first());
            if let Some(s) = chosen {
                let mut pd = picture_descriptor(s);
                if pd.container.is_none() {
                    pd.container = d.ul(ESSENCE_CONTAINER);
                }
                st.picture = Some(pd);
            }
        }
        let fp_uid = fp.get(PACKAGE_UID);
        if let Some(ecd) = md
            .of_kind(0x23)
            .find(|e| fp_uid.is_some() && e.get(LINKED_PACKAGE_UID) == fp_uid)
            .or_else(|| md.of_kind(0x23).next())
        {
            st.body_sid = ecd.u32(BODY_SID).filter(|&s| s != 0);
            st.index_sid = ecd.u32(INDEX_SID).filter(|&s| s != 0);
        }
    }
    let _ = INSTANCE_UID;
    st
}
