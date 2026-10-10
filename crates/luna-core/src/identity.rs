//! Which camera and reel each clip belongs to, and the camera → card → clip
//! grouping of the report.
//!
//! Per clip, camera and reel each come from the first step of the chain that
//! names them: embedded metadata, then camera-original file-name conventions,
//! then folder structure, then the bare folder name. Clips in one folder form a
//! card whose camera and reel come from the strongest evidence among them;
//! clips whose embedded reels disagree are split into separate cards.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use luna_formats::{ClipFiles, ClipMeta};
use luna_report::IdentitySource;

use crate::scan::relative;

/// Camera and reel named by a camera-original file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameIdentity {
    pub camera: String,
    pub reel: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraGroup {
    pub name: String,
    pub cards: Vec<CardGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardGroup {
    /// The reel as printed. When two folders of one camera claim the same reel,
    /// each gets its folder appended: `A0001 (A_CAM/A002)`.
    pub reel: String,
    /// The card's folder relative to the root (`.` for the root itself).
    pub folder: String,
    pub clips: Vec<IdentifiedClip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentifiedClip {
    /// Index into the slice given to [`resolve_identity`].
    pub index: usize,
    pub camera_source: IdentitySource,
    pub reel_source: IdentitySource,
}

/// Groups clips into cameras and cards, ordered for the report: cameras by
/// letter then name, cards by reel (natural order), clips by recording start
/// (date, then start timecode) with the file name as tie-break.
///
/// Runs after probing so embedded identity is known; pass `None` for a clip
/// whose probe failed.
pub fn resolve_identity(root: &Path, clips: &[(ClipFiles, Option<&ClipMeta>)]) -> Vec<CameraGroup> {
    // Per-clip evidence, grouped by folder.
    let mut folders: BTreeMap<PathBuf, Vec<Evidence>> = BTreeMap::new();
    for (index, (files, meta)) in clips.iter().enumerate() {
        let dir = files.main.parent().unwrap_or(root).to_path_buf();
        let ev = evidence(root, &dir, index, files, *meta);
        folders.entry(dir).or_default().push(ev);
    }

    let mut cameras: Vec<CameraGroup> = Vec::new();
    // The camera body behind each entry of `cameras`, when the metadata names one.
    let mut bodies: Vec<(String, Option<Body>)> = Vec::new();
    let mut pending: Vec<(String, Option<Body>, CardGroup)> = Vec::new();
    for (dir, evs) in folders {
        let folder = relative(root, &dir);
        for bucket in split_by_embedded_reel(evs) {
            let camera = strongest(bucket.iter().map(|e| &e.camera));
            let reel = strongest(bucket.iter().map(|e| &e.reel));
            let mut card_clips: Vec<(&Evidence, IdentifiedClip)> = bucket
                .iter()
                .map(|e| {
                    let id = IdentifiedClip {
                        index: e.index,
                        camera_source: own_or_card(&e.camera, &camera),
                        reel_source: own_or_card(&e.reel, &reel),
                    };
                    (e, id)
                })
                .collect();
            card_clips.sort_by(|a, b| clip_order(clips, a.0, b.0));
            let card_body = body(card_clips.iter().map(|(e, _)| clips[e.index].1));
            let card = CardGroup {
                reel: reel.value,
                folder: folder.clone(),
                clips: card_clips.into_iter().map(|(_, id)| id).collect(),
            };
            pending.push((camera.value, card_body, card));
        }
    }
    group_by_body(pending, &mut cameras, &mut bodies);
    name_bodies(&mut cameras, &bodies);

    for cam in &mut cameras {
        disambiguate_reels(&mut cam.cards);
        cam.cards
            .sort_by(|a, b| natural_cmp(&a.reel, &b.reel).then_with(|| a.folder.cmp(&b.folder)));
    }
    cameras.sort_by(|a, b| camera_key(&a.name, &b.name));
    cameras
}

/// One letter can be several physical cameras (bodies left on the default
/// "A"): cards whose metadata names different bodies become separate cameras.
/// A card with no known body joins its letter only when that letter has at
/// most one known body; otherwise it stays under the bare letter.
fn group_by_body(
    pending: Vec<(String, Option<Body>, CardGroup)>,
    cameras: &mut Vec<CameraGroup>,
    bodies: &mut Vec<(String, Option<Body>)>,
) {
    let (known, unknown): (Vec<_>, Vec<_>) = pending.into_iter().partition(|(_, b, _)| b.is_some());
    for (name, card_body, card) in known {
        match bodies
            .iter()
            .position(|(n, b)| same(n, &name) && b.is_some() && compatible(b, &card_body))
        {
            Some(i) => cameras[i].cards.push(card),
            None => {
                bodies.push((name.clone(), card_body));
                cameras.push(CameraGroup {
                    name,
                    cards: vec![card],
                });
            }
        }
    }
    for (name, _, card) in unknown {
        let known_for_name: Vec<usize> = (0..bodies.len())
            .filter(|&i| same(&bodies[i].0, &name) && bodies[i].1.is_some())
            .collect();
        let target = match known_for_name.as_slice() {
            [only] => Some(*only),
            _ => bodies
                .iter()
                .position(|(n, b)| same(n, &name) && b.is_none()),
        };
        match target {
            Some(i) => cameras[i].cards.push(card),
            None => {
                bodies.push((name.clone(), None));
                cameras.push(CameraGroup {
                    name,
                    cards: vec![card],
                });
            }
        }
    }
}

/// A physical camera as the metadata names it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Body {
    model: Option<String>,
    serial: Option<String>,
}

/// The most common body among a card's clips; `None` when no clip names one.
fn body<'a>(metas: impl Iterator<Item = Option<&'a ClipMeta>>) -> Option<Body> {
    let mut counts: Vec<(Body, usize)> = Vec::new();
    for m in metas.flatten() {
        if m.camera_model.is_none() && m.camera_serial.is_none() {
            continue;
        }
        let b = Body {
            model: m.camera_model.clone(),
            serial: m.camera_serial.clone(),
        };
        match counts.iter_mut().find(|(c, _)| *c == b) {
            Some((_, n)) => *n += 1,
            None => counts.push((b, 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(b, _)| b)
}

/// Unknown bodies join anything; known ones must name the same camera.
fn compatible(a: &Option<Body>, b: &Option<Body>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => match (&a.serial, &b.serial) {
            (Some(x), Some(y)) => x == y,
            _ => a.model == b.model,
        },
        _ => true,
    }
}

/// When one camera name covers several bodies, names each one: the model, plus
/// the serial when the model alone doesn't tell them apart.
fn name_bodies(cameras: &mut [CameraGroup], bodies: &[(String, Option<Body>)]) {
    for i in 0..cameras.len() {
        let name = bodies[i].0.clone();
        let siblings: Vec<usize> = (0..cameras.len())
            .filter(|&j| same(&bodies[j].0, &name))
            .collect();
        if siblings.len() < 2 {
            continue;
        }
        let Some(b) = &bodies[i].1 else {
            continue;
        };
        let model_shared = siblings
            .iter()
            .any(|&j| j != i && bodies[j].1.as_ref().is_some_and(|o| o.model == b.model));
        let mut label = b.model.clone().unwrap_or_default();
        if (model_shared || label.is_empty())
            && let Some(s) = &b.serial
        {
            label = if label.is_empty() {
                format!("#{s}")
            } else {
                format!("{label} #{s}")
            };
        }
        if !label.is_empty() {
            cameras[i].name = format!("{name} · {label}");
        }
    }
}

/// Recognises camera-original naming conventions. Conservative: `None` unless
/// the whole pattern matches.
///
/// `stem` is the file name without its extension.
pub fn identity_from_file_name(stem: &str) -> Option<NameIdentity> {
    let t: Vec<&str> = stem.split('_').collect();
    let tok = |i: usize| t.get(i).copied().unwrap_or("");
    let id = |camera: &str, reel: &str| {
        Some(NameIdentity {
            camera: camera.to_ascii_uppercase(),
            reel: reel.to_ascii_uppercase(),
        })
    };

    // ALEXA 35: A0001C027_251020_135413_p1I7H, DW0001C014_…
    for letters in [1, 2] {
        let head = format!("{}DDDDCDDD", "L".repeat(letters));
        if shape(tok(0), &head)
            && shape(tok(1), "DDDDDD")
            && shape(tok(2), "DDDDDD")
            && starts_shape(tok(3), "xxxxx")
        {
            return id(&tok(0)[..letters], &tok(0)[..letters + 4]);
        }
    }
    // ARRI A001C003_YYMMDD_RXXX, Sony VENICE B005C039_201101OZ,
    // Canon A001C004_22032472_CANON, DJI A001C0004_210816_0000_…
    if shape(tok(0), "LDDDCDDD") || shape(tok(0), "LDDDCDDDD") {
        let arri = shape(tok(1), "DDDDDD") && starts_shape(tok(2), "XXXX");
        let dji = shape(tok(0), "LDDDCDDDD") && shape(tok(1), "DDDDDD") && shape(tok(2), "DDDD");
        let sony = t.len() == 2 && shape(tok(1), "DDDDDDXX");
        let canon = shape(tok(1), "DDDDDDDD") && tok(2).starts_with("CANON");
        if arri || dji || sony || canon {
            return id(&tok(0)[..1], &tok(0)[..4]);
        }
    }
    // Canon S_0001C038X241003_1807575U_CANONRAW, A_0001C107H…_CANON
    if shape(tok(0), "l")
        && shape(tok(1), "DDDDCDDDLDDDDDD")
        && shape(tok(2), "DDDDDDXX")
        && t.iter()
            .skip(3)
            .any(|s| s.to_ascii_uppercase().starts_with("CANON"))
    {
        return id(tok(0), &format!("{}{}", tok(0), &tok(1)[..4]));
    }
    // RED A005_C055_0819CM[_001]
    if shape(tok(0), "LDDD")
        && shape(tok(1), "LDDD")
        && shape(tok(2), "DDDDXX")
        && (t.len() == 3 || t.len() == 4 && shape(tok(3), "DDD"))
    {
        return id(&tok(0)[..1], tok(0));
    }
    // Blackmagic D001_09150935_C013
    if t.len() == 3
        && shape(tok(0), "LDDD")
        && shape(tok(1), "DDDDDDDD")
        && (shape(tok(2), "CDDD") || shape(tok(2), "CDDDD"))
    {
        return id(&tok(0)[..1], tok(0));
    }
    None
}

/// `A_CAM`, `A-CAM`, `ACAM`, `A Cam`, `CAM_A`, `B Camera` → the camera letter.
pub fn camera_from_folder_name(name: &str) -> Option<String> {
    let up = name.trim().to_ascii_uppercase();
    let letter = |s: &str| {
        let s = s.strip_prefix([' ', '_', '-']).unwrap_or(s);
        (s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase()).then(|| s.to_string())
    };
    for word in ["CAMERA", "CAM"] {
        if let Some(rest) = up.strip_prefix(word)
            && let Some(l) = letter(rest)
        {
            return Some(l);
        }
        if let Some(rest) = up.strip_suffix(word) {
            let rest: String = rest.chars().rev().collect();
            if let Some(l) = letter(&rest) {
                return Some(l);
            }
        }
    }
    None
}

/// A folder named like a reel: one or two letters and three or four digits
/// (`A001`, `s004`, `DW0001`).
pub fn looks_like_reel(name: &str) -> bool {
    ["lDDD", "lDDDD", "llDDD", "llDDDD"]
        .iter()
        .any(|p| shape(name, p))
}

/// Case-insensitive order that compares digit runs as numbers: `A2 < A10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let da = a.iter().take_while(|c| c.is_ascii_digit()).count();
                let db = b.iter().take_while(|c| c.is_ascii_digit()).count();
                let na = trim_zeros(&a[..da]);
                let nb = trim_zeros(&b[..db]);
                let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[da..];
                b = &b[db..];
            }
            (Some(x), Some(y)) => {
                let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[1..];
                b = &b[1..];
            }
        }
    }
}

