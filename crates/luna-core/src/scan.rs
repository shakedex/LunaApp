//! Walk the source root, classify every file, and assemble clips.
//!
//! Every file found ends up in exactly one place: a clip (main, segment or
//! sidecar), [`Scan::not_included`], or [`Scan::unreadable`]. OS clutter is
//! skipped and not reported. Symlinks and junctions are never followed; they are
//! listed as not included. The walk only reads.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};

use luna_formats::{ClipFiles, Container, SidecarKind, Sniffed};
use luna_report::{Unreadable, UnreadableKind};

/// What a walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// Assembled clips, sorted by main file path.
    pub clips: Vec<ClipFiles>,
    /// Files that are neither part of a clip nor unreadable, relative to the
    /// root with `/` separators, sorted.
    pub not_included: Vec<String>,
    /// Folders and files the walk could not read, sorted by path.
    pub unreadable: Vec<Unreadable>,
}

impl Scan {
    /// Logs what the walk found: each clip with its segments and sidecars,
    /// then every file left out and every path that couldn't be read.
    pub fn log(&self, roots: &[PathBuf]) {
        let show = |p: &Path| display_path(roots, p);
        for clip in &self.clips {
            let segments: Vec<_> = clip.segments.iter().map(|p| show(p)).collect();
            let sidecars: Vec<_> = clip
                .sidecars
                .iter()
                .map(|(kind, p)| format!("{kind:?} {}", show(p)))
                .collect();
            tracing::debug!(
                container = ?clip.container,
                segments = ?segments,
                sidecars = ?sidecars,
                "scan: {}",
                show(&clip.main)
            );
        }
        for path in &self.not_included {
            tracing::debug!("not included: {path} (not part of a clip)");
        }
        for u in &self.unreadable {
            tracing::debug!(kind = ?u.kind, "unreadable: {}: {}", u.path, u.reason);
        }
    }
}

/// Folder and file names an OS leaves behind. Matched case-insensitively.
const CLUTTER: [&str; 9] = [
    ".trashes",
    ".spotlight-v100",
    ".fseventsd",
    ".temporaryitems",
    "system volume information",
    "$recycle.bin",
    ".ds_store",
    "thumbs.db",
    "desktop.ini",
];

/// True for OS clutter: Spotlight/Trash/recycle folders, Finder and Explorer
/// droppings, and `._*` AppleDouble files.
pub fn is_clutter(name: &str) -> bool {
    name.starts_with("._") || CLUTTER.iter().any(|c| name.eq_ignore_ascii_case(c))
}

/// `path` relative to `root`, `/`-separated; `"."` for the root itself.
pub fn relative(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<_> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// The given roots without duplicates and without any root that sits inside
/// another one (its files are already part of the outer walk). Order kept.
pub fn outermost(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .enumerate()
        .filter(|&(i, r)| {
            !roots
                .iter()
                .enumerate()
                .any(|(j, o)| (o != r && r.starts_with(o)) || (o == r && j < i))
        })
        .map(|(_, r)| r.clone())
        .collect()
}

/// The root `path` lives in; the longest when several contain it.
pub fn containing_root<'a>(roots: &'a [PathBuf], path: &Path) -> Option<&'a PathBuf> {
    roots
        .iter()
        .filter(|r| path.starts_with(r))
        .max_by_key(|r| r.components().count())
}

/// `path` for display: relative to the root with one root, and prefixed with
/// its root (`E:/A_CAM/A001`) when there are several, so paths from different
/// drives stay apart.
pub fn display_path(roots: &[PathBuf], path: &Path) -> String {
    let roots = outermost(roots);
    match containing_root(&roots, path) {
        Some(root) if roots.len() == 1 => relative(root, path),
        Some(root) => join_display(root, &relative(root, path)),
        None => path.to_string_lossy().replace('\\', "/"),
    }
}

fn join_display(root: &Path, rel: &str) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let root = match root.trim_end_matches('/') {
        "" => "/",
        r => r,
    };
    match rel {
        "." => root.to_owned(),
        rel if root.ends_with('/') => format!("{root}{rel}"),
        rel => format!("{root}/{rel}"),
    }
}

