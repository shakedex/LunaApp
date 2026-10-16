//! Content-based file classification. The extension is only a tie-breaker.

use std::path::Path;

use crate::types::{Container, SidecarKind, Sniffed};

/// How many leading bytes [`sniff`] wants.
pub const SNIFF_LEN: usize = 4096;

/// SMPTE 377-1 header partition pack key, up to (and including) the byte that
/// says "header partition". The last two bytes (open/closed, complete) vary.
const MXF_HEADER_PARTITION: [u8; 14] = [
    0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02,
];

/// Top-level atoms a QuickTime / ISO-BMFF file can start with.
const LEADING_ATOMS: [&[u8; 4]; 7] = [
    b"ftyp", b"moov", b"wide", b"mdat", b"free", b"skip", b"pnot",
];

/// ISO-BMFF major brands that are stills or audio, never camera originals.
const NOT_MOVIE_BRANDS: [&[u8; 4]; 13] = [
    b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"msf1", b"avif", b"avis",
    b"M4A ", b"M4B ", b"M4P ",
];

/// Classifies a file from its first bytes (up to [`SNIFF_LEN`]).
pub fn sniff(path: &Path, head: &[u8]) -> Sniffed {
    let head = &head[..head.len().min(SNIFF_LEN)];
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if head.len() >= 8 && &head[..4] == b"ARRI" && head[4..8] == [0x12, 0x34, 0x56, 0x78] {
        return Sniffed::CameraOriginal(Container::AriFrame);
    }
    if head.len() >= 8 && (&head[4..8] == b"RED1" || &head[4..8] == b"RED2") {
        return Sniffed::CameraOriginal(Container::R3d);
    }
    if let Some(c) = quicktime(head, &ext) {
        return c;
    }
    if head
        .windows(MXF_HEADER_PARTITION.len())
        .any(|w| w == MXF_HEADER_PARTITION)
    {
        return Sniffed::CameraOriginal(Container::Mxf);
    }
    text(head, &ext).map_or(Sniffed::Other, Sniffed::Sidecar)
}

/// `Some` when the head is an atom structure: a camera original, or `Other`
/// for stills and audio that share the structure.
fn quicktime(head: &[u8], ext: &str) -> Option<Sniffed> {
    if head.len() < 8 || !LEADING_ATOMS.iter().any(|a| &head[4..8] == *a) {
        return None;
    }
    let size = u32::from_be_bytes(head[..4].try_into().ok()?) as usize;
    let size = match size {
        // Runs to the end of the file.
        0 => head.len(),
        // 64-bit size follows the type.
        1 => {
            let big = u64::from_be_bytes(head.get(8..16)?.try_into().ok()?);
            if big < 16 {
                return None;
            }
            usize::try_from(big).unwrap_or(usize::MAX)
        }
        n if n < 8 => return None,
        n => n,
    };
    let mut brands: Vec<&[u8]> = Vec::new();
    if &head[4..8] == b"ftyp" {
        let end = size.min(head.len());
        if end >= 12 {
            brands.push(&head[8..12]);
        }
        let mut at = 16;
        while at + 4 <= end {
            brands.push(&head[at..at + 4]);
            at += 4;
        }
    }
    let original = |c| Some(Sniffed::CameraOriginal(c));
    if brands.iter().any(|b| *b == b"crx ") {
        // Canon CR3 stills carry the same brand.
        return if ext == "cr3" {
            Some(Sniffed::Other)
        } else {
            original(Container::Crm)
        };
    }
    // Nikon N-RAW. Nikon's MP4s list `niko` only as a compatible brand.
    if brands.first().is_some_and(|major| *major == b"niko") {
        return original(Container::Nev);
    }
    if brands
        .first()
        .is_some_and(|major| NOT_MOVIE_BRANDS.iter().any(|b| major == b))
    {
        return Some(Sniffed::Other);
    }
    // BRAW's `braw` sample entry sits beyond the head; the extension breaks the tie.
    if ext == "braw" || brands.iter().any(|b| *b == b"braw") {
        return original(Container::Braw);
    }
    original(Container::QuickTime)
}

/// Text sidecars: XML by root element, BRAW JSON, ALE.
fn text(head: &[u8], ext: &str) -> Option<SidecarKind> {
    let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    let text = String::from_utf8_lossy(head);
    let body = text.trim_start();
    if body.starts_with('<') {
        return xml(body, ext);
    }
    if body.starts_with('{') {
        return (ext == "sidecar").then_some(SidecarKind::BrawSidecar);
    }
    let mut lines = body.lines().map(str::trim_end);
    if lines.next() == Some("Heading") && lines.any(|l| l.starts_with("FIELD_DELIM")) {
        return Some(SidecarKind::Ale);
    }
    None
}

fn xml(body: &str, ext: &str) -> Option<SidecarKind> {
    let rmd = (ext == "rmd").then_some(SidecarKind::RedRmd);
    let Some(tag) = root_tag(body) else {
        return rmd;
    };
    let name: String = tag
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
        .collect();
    let local = name.rsplit(':').next().unwrap_or(&name);
    let squashed = local.to_ascii_lowercase().replace('_', "");
    if local == "NonRealTimeMeta" {
        Some(SidecarKind::SonyXml)
    } else if tag.to_ascii_lowercase().contains("canon.com/ns") {
        Some(SidecarKind::CanonXml)
    } else if squashed.starts_with("redmetadata") {
        Some(SidecarKind::RedRmd)
    } else {
        rmd
    }
}