fn trim_zeros(d: &[u8]) -> &[u8] {
    let z = d.iter().take_while(|&&c| c == b'0').count();
    &d[z..]
}

// ---- evidence ----

#[derive(Debug, Clone)]
struct Claim {
    value: String,
    source: IdentitySource,
}

#[derive(Debug, Clone)]
struct Evidence {
    index: usize,
    camera: Claim,
    reel: Claim,
    embedded_reel: Option<String>,
}

fn rank(s: IdentitySource) -> u8 {
    match s {
        IdentitySource::Metadata => 0,
        IdentitySource::FileName => 1,
        IdentitySource::Folder => 2,
        IdentitySource::FolderName => 3,
        IdentitySource::Card => 4,
    }
}

fn evidence(
    root: &Path,
    dir: &Path,
    index: usize,
    files: &ClipFiles,
    meta: Option<&ClipMeta>,
) -> Evidence {
    let claim = |v: &str, source| Claim {
        value: v.to_string(),
        source,
    };
    let embedded = |f: fn(&ClipMeta) -> &Option<String>| {
        meta.and_then(|m| f(m).as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let embedded_camera = embedded(|m| &m.embedded_camera_id);
    let embedded_reel = embedded(|m| &m.embedded_reel);

    let stem = files
        .main
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let named = identity_from_file_name(&stem);
    let folder = folder_evidence(root, dir);

    let camera = embedded_camera
        .as_deref()
        .map(|v| claim(v, IdentitySource::Metadata))
        .or_else(|| {
            named
                .as_ref()
                .map(|n| claim(&n.camera, IdentitySource::FileName))
        })
        .or_else(|| {
            folder
                .camera
                .as_deref()
                .map(|v| claim(v, IdentitySource::Folder))
        })
        .unwrap_or_else(|| claim(&folder.name, IdentitySource::FolderName));
    let reel = embedded_reel
        .as_deref()
        .map(|v| claim(v, IdentitySource::Metadata))
        .or_else(|| {
            named
                .as_ref()
                .map(|n| claim(&n.reel, IdentitySource::FileName))
        })
        .or_else(|| {
            folder
                .reel
                .as_deref()
                .map(|v| claim(v, IdentitySource::Folder))
        })
        .unwrap_or_else(|| claim(&folder.name, IdentitySource::FolderName));
    Evidence {
        index,
        camera,
        reel,
        embedded_reel,
    }
}

struct FolderEvidence {
    /// The card folder's own name (the root's display path when it has none).
    name: String,
    camera: Option<String>,
    reel: Option<String>,
}

/// Step 3: the card folder names the reel when it looks like one; a camera
/// folder (`A_CAM`) names the camera, whether it holds the clips or the card
/// folder; any other parent inside the root names the camera of a reel folder.
fn folder_evidence(root: &Path, dir: &Path) -> FolderEvidence {
    let file_name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    let name = file_name(dir).unwrap_or_else(|| dir.display().to_string());
    let reel = looks_like_reel(&name).then(|| name.clone());
    let parent = dir.parent();
    let parent_name = parent.and_then(file_name);
    let parent_inside = dir != root && parent.is_some_and(|p| p.starts_with(root));

    let camera = camera_from_folder_name(&name).or_else(|| {
        let parent_name = parent_name.as_deref()?;
        camera_from_folder_name(parent_name)
            .or_else(|| (reel.is_some() && parent_inside).then(|| parent_name.to_string()))
    });
    FolderEvidence { name, camera, reel }
}

// ---- cards ----

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// One bucket per distinct embedded reel when the folder holds more than one;
/// clips without an embedded reel join the bucket their own reel names, and
/// otherwise form a bucket of their own.
fn split_by_embedded_reel(evs: Vec<Evidence>) -> Vec<Vec<Evidence>> {
    let mut keys: Vec<String> = Vec::new();
    for e in &evs {
        if let Some(r) = &e.embedded_reel
            && !keys.iter().any(|k| same(k, r))
        {
            keys.push(r.clone());
        }
    }
    if keys.len() < 2 {
        return vec![evs];
    }
    let mut buckets: Vec<Vec<Evidence>> = vec![Vec::new(); keys.len() + 1];
    for e in evs {
        let reel = e.embedded_reel.as_deref().unwrap_or(&e.reel.value);
        let slot = keys
            .iter()
            .position(|k| same(k, reel))
            .unwrap_or(keys.len());
        buckets[slot].push(e);
    }
    buckets.retain(|b| !b.is_empty());
    buckets
}

/// The strongest claim; among equally strong ones the most common value, then
/// the first in natural order.
fn strongest<'a>(claims: impl Iterator<Item = &'a Claim>) -> Claim {
    let claims: Vec<&Claim> = claims.collect();
    let best = claims
        .iter()
        .map(|c| rank(c.source))
        .min()
        .unwrap_or(u8::MAX);
    let mut counts: HashMap<String, (usize, &Claim)> = HashMap::new();
    for c in claims.iter().filter(|c| rank(c.source) == best) {
        counts.entry(c.value.to_lowercase()).or_insert((0, c)).0 += 1;
    }
    let mut ranked: Vec<(usize, &Claim)> = counts.into_values().collect();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| natural_cmp(&a.1.value, &b.1.value))
    });
    ranked.first().map(|(_, c)| (*c).clone()).unwrap_or(Claim {
        value: String::new(),
        source: IdentitySource::FolderName,
    })
}