/// Walks every root (see [`outermost`]) and merges what they found. With one
/// root this is [`scan`]; with several, `not_included` and `unreadable` paths
/// carry their root (see [`display_path`]).
pub fn scan_roots(roots: &[PathBuf]) -> Scan {
    let roots = outermost(roots);
    if let [root] = roots.as_slice() {
        return scan(root);
    }
    let mut out = Scan::default();
    for root in &roots {
        let s = scan(root);
        out.clips.extend(s.clips);
        out.not_included.extend(
            s.not_included
                .into_iter()
                .map(|rel| join_display(root, &rel)),
        );
        out.unreadable.extend(s.unreadable.into_iter().map(|mut u| {
            u.path = join_display(root, &u.path);
            u
        }));
    }
    out.clips.sort_by(|a, b| a.main.cmp(&b.main));
    out.not_included.sort();
    out.unreadable.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Walks `root` recursively and assembles clips.
pub fn scan(root: &Path) -> Scan {
    let mut out = Scan::default();
    let mut by_folder: BTreeMap<PathBuf, Vec<(PathBuf, Sniffed)>> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                out.unreadable
                    .push(unreadable(root, &dir, UnreadableKind::Folder, &e));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    out.unreadable
                        .push(unreadable(root, &dir, UnreadableKind::Folder, &e));
                    continue;
                }
            };
            let path = entry.path();
            if is_clutter(&entry.file_name().to_string_lossy()) {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    out.unreadable
                        .push(unreadable(root, &path, UnreadableKind::File, &e));
                    continue;
                }
            };
            // `is_symlink` is true for Windows junctions too; never follow either.
            if file_type.is_symlink() {
                out.not_included.push(relative(root, &path));
            } else if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                match sniff(&path) {
                    Ok(s) => by_folder.entry(dir.clone()).or_default().push((path, s)),
                    Err(e) => {
                        out.unreadable
                            .push(unreadable(root, &path, UnreadableKind::File, &e))
                    }
                }
            } else {
                out.not_included.push(relative(root, &path));
            }
        }
    }

    for files in by_folder.into_values() {
        let (clips, rest) = assemble(files);
        out.clips.extend(clips);
        out.not_included
            .extend(rest.iter().map(|p| relative(root, p)));
    }
    out.clips.sort_by(|a, b| a.main.cmp(&b.main));
    out.not_included.sort();
    out.unreadable.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn unreadable(root: &Path, path: &Path, kind: UnreadableKind, e: &io::Error) -> Unreadable {
    Unreadable {
        path: relative(root, path),
        kind,
        reason: reason(e),
    }
}

fn reason(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::PermissionDenied => "permission denied".into(),
        io::ErrorKind::NotFound => "not found".into(),
        _ => e.to_string(),
    }
}

/// Assembles the files of one folder into clips; returns the clips and the
/// files that belong to none of them.
fn assemble(mut files: Vec<(PathBuf, Sniffed)>) -> (Vec<ClipFiles>, Vec<PathBuf>) {
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut clips: Vec<ClipFiles> = Vec::new();
    let mut rest = Vec::new();
    let mut sidecars = Vec::new();
    // Spanned clips, keyed by (container, base name, extension, digit count).
    let mut spans: HashMap<(Container, String, String, usize), usize> = HashMap::new();
    let mut span_parts: Vec<Vec<(u64, PathBuf)>> = Vec::new();

    for (path, sniffed) in files {
        match sniffed {
            Sniffed::CameraOriginal(container) => {
                let numbered = match container {
                    Container::R3d => r3d_segment(&path),
                    Container::AriFrame => ari_frame(&path),
                    _ => None,
                };
                match numbered {
                    Some((base, digits, n)) => {
                        let ext = extension(&path);
                        let key = (container, base.to_lowercase(), ext, digits);
                        let slot = *spans.entry(key).or_insert_with(|| {
                            clips.push(ClipFiles {
                                container,
                                main: PathBuf::new(),
                                segments: Vec::new(),
                                sidecars: Vec::new(),
                            });
                            span_parts.push(Vec::new());
                            span_parts.len() - 1
                        });
                        span_parts[slot].push((n, path));
                    }
                    None => clips.push(ClipFiles {
                        container,
                        main: path,
                        segments: Vec::new(),
                        sidecars: Vec::new(),
                    }),
                }
            }
            Sniffed::Sidecar(kind) => sidecars.push((kind, path)),
            Sniffed::Other => rest.push(path),
        }
    }

    // Fill the spanned clips, in the order they were created.
    let mut parts = span_parts.into_iter();
    for clip in clips.iter_mut().filter(|c| c.main.as_os_str().is_empty()) {
        let mut segs = parts.next().unwrap_or_default();
        segs.sort();
        let mut segs = segs.into_iter().map(|(_, p)| p);
        clip.main = segs.next().unwrap_or_default();
        clip.segments = segs.collect();
    }

    for (kind, path) in sidecars {
        match attach_to(&clips, kind, &path) {
            Some(i) => clips[i].sidecars.push((kind, path)),
            None => rest.push(path),
        }
    }
    (clips, rest)
}

