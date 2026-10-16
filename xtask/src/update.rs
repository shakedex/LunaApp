//! Update signing and feeds: the ed25519 key that signs every update, and
//! the Sparkle-format appcast that Sparkle (macOS) and WinSparkle (Windows)
//! read.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};

/// The update file an appcast points at.
pub struct Enclosure {
    pub url: String,
    pub length: u64,
    /// Base64 ed25519 signature of the file's bytes.
    pub signature: String,
    /// WinSparkle runs the installer with these (Windows only).
    pub installer_arguments: Option<String>,
}

/// Where the private signing key lives: `LUNA_UPDATE_KEY`, or
/// `~/.luna/update-key`. It is never in a repository.
pub fn key_path() -> PathBuf {
    if let Some(p) = std::env::var_os("LUNA_UPDATE_KEY") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".luna").join("update-key")
}

pub fn load_key() -> Result<SigningKey, String> {
    load_key_from(&key_path())
}

/// Reads a key file: the base64 32-byte ed25519 seed.
pub fn load_key_from(path: &Path) -> Result<SigningKey, String> {
    let text = fs::read_to_string(path).map_err(|_| {
        format!(
            "no update signing key at {}; run `cargo xtask update-key` on a fresh machine \
             or copy it from the other build machine",
            path.display()
        )
    })?;
    let seed: [u8; 32] = STANDARD
        .decode(text.trim())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(format!("{} isn't an update signing key", path.display()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Makes a new key at `path` (never over an existing one) and returns its
/// base64 public key.
pub fn new_key(path: &Path) -> Result<String, String> {
    if path.exists() {
        return Err(format!(
            "{} already exists; one key signs every update",
            path.display()
        ));
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no randomness: {e}"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    fs::write(path, STANDARD.encode(seed) + "\n")
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    let key = SigningKey::from_bytes(&seed);
    Ok(STANDARD.encode(key.verifying_key().to_bytes()))
}

/// Base64 ed25519 signature of `bytes`, the `sparkle:edSignature` format.
pub fn sign(key: &SigningKey, bytes: &[u8]) -> String {
    STANDARD.encode(key.sign(bytes).to_bytes())
}

/// A one-item Sparkle appcast for `version`.
pub fn appcast(version: &str, notes_url: &str, enclosure: &Enclosure) -> String {
    let v = escape(version);
    let args = enclosure
        .installer_arguments
        .as_deref()
        .map(|a| format!("\n            sparkle:installerArguments=\"{}\"", escape(a)))
        .unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Luna</title>
    <item>
      <title>Luna {v}</title>
      <sparkle:version>{v}</sparkle:version>
      <sparkle:shortVersionString>{v}</sparkle:shortVersionString>
      <sparkle:releaseNotesLink>{notes}</sparkle:releaseNotesLink>
      <enclosure url="{url}" length="{length}" type="application/octet-stream"
            sparkle:edSignature="{sig}"{args}/>
    </item>
  </channel>
</rss>
"#,
        notes = escape(notes_url),
        url = escape(&enclosure.url),
        length = enclosure.length,
        sig = escape(&enclosure.signature),
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use ed25519_dalek::{Signature, SigningKey, Verifier};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("luna-xtask-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn signature_round_trips() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let sig = STANDARD.decode(sign(&key, b"luna")).unwrap();
        let sig = Signature::from_slice(&sig).unwrap();
        assert!(key.verifying_key().verify(b"luna", &sig).is_ok());
        assert!(key.verifying_key().verify(b"lunb", &sig).is_err());
    }

    fn windows_enclosure() -> Enclosure {
        Enclosure {
            url: "https://github.com/shakedex/LunaApp/releases/download/v2026.10.0/Luna-2026.10.0-windows-x64-setup.exe".into(),
            length: 26883614,
            signature: "abc=".into(),
            installer_arguments: Some("/SILENT /SP- /NOCANCEL /NORESTART".into()),
        }
    }

    #[test]
    fn appcast_has_signed_enclosure() {
        let xml = appcast(
            "2026.10.0",
            "https://github.com/shakedex/LunaApp/releases/tag/v2026.10.0",
            &windows_enclosure(),
        );
        assert!(xml.contains("<sparkle:version>2026.10.0</sparkle:version>"));
        assert!(xml.contains("<sparkle:shortVersionString>2026.10.0</sparkle:shortVersionString>"));
        assert!(xml.contains(r#"sparkle:edSignature="abc=""#));
        assert!(xml.contains(r#"length="26883614""#));
        assert!(xml.contains(r#"sparkle:installerArguments="/SILENT /SP- /NOCANCEL /NORESTART""#));
        assert!(
            xml.contains(r#"xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle""#)
        );
    }

    #[test]
    fn appcast_escapes() {
        let xml = appcast("2026.10.0", "a&b", &windows_enclosure());
        assert!(xml.contains("a&amp;b"));
        assert!(!xml.contains("a&b"));
    }

    #[test]
    fn installer_needs_key() {
        let missing = temp("nokey").join("update-key");
        let err = load_key_from(&missing).unwrap_err();
        assert!(err.contains(&missing.display().to_string()), "{err}");
    }

    #[test]
    fn new_key_round_trips_and_refuses_overwrite() {
        let path = temp("newkey").join("update-key");
        let public = new_key(&path).unwrap();
        let key = load_key_from(&path).unwrap();
        assert_eq!(STANDARD.encode(key.verifying_key().to_bytes()), public);
        assert!(new_key(&path).is_err());
    }
}
