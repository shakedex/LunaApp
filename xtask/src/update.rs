//! Update signing and feeds: the ed25519 key that signs every update, and
//! the Sparkle-format appcast that Sparkle (macOS) and WinSparkle (Windows)
//! read.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

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
            "no update signing key at {}; copy it from the build machine that made it \
             (a new key would stop every installed Luna from updating)",
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

/// The key must be the one whose public half the app ships with
/// (`packaging/update-public-key.txt`), or installed copies refuse the update.
pub fn check_public_key(key: &SigningKey, committed: &str) -> Result<(), String> {
    let ours = STANDARD.encode(key.verifying_key().to_bytes());
    if committed.trim() != ours {
        return Err(format!(
            "the update signing key at {} doesn't match packaging/update-public-key.txt; \
             copy the key from the build machine that made it",
            key_path().display()
        ));
    }
    Ok(())
}

/// One key signs every update: never make a second once one is committed.
pub fn refuse_if_committed(committed: &str) -> Result<(), String> {
    if !committed.trim().is_empty() {
        return Err(
            "packaging/update-public-key.txt already has a key; copy ~/.luna/update-key \
             from the build machine that made it instead"
                .into(),
        );
    }
    Ok(())
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

/// Checks that the feed's `sparkle:edSignature` signs `file` under the
/// committed public key, as Sparkle and WinSparkle will.
pub fn verify_feed(feed: &str, file: &[u8], public_key: &str) -> Result<(), String> {
    let sig = feed
        .split("sparkle:edSignature=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .ok_or("the feed has no sparkle:edSignature")?;
    let sig: [u8; 64] = STANDARD
        .decode(sig)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("the feed's signature isn't 64 base64 bytes")?;
    let key: [u8; 32] = STANDARD
        .decode(public_key.trim())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("the public key isn't 32 base64 bytes")?;
    VerifyingKey::from_bytes(&key)
        .map_err(|e| format!("public key: {e}"))?
        .verify(file, &Signature::from_bytes(&sig))
        .map_err(|_| "the feed's signature doesn't match the file it points at".to_string())
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
    fn key_must_match_committed_public_key() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let ours = STANDARD.encode(key.verifying_key().to_bytes());
        assert!(check_public_key(&key, &format!("{ours}\n")).is_ok());
        let other = STANDARD.encode(
            SigningKey::from_bytes(&[8u8; 32])
                .verifying_key()
                .to_bytes(),
        );
        let err = check_public_key(&key, &other).unwrap_err();
        assert!(err.contains("doesn't match"), "{err}");
    }

    #[test]
    fn no_new_key_once_one_is_committed() {
        assert!(refuse_if_committed("").is_ok());
        assert!(refuse_if_committed("\n").is_ok());
        let err =
            refuse_if_committed("Q9+DsOpRFbNAWuEGO1htLzOelw1YHDEir0UWYX/Wd5c=\n").unwrap_err();
        assert!(err.contains("copy"), "{err}");
    }

    #[test]
    fn missing_key_says_where_to_copy_it_from() {
        let missing = temp("nokey2").join("update-key");
        let err = load_key_from(&missing).unwrap_err();
        assert!(err.contains("build machine that made it"), "{err}");
        assert!(!err.contains("update-key on a fresh machine"), "{err}");
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