/// Which clip a sidecar belongs to, by name. ALEs describe a whole card, not
/// one clip, so they are never attached.
fn attach_to(clips: &[ClipFiles], kind: SidecarKind, sidecar: &Path) -> Option<usize> {
    let stem = stem(sidecar).to_lowercase();
    let wanted: Vec<String> = match kind {
        SidecarKind::Ale => return None,
        // `<clip>M01.XML`
        SidecarKind::SonyXml => {
            let b = stem.as_bytes();
            let n = b.len();
            if n > 3 && b[n - 3] == b'm' && b[n - 2..].iter().all(u8::is_ascii_digit) {
                vec![stem[..n - 3].to_string()]
            } else {
                vec![stem]
            }
        }
        SidecarKind::CanonXml | SidecarKind::RedRmd | SidecarKind::BrawSidecar => vec![stem],
    };
    clips
        .iter()
        .position(|c| clip_names(c).iter().any(|n| wanted.contains(n)))
}

/// The names a sidecar may use for a clip: the main file's stem, and for a
/// spanned clip the base name without the segment or frame number.
fn clip_names(clip: &ClipFiles) -> Vec<String> {
    let mut names = vec![stem(&clip.main).to_lowercase()];
    let base = match clip.container {
        Container::R3d => r3d_segment(&clip.main).map(|(b, ..)| b),
        Container::AriFrame => {
            ari_frame(&clip.main).map(|(b, ..)| b.trim_end_matches(['.', '_', '-']).to_string())
        }
        _ => None,
    };
    names.extend(base.map(|b| b.to_lowercase()));
    names
}

/// `NAME_001.R3D` → ("NAME", 3, 1).
fn r3d_segment(path: &Path) -> Option<(String, usize, u64)> {
    let stem = stem(path);
    let (base, num) = stem.rsplit_once('_')?;
    (num.len() == 3 && num.bytes().all(|b| b.is_ascii_digit()) && !base.is_empty())
        .then(|| (base.to_string(), 3, num.parse().unwrap_or(0)))
}

/// `A001C001_120101_R1AB.0000001.ari` → ("A001C001_120101_R1AB.", 7, 1).
fn ari_frame(path: &Path) -> Option<(String, usize, u64)> {
    let stem = stem(path);
    let digits = stem.bytes().rev().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let (base, num) = stem.split_at(stem.len() - digits);
    Some((base.to_string(), digits, num.parse().unwrap_or(0)))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn read_dir(dir: &Path) -> io::Result<std::fs::ReadDir> {
    #[cfg(test)]
    hook::check(dir)?;
    std::fs::read_dir(dir)
}

fn sniff(path: &Path) -> io::Result<Sniffed> {
    #[cfg(test)]
    hook::check(path)?;
    match luna_formats::sniff_path(path)? {
        // One sound track of an OP-Atom clip: its picture is a file of its own.
        Sniffed::CameraOriginal(Container::Mxf) if luna_formats::is_sound_only(path) => {
            Ok(Sniffed::Other)
        }
        s => Ok(s),
    }
}

/// Test-only stand-in for OS permissions, which are awkward to set up on Windows.
#[cfg(test)]
pub(crate) mod hook {
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};

    thread_local! {
        static DENIED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    }

    pub fn deny(path: &Path) {
        DENIED.with(|d| d.borrow_mut().push(path.to_path_buf()));
    }

    pub fn check(path: &Path) -> io::Result<()> {
        DENIED.with(|d| {
            if d.borrow().iter().any(|p| p == path) {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                Ok(())
            }
        })
    }
}

#[cfg(test)]
mod tests;