/// The root element's start tag (without the leading `<`), skipping the XML
/// declaration, processing instructions, comments and the doctype.
fn root_tag(body: &str) -> Option<&str> {
    let mut rest = body;
    loop {
        let at = rest.find('<')?;
        rest = &rest[at + 1..];
        if let Some(r) = rest.strip_prefix('?') {
            rest = &r[r.find("?>")? + 2..];
        } else if let Some(r) = rest.strip_prefix("!--") {
            rest = &r[r.find("-->")? + 3..];
        } else if let Some(r) = rest.strip_prefix('!') {
            rest = &r[r.find('>')? + 1..];
        } else {
            let end = rest.find('>').unwrap_or(rest.len());
            return Some(&rest[..end]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Container, SidecarKind};

    const MXF_KEY: [u8; 16] = [
        0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02, 0x04,
        0x00,
    ];

    fn s(name: &str, head: &[u8]) -> Sniffed {
        sniff(Path::new(name), head)
    }

    fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn ftyp(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
        let mut body = major.to_vec();
        body.extend_from_slice(&[0, 0, 0, 1]);
        for c in compatible {
            body.extend_from_slice(*c);
        }
        atom(b"ftyp", &body)
    }

    #[test]
    fn mxf_header_partition_at_start() {
        let mut head = MXF_KEY.to_vec();
        head.extend_from_slice(&[0x83, 0, 0, 0x98]);
        assert_eq!(s("x.bin", &head), Sniffed::CameraOriginal(Container::Mxf));
    }

    #[test]
    fn mxf_with_run_in() {
        let mut head = vec![0xAAu8; 300];
        head.extend_from_slice(&MXF_KEY);
        assert_eq!(s("x.mxf", &head), Sniffed::CameraOriginal(Container::Mxf));
    }

    #[test]
    fn mxf_body_partition_is_not_a_file_start() {
        // A body partition key (byte 13 = 0x03) is not a header partition.
        let mut key = MXF_KEY;
        key[13] = 0x03;
        assert_eq!(s("x.mxf", &key), Sniffed::Other);
    }

    #[test]
    fn quicktime_family() {
        let mov = ftyp(b"qt  ", &[b"qt  "]);
        assert_eq!(
            s("a.mov", &mov),
            Sniffed::CameraOriginal(Container::QuickTime)
        );
        let xf = ftyp(b"XFVC", &[b"XFVC", b"mp42"]);
        assert_eq!(
            s("a.MP4", &xf),
            Sniffed::CameraOriginal(Container::QuickTime)
        );
        let wide = [atom(b"wide", &[]), atom(b"mdat", &[1, 2, 3])].concat();
        assert_eq!(
            s("a.mov", &wide),
            Sniffed::CameraOriginal(Container::QuickTime)
        );
        let moov = atom(b"moov", &[0; 16]);
        assert_eq!(
            s("noext", &moov),
            Sniffed::CameraOriginal(Container::QuickTime)
        );
    }

    #[test]
    fn crm_by_brand_not_extension() {
        let crm = ftyp(b"crx ", &[b"crx ", b"isom"]);
        assert_eq!(s("A.CRM", &crm), Sniffed::CameraOriginal(Container::Crm));
        assert_eq!(
            s("renamed.mov", &crm),
            Sniffed::CameraOriginal(Container::Crm)
        );
        // A CR3 still shares the brand; the extension breaks the tie.
        assert_eq!(s("IMG_0001.CR3", &crm), Sniffed::Other);
    }

    #[test]
    fn braw_is_quicktime_structure_with_braw_extension() {
        let head = [atom(b"wide", &[]), atom(b"mdat", &[0; 8])].concat();
        assert_eq!(
            s("D001.braw", &head),
            Sniffed::CameraOriginal(Container::Braw)
        );
        assert_eq!(
            s("D001.BRAW", &head),
            Sniffed::CameraOriginal(Container::Braw)
        );
        // The extension alone is not enough.
        assert_eq!(s("fake.braw", b"hello"), Sniffed::Other);
    }

    #[test]
    fn iso_bmff_stills_and_audio_are_not_camera_originals() {
        assert_eq!(s("a.heic", &ftyp(b"heic", &[b"mif1"])), Sniffed::Other);
        assert_eq!(s("a.m4a", &ftyp(b"M4A ", &[b"isom"])), Sniffed::Other);
        assert_eq!(s("a.avif", &ftyp(b"avif", &[b"mif1"])), Sniffed::Other);
    }

    #[test]
    fn nikon_nev_by_major_brand() {
        assert_eq!(
            s("DSC_0001.NEV", &ftyp(b"niko", &[b"niko"])),
            Sniffed::CameraOriginal(Container::Nev)
        );
        // A Nikon H.264 MP4 is a plain QuickTime clip.
        assert_eq!(
            s("DSC_0001.MP4", &ftyp(b"mp42", &[b"mp42", b"avc1", b"niko"])),
            Sniffed::CameraOriginal(Container::QuickTime)
        );
    }

    #[test]
    fn r3d_magic() {
        let mut head = vec![0, 0, 0x0F, 0x4F];
        head.extend_from_slice(b"RED2");
        head.extend_from_slice(&[0; 20]);
        assert_eq!(
            s("A_001.R3D", &head),
            Sniffed::CameraOriginal(Container::R3d)
        );
        head[4..8].copy_from_slice(b"RED1");
        assert_eq!(s("x", &head), Sniffed::CameraOriginal(Container::R3d));
    }

    #[test]
    fn ari_frame_magic() {
        let mut head = b"ARRI".to_vec();
        head.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0, 0, 0, 0]);
        assert_eq!(
            s("f.0000001.ari", &head),
            Sniffed::CameraOriginal(Container::AriFrame)
        );
        // ARRI in a QuickTime ftyp's compatible brands is not an .ari frame.
        let mov = ftyp(b"qt  ", &[b"qt  ", b"ARRI"]);
        assert_eq!(
            s("M.mov", &mov),
            Sniffed::CameraOriginal(Container::QuickTime)
        );
    }

    #[test]
    fn sony_xml() {
        let x = br#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<NonRealTimeMeta lastUpdate="2020" xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">"#;
        assert_eq!(
            s("B005C039M01.XML", x),
            Sniffed::Sidecar(SidecarKind::SonyXml)
        );
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(x);
        assert_eq!(s("a.xml", &bom), Sniffed::Sidecar(SidecarKind::SonyXml));
    }

    #[test]
    fn canon_xml() {
        let x = br#"<?xml version="1.0" encoding="UTF-8"?>
<VideoClip xmlns="http://www.canon.com/ns/RawMovieClip"><ClipType>MovieType2</ClipType>"#;
        assert_eq!(
            s("S_0001C038.XML", x),
            Sniffed::Sidecar(SidecarKind::CanonXml)
        );
        let other = br#"<?xml version="1.0"?><!-- c --><VideoClip xmlns="urn:other">"#;
        assert_eq!(s("x.xml", other), Sniffed::Other);
    }

    #[test]
    fn red_rmd() {
        let x = br#"<?xml version="1.0" encoding="UTF-8"?><red_metadata version="1">"#;
        assert_eq!(s("A005_W055.RMD", x), Sniffed::Sidecar(SidecarKind::RedRmd));
        let y = br#"<?xml version="1.0"?><RedMetadata>"#;
        assert_eq!(s("y.xml", y), Sniffed::Sidecar(SidecarKind::RedRmd));
        // Any XML in an .RMD file is RED's.
        let z = br#"<?xml version="1.0"?><clip>"#;
        assert_eq!(s("z.rmd", z), Sniffed::Sidecar(SidecarKind::RedRmd));
        assert_eq!(s("z.xml", z), Sniffed::Other);
    }

    #[test]
    fn braw_sidecar_is_json_with_sidecar_extension() {
        let j = b"  {\n  \"iso\": 800,\n  \"white_balance_kelvin\": 5600\n}";
        assert_eq!(
            s("D001_C013.sidecar", j),
            Sniffed::Sidecar(SidecarKind::BrawSidecar)
        );
        assert_eq!(s("settings.json", j), Sniffed::Other);
        assert_eq!(s("x.sidecar", b"not json"), Sniffed::Other);
    }

    #[test]
    fn ale() {
        let a = b"Heading\nFIELD_DELIM\tTABS\nVIDEO_FORMAT\tCUSTOM\n";
        assert_eq!(
            s("M001R00H_AVID.ale", a),
            Sniffed::Sidecar(SidecarKind::Ale)
        );
        let crlf = b"Heading\r\nFIELD_DELIM\tTABS\r\n";
        assert_eq!(s("x.txt", crlf), Sniffed::Sidecar(SidecarKind::Ale));
        assert_eq!(s("x.ale", b"Heading\nnothing else"), Sniffed::Other);
    }

    #[test]
    fn everything_else_is_other() {
        assert_eq!(s("empty.mxf", b""), Sniffed::Other);
        assert_eq!(s("a.pdf", b"%PDF-1.7\n"), Sniffed::Other);
        assert_eq!(s("a.jpg", &[0xFF, 0xD8, 0xFF, 0xE0]), Sniffed::Other);
        assert_eq!(s("a.txt", b"hello world"), Sniffed::Other);
        assert_eq!(
            s("a.mhl", b"<?xml version=\"1.0\"?><hashlist>"),
            Sniffed::Other
        );
        assert_eq!(
            s("bin.bin", &[0xFE, 0, 0, 0, 0x0A, 0, 0, 0, 0, 0, 0xC0, 0x41]),
            Sniffed::Other
        );
        // An atom size that can't be right is not QuickTime.
        let mut bogus = 3u32.to_be_bytes().to_vec();
        bogus.extend_from_slice(b"free");
        assert_eq!(s("x.mov", &bogus), Sniffed::Other);
    }
}
