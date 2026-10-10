//! Synthetic folder trees for tests: each file gets content that sniffs as
//! what its extension suggests, so tests read like a card listing.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub fn tree(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for rel in files {
        write(dir.path(), rel);
    }
    dir
}

pub fn write(root: &Path, rel: &str) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content(rel)).unwrap();
    path
}

fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

pub fn content(rel: &str) -> Vec<u8> {
    let lower = rel.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "mxf" => {
            let mut v = vec![
                0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01,
                0x02, 0x04, 0x00,
            ];
            v.extend_from_slice(&[0; 32]);
            v
        }
        "mov" | "mp4" => atom(b"ftyp", b"qt  \0\0\0\0qt  "),
        "crm" => atom(b"ftyp", b"crx \0\0\0\x01crx isom"),
        "braw" => [atom(b"wide", &[]), atom(b"mdat", &[0; 8])].concat(),
        "r3d" => {
            let mut v = vec![0, 0, 0x0F, 0x4F];
            v.extend_from_slice(b"RED2");
            v.extend_from_slice(&[0; 16]);
            v
        }
        "ari" | "arx" => {
            let mut v = b"ARRI".to_vec();
            v.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0, 0, 0, 0]);
            v
        }
        "xml" if lower.ends_with("m01.xml") => {
            br#"<?xml version="1.0"?><NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">"#.to_vec()
        }
        "xml" => br#"<?xml version="1.0"?><VideoClip xmlns="http://www.canon.com/ns/RawMovieClip">"#
            .to_vec(),
        "rmd" => br#"<?xml version="1.0"?><red_metadata>"#.to_vec(),
        "sidecar" => b"{ \"iso\": 800 }".to_vec(),
        "ale" => b"Heading\nFIELD_DELIM\tTABS\n".to_vec(),
        _ => b"not a camera file".to_vec(),
    }
}