/// The clip's own source when its own evidence agrees with the card, else
/// [`IdentitySource::Card`].
fn own_or_card(own: &Claim, card: &Claim) -> IdentitySource {
    if same(&own.value, &card.value) {
        own.source
    } else {
        IdentitySource::Card
    }
}

fn disambiguate_reels(cards: &mut [CardGroup]) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for c in cards.iter() {
        *seen.entry(c.reel.to_lowercase()).or_default() += 1;
    }
    for c in cards.iter_mut() {
        if seen[&c.reel.to_lowercase()] > 1 {
            c.reel = format!("{} ({})", c.reel, c.folder);
        }
    }
}

fn camera_key(a: &str, b: &str) -> Ordering {
    let first = |s: &str| s.chars().next().map(|c| c.to_ascii_uppercase());
    first(a)
        .cmp(&first(b))
        .then_with(|| natural_cmp(a, b))
        .then_with(|| a.cmp(b))
}

/// Recording date, then start timecode, then file name; unknowns last.
fn clip_order(clips: &[(ClipFiles, Option<&ClipMeta>)], a: &Evidence, b: &Evidence) -> Ordering {
    let key = |e: &Evidence| {
        let (files, meta) = &clips[e.index];
        let date = meta.and_then(|m| m.recording_date.clone());
        let tc = meta
            .and_then(|m| m.start_timecode)
            .map(|t| t.frames as u128 * 1_000_000 / u128::from(t.base.max(1)));
        let name = files
            .main
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (date, tc, name)
    };
    let (da, ta, na) = key(a);
    let (db, tb, nb) = key(b);
    none_last(&da, &db)
        .then_with(|| none_last(&ta, &tb))
        .then_with(|| natural_cmp(&na, &nb))
        .then_with(|| na.cmp(&nb))
}

fn none_last<T: Ord>(a: &Option<T>, b: &Option<T>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

// ---- shape matching ----

/// `pattern`: `L` uppercase letter, `l` any letter, `D` digit, `X` uppercase
/// letter or digit, `x` any letter or digit, anything else literal.
fn shape(s: &str, pattern: &str) -> bool {
    s.len() == pattern.len() && prefix_matches(s, pattern)
}

/// `s` starts with `pattern` and the next character (if any) is not alphanumeric.
fn starts_shape(s: &str, pattern: &str) -> bool {
    s.len() >= pattern.len()
        && prefix_matches(s, pattern)
        && !s
            .as_bytes()
            .get(pattern.len())
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn prefix_matches(s: &str, pattern: &str) -> bool {
    s.bytes().zip(pattern.bytes()).all(|(c, p)| match p {
        b'L' => c.is_ascii_uppercase(),
        b'l' => c.is_ascii_alphabetic(),
        b'D' => c.is_ascii_digit(),
        b'X' => c.is_ascii_uppercase() || c.is_ascii_digit(),
        b'x' => c.is_ascii_alphanumeric(),
        lit => c == lit,
    })
}

#[cfg(test)]
mod tests;
