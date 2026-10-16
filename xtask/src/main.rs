//! Luna's build tasks: `cargo xtask <task>` (`cargo xtask` lists them).
//!
//! The same commands serve day-to-day development (`check`), a local build to
//! try (`package`), the installers people download (`installer`), cutting a
//! version (`release`) and putting it on GitHub (`publish`). Windows builds `Luna-<version>-windows-x64-setup.exe`
//! with Inno Setup; macOS builds `Luna-<version>-macos.dmg` with Apple's tools.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod update;

const HELP: &str = "\
Luna build tasks

  cargo xtask check             fmt check, clippy and tests (before every commit)
  cargo xtask package           build the app into dist/ to try it
  cargo xtask installer         package, then build the installer into dist/
  cargo xtask release           check, set the next version, build the
                                installer, commit and tag. Versions are
                                YEAR.MONTH.N (2026.10.0, then 2026.10.1);
                                `release 2026.11.0` sets one exactly
  cargo xtask update-key        create the update signing key (once, then copy
                                ~/.luna/update-key to the other build machine)
  cargo xtask publish           push the release tag and upload this machine's
                                installer to its GitHub release

Inputs (environment variables, each with a default):
  BRAW_SDK_DIR         Blackmagic RAW SDK install
  LUNA_R3D_REPO        r3d-sdk checkout (default: ../decoders/r3d-sdk)
  ISCC                 Inno Setup compiler, Windows (default: Program Files)
  LUNA_SIGN_IDENTITY   macOS codesign identity (default: ad-hoc)
";

type Result<T = ()> = std::result::Result<T, String>;

const RELEASES: &str = "https://github.com/shakedex/LunaApp/releases";

/// WinSparkle (Windows updates), pinned to the release Luna was tested with.
const WINSPARKLE_URL: &str =
    "https://github.com/vslavik/winsparkle/releases/download/v0.9.4/WinSparkle-0.9.4.zip";
const WINSPARKLE_SHA256: &str = "6037df37fc263bd1650a1c4949681a9d40ffe991d01f35892a406cb5d103c976";
const WINSPARKLE_DIR: &str = "WinSparkle-0.9.4";

/// How WinSparkle runs the Inno Setup installer for an update.
const WINDOWS_UPDATE_ARGS: &str = "/SILENT /SP- /NOCANCEL /NORESTART";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => check(),
        Some("package") => package().map(|_| ()),
        Some("installer") => installer().map(|_| ()),
        Some("publish") => publish(),
        Some("update-key") => update_key(),
        Some("release") => release(args.get(1).map(String::as_str)),
        None | Some("help" | "-h" | "--help") => {
            print!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("unknown task `{other}`\n\n{HELP}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nerror: {e}");
            ExitCode::FAILURE
        }
    }
}

// ---- tasks ----

fn check() -> Result {
    cargo(&["fmt", "--all", "--", "--check"])?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["test", "--workspace"])
}

/// Builds Luna and the R3D add-on and assembles the app in `dist/`. Returns
/// the app's path (`dist/Luna` folder or `dist/Luna.app`).
fn package() -> Result<PathBuf> {
    let root = root();
    let version = version()?;
    let braw = braw_libraries()?;
    let r3d = r3d_repo()?;
    let r3d_sdk = require(r3d.join("reference"))?;

    step(&format!("Building Luna {version}"));
    cargo(&["build", "--release", "-p", "luna-app"])?;
    step("Building the R3D add-on");
    run(Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&r3d)
        .env("R3D_SDK_DIR", &r3d_sdk))?;

    let dist = root.join("dist");
    fs::create_dir_all(&dist).map_err(|e| format!("create {}: {e}", dist.display()))?;
    let licences = licences_text(&root)?;
    step("Assembling");
    let app = if cfg!(target_os = "macos") {
        assemble_macos(&root, &dist, &version, &braw, &r3d, &r3d_sdk, &licences)?
    } else {
        assemble_windows(&root, &dist, &braw, &r3d, &r3d_sdk, &licences)?
    };
    println!("Packaged: {}", app.display());
    Ok(app)
}

fn installer() -> Result<PathBuf> {
    // Before any build: an installer without a signed feed can't update.
    let key = update::load_key()?;
    let app = package()?;
    let root = root();
    let version = version()?;
    let dist = root.join("dist");
    step("Building the installer");
    let out = if cfg!(target_os = "macos") {
        dmg(&app, &dist, &version)?
    } else {
        let out = dist.join(format!("Luna-{version}-windows-x64-setup.exe"));
        remove(&out)?;
        run(Command::new(iscc()?)
            .arg("/Qp")
            .arg(format!("/DAppVersion={version}"))
            .arg(format!("/DSourceDir={}", app.display()))
            .arg(format!("/DOutputDir={}", dist.display()))
            .arg(format!("/DRepoDir={}", root.display()))
            .arg(root.join("packaging").join("windows").join("luna.iss")))?;
        let out = require(out)?;
        write_appcast(
            &dist.join("appcast-windows.xml"),
            &version,
            &out,
            Some(WINDOWS_UPDATE_ARGS),
            &key,
        )?;
        out
    };
    println!("Installer: {}", out.display());
    Ok(out)
}

fn release(exact: Option<&str>) -> Result {
    let root = root();
    let status = output(Command::new("git").args(["status", "--porcelain"]))?;
    if !status.trim().is_empty() {
        return Err(
            "commit or stash your changes first; a release starts from a clean tree".into(),
        );
    }
    let old = version()?;
    let new = next_version(&old, today(), exact)?;
    let tag = format!("v{new}");
    if !output(Command::new("git").args(["tag", "--list", &tag]))?
        .trim()
        .is_empty()
    {
        return Err(format!("tag {tag} already exists"));
    }

    check()?;

    step(&format!("Version {old} -> {new}"));
    let manifest = root.join("Cargo.toml");
    let lock = root.join("Cargo.lock");
    let saved = (read(&manifest)?, read(&lock)?);
    write(&manifest, &set_version(&saved.0, &new)?)?;
    if let Err(e) = installer() {
        write(&manifest, &saved.0)?;
        write(&lock, &saved.1)?;
        return Err(format!("{e}\n(version left at {old})"));
    }

    run(Command::new("git").args(["commit", "-q", "-am", &format!("release: {tag}")]))?;
    run(Command::new("git").args(["tag", "-a", &tag, "-m", &format!("Luna {new}")]))?;
    if read(&root.join("packaging").join("R3D-EULA.txt"))?.contains("DRAFT") {
        println!("\nwarning: packaging/R3D-EULA.txt is still a draft; don't publish this build.");
    }
    println!(
        "\nReleased {tag} locally (nothing pushed).\n\
         Publish:  cargo xtask publish\n\
         Mac:      git fetch --tags && git checkout {tag} && cargo xtask installer && cargo xtask publish"
    );
    Ok(())
}

/// Creates the signing key and records its public half in the repository.
fn update_key() -> Result {
    let path = update::key_path();
    let public = update::new_key(&path)?;
    let public_file = root().join("packaging").join("update-public-key.txt");
    write(&public_file, &format!("{public}\n"))?;
    println!(
        "Private key: {} (keep it safe; copy it to the other build machine)\n\
         Public key:  {} (commit it)",
        path.display(),
        public_file.display()
    );
    Ok(())
}

/// Pushes the tag for the current version and uploads this machine's
/// installer to that tag's GitHub release, creating the release if needed.
/// Run once per platform; uploading again replaces the file.
fn publish() -> Result {
    let version = version()?;
    let tag = format!("v{version}");
    let tagged = output(Command::new("git").args(["rev-parse", &format!("{tag}^{{commit}}")]))
        .map_err(|_| format!("no tag {tag}; run `cargo xtask release` first"))?;
    let head = output(Command::new("git").args(["rev-parse", "HEAD"]))?;
    if tagged.trim() != head.trim() {
        return Err(format!("HEAD isn't {tag}; check it out first"));
    }
    let files = release_files(&version)?;

    step(&format!("Publishing {tag}"));
    let branch = output(Command::new("git").args(["branch", "--show-current"]))?;
    if !branch.trim().is_empty() {
        run(Command::new("git").args(["push", "origin", branch.trim()]))?;
    }
    run(Command::new("git").args(["push", "origin", &format!("refs/tags/{tag}")]))?;
    let exists = Command::new("gh")
        .args(["release", "view", &tag])
        .current_dir(root())
        .output()
        .map_err(|e| format!("gh: {e}"))?
        .status
        .success();
    if exists {
        run(Command::new("gh")
            .args(["release", "upload", &tag, "--clobber"])
            .args(&files)
            .current_dir(root()))?;
    } else {
        run(Command::new("gh")
            .args([
                "release",
                "create",
                &tag,
                "--verify-tag",
                "--generate-notes",
            ])
            .args(["--title", &format!("Luna {version}")])
            .args(&files)
            .current_dir(root()))?;
    }
    println!("Published {} files to {tag}", files.len());
    Ok(())
}

/// This platform's release files: the installer and its signed update feed.
fn release_files(version: &str) -> Result<Vec<PathBuf>> {
    let dist = root().join("dist");
    let again = |e: String| format!("{e}\nrun `cargo xtask installer` first");
    let (installer, feed) = if cfg!(target_os = "macos") {
        (
            dist.join(format!("Luna-{version}-macos.dmg")),
            dist.join("appcast-macos.xml"),
        )
    } else {
        (
            dist.join(format!("Luna-{version}-windows-x64-setup.exe")),
            dist.join("appcast-windows.xml"),
        )
    };
    let installer = require(installer).map_err(again)?;
    feed_is_fresh(&installer, &feed).map_err(again)?;
    Ok(vec![installer, feed])
}

/// The feed must exist and be written after the installer it signs.
fn feed_is_fresh(installer: &Path, feed: &Path) -> Result {
    let modified = |p: &Path| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .map_err(|_| format!("missing: {}", p.display()))
    };
    if modified(feed)? < modified(installer)? {
        return Err(format!(
            "{} is older than {}",
            feed.display(),
            installer.display()
        ));
    }
    Ok(())
}

/// Signs `file` and writes the one-release appcast that points at it on the
/// GitHub release for `version`.
fn write_appcast(
    path: &Path,
    version: &str,
    file: &Path,
    installer_arguments: Option<&str>,
    key: &ed25519_dalek::SigningKey,
) -> Result {
    let bytes = fs::read(file).map_err(|e| format!("read {}: {e}", file.display()))?;
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let enclosure = update::Enclosure {
        url: format!("{RELEASES}/download/v{version}/{name}"),
        length: bytes.len() as u64,
        signature: update::sign(key, &bytes),
        installer_arguments: installer_arguments.map(str::to_owned),
    };
    let notes = format!("{RELEASES}/tag/v{version}");
    write(path, &update::appcast(version, &notes, &enclosure))?;
    println!("Feed: {}", path.display());
    Ok(())
}

// ---- assembling ----

fn assemble_windows(
    root: &Path,
    dist: &Path,
    braw: &Path,
    r3d: &Path,
    r3d_sdk: &Path,
    licences: &str,
) -> Result<PathBuf> {
    let app = dist.join("Luna");
    let lib = app.join("lib");
    remove(&app)?;
    fs::create_dir_all(&lib).map_err(|e| format!("create {}: {e}", lib.display()))?;
    copy(
        &root.join("target/release/luna-app.exe"),
        &app.join("Luna.exe"),
    )?;
    // Blackmagic RAW SDK libraries (redistributable under the SDK licence, 1.1(d)).
    for dll in [
        "BlackmagicRawAPI.dll",
        "InstructionSetServicesAVX.dll",
        "InstructionSetServicesAVX2.dll",
    ] {
        copy(&braw.join(dll), &lib.join(dll))?;
    }
    // The R3D add-on (closed, own EULA) and RED's library. CPU decode only:
    // RED's GPU libraries (REDDecoder, REDCuda, REDOpenCL) never load.
    copy(
        &r3d.join("target/release/r3d.dll"),
        &lib.join("luna_r3d.dll"),
    )?;
    copy(
        &r3d_sdk.join("Redistributable/win/REDR3D-x64.dll"),
        &lib.join("REDR3D-x64.dll"),
    )?;
    for f in ["LICENSE", "NOTICE", "DISCLAIMER.md"] {
        copy(&root.join(f), &app.join(f))?;
    }
    // WinSparkle (MIT), which updates Luna; its licence travels with it.
    let winsparkle = extracted(WINSPARKLE_URL, WINSPARKLE_SHA256)?.join(WINSPARKLE_DIR);
    copy(
        &winsparkle.join("x64/Release/WinSparkle.dll"),
        &lib.join("WinSparkle.dll"),
    )?;
    copy(
        &winsparkle.join("COPYING"),
        &lib.join("WinSparkle-COPYING.txt"),
    )?;
    write(&app.join("LICENSES.txt"), licences)?;
    Ok(app)
}

fn assemble_macos(
    root: &Path,
    dist: &Path,
    version: &str,
    braw: &Path,
    r3d: &Path,
    r3d_sdk: &Path,
    licences: &str,
) -> Result<PathBuf> {
    let app = dist.join("Luna.app");
    let contents = app.join("Contents");
    let frameworks = contents.join("Frameworks");
    let resources = contents.join("Resources");
    remove(&app)?;
    for d in [
        contents.join("MacOS"),
        frameworks.clone(),
        resources.clone(),
    ] {
        fs::create_dir_all(&d).map_err(|e| format!("create {}: {e}", d.display()))?;
    }
    copy(
        &root.join("target/release/luna-app"),
        &contents.join("MacOS/Luna"),
    )?;
    write(&contents.join("Info.plist"), &info_plist(version))?;
    copy(
        &root.join("Assets/luna-logo.icns"),
        &resources.join("luna.icns"),
    )?;
    // Blackmagic RAW SDK frameworks; ditto keeps their symlinks.
    let entries = fs::read_dir(braw).map_err(|e| format!("read {}: {e}", braw.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".framework") {
            run(Command::new("ditto")
                .arg(entry.path())
                .arg(frameworks.join(&name)))?;
        }
    }
    copy(
        &r3d.join("target/release/libr3d.dylib"),
        &frameworks.join("libluna_r3d.dylib"),
    )?;
    copy(
        &r3d_sdk.join("Redistributable/mac/REDR3D.dylib"),
        &frameworks.join("REDR3D.dylib"),
    )?;
    for f in ["LICENSE", "NOTICE", "DISCLAIMER.md"] {
        copy(&root.join(f), &resources.join(f))?;
    }
    write(&resources.join("LICENSES.txt"), licences)?;

    let identity = env::var("LUNA_SIGN_IDENTITY").unwrap_or_else(|_| "-".into());
    let mut sign = Command::new("codesign");
    sign.args(["--force", "--deep", "--sign", &identity]);
    if identity != "-" {
        sign.args(["--options", "runtime", "--timestamp"]);
    }
    run(sign.arg(&app))?;
    Ok(app)
}

fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Luna</string>
    <key>CFBundleDisplayName</key><string>Luna</string>
    <key>CFBundleIdentifier</key><string>co.shaked.luna</string>
    <key>CFBundleExecutable</key><string>Luna</string>
    <key>CFBundleIconFile</key><string>luna</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>{version}</string>
    <key>CFBundleVersion</key><string>{version}</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#
    )
}

/// A drag-to-Applications disk image of the app.
fn dmg(app: &Path, dist: &Path, version: &str) -> Result<PathBuf> {
    let stage = dist.join("dmg");
    remove(&stage)?;
    fs::create_dir_all(&stage).map_err(|e| format!("create {}: {e}", stage.display()))?;
    run(Command::new("ditto").arg(app).arg(stage.join("Luna.app")))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("/Applications", stage.join("Applications"))
        .map_err(|e| format!("link Applications: {e}"))?;
    let out = dist.join(format!("Luna-{version}-macos.dmg"));
    remove(&out)?;
    run(Command::new("hdiutil")
        .args(["create", "-quiet", "-format", "UDZO", "-volname"])
        .arg(format!("Luna {version}"))
        .arg("-srcfolder")
        .arg(&stage)
        .arg(&out))?;
    remove(&stage)?;
    require(out)
}

/// What the installer asks people to accept: Luna's MIT licence and the R3D
/// add-on's EULA.
fn licences_text(root: &Path) -> Result<String> {
    Ok(format!(
        "LUNA\n\n{}\n\n\nR3D SUPPORT\n\n{}",
        read(&root.join("LICENSE"))?.trim(),
        read(&root.join("packaging/R3D-EULA.txt"))?.trim()
    ))
}

// ---- inputs ----

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace")
        .to_path_buf()
}

fn braw_libraries() -> Result<PathBuf> {
    let sdk = env::var_os("BRAW_SDK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "/Applications/Blackmagic RAW/Blackmagic RAW SDK".into()
            } else {
                r"C:\Program Files (x86)\Blackmagic Design\Blackmagic RAW\Blackmagic RAW SDK".into()
            }
        });
    let platform = if cfg!(target_os = "macos") {
        "Mac"
    } else {
        "Win"
    };
    require(sdk.join(platform).join("Libraries"))
}

fn r3d_repo() -> Result<PathBuf> {
    let repo = env::var_os("LUNA_R3D_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("../decoders/r3d-sdk"));
    require(repo.join("Cargo.toml"))?;
    Ok(repo)
}

fn iscc() -> Result<PathBuf> {
    if let Some(p) = env::var_os("ISCC") {
        return require(PathBuf::from(p));
    }
    let mut roots: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(|v| env::var_os(v).map(PathBuf::from))
        .collect();
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Programs"));
    }
    for dir in ["Inno Setup 7", "Inno Setup 6"] {
        for r in &roots {
            let p = r.join(dir).join("ISCC.exe");
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err(
        "Inno Setup isn't installed (https://jrsoftware.org/isdl.php), or set ISCC to ISCC.exe"
            .into(),
    )
}

// ---- version ----

/// The workspace version in the root `Cargo.toml`.
fn version() -> Result<String> {
    let text = read(&root().join("Cargo.toml"))?;
    text.lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| "no workspace version in Cargo.toml".into())
}

fn set_version(manifest: &str, new: &str) -> Result<String> {
    let mut done = false;
    let out: Vec<String> = manifest
        .lines()
        .map(|l| {
            if !done && l.starts_with("version = \"") {
                done = true;
                format!("version = \"{new}\"")
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !done {
        return Err("no workspace version in Cargo.toml".into());
    }
    Ok(out.join("\n") + "\n")
}

/// The next CalVer version, YEAR.MONTH.N: N counts releases within the month
/// and restarts at 0 in a new month. `exact` overrides it but must be newer.
fn next_version(old: &str, (year, month): (u64, u64), exact: Option<&str>) -> Result<String> {
    let parse = |v: &str| -> Option<[u64; 3]> {
        let mut it = v.split('.').map(|n| n.parse::<u64>().ok());
        let parts = [it.next()??, it.next()??, it.next()??];
        it.next().is_none().then_some(parts)
    };
    let current = parse(old).ok_or(format!("bad current version {old}"))?;
    let new = match exact {
        Some(v) => parse(v).ok_or(format!("`{v}` isn't a version like 2026.10.0"))?,
        None if current[..2] == [year, month] => [year, month, current[2] + 1],
        None => [year, month, 0],
    };
    if new <= current {
        return Err(format!(
            "{}.{}.{} isn't newer than {old}",
            new[0], new[1], new[2]
        ));
    }
    Ok(format!("{}.{}.{}", new[0], new[1], new[2]))
}

/// Today's (year, month) in UTC.
fn today() -> (u64, u64) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    year_month((secs / 86_400) as i64)
}

/// (year, month) of a day count since 1970-01-01 (Howard Hinnant's
/// civil_from_days).
fn year_month(days: i64) -> (u64, u64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as u64, month as u64)
}

// ---- helpers ----

/// A download checked against its pinned SHA-256, cached in `.cache/downloads`.
fn fetch_pinned(url: &str, sha256: &str) -> Result<PathBuf> {
    let name = url.rsplit('/').next().unwrap_or("download");
    let dir = root().join(".cache").join("downloads");
    let file = dir.join(name);
    let matches = |p: &Path| fs::read(p).is_ok_and(|b| sha256_hex(&b) == sha256);
    if !matches(&file) {
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        run(Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&file)
            .arg(url))?;
        if !matches(&file) {
            remove(&file)?;
            return Err(format!("{url} doesn't match its pinned SHA-256"));
        }
    }
    Ok(file)
}

/// A pinned archive, unpacked once into `.cache/<archive name>.d/`.
fn extracted(url: &str, sha256: &str) -> Result<PathBuf> {
    let archive = fetch_pinned(url, sha256)?;
    let name = archive.file_name().unwrap_or_default().to_string_lossy();
    let dir = root().join(".cache").join(format!("{name}.d"));
    let done = dir.join(sha256);
    if !done.exists() {
        remove(&dir)?;
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        // bsdtar reads zip and tar.xz. On Windows use the system one: Git
        // Bash's GNU tar may come first on PATH and can't read zip.
        let tar = match env::var_os("SystemRoot") {
            Some(sys) if cfg!(windows) => PathBuf::from(sys).join("System32").join("tar.exe"),
            _ => PathBuf::from("tar"),
        };
        run(Command::new(tar)
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&dir))?;
        write(&done, "")?;
    }
    Ok(dir)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn step(what: &str) {
    println!("\n== {what}");
}

fn cargo(args: &[&str]) -> Result {
    run(Command::new("cargo").args(args).current_dir(root()))
}

fn run(cmd: &mut Command) -> Result {
    let shown = format!(
        "{} {}",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("> {shown}");
    let status = cmd.status().map_err(|e| format!("{shown}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{shown} failed ({status})"))
    }
}

fn output(cmd: &mut Command) -> Result<String> {
    let out = cmd
        .current_dir(root())
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn require(p: PathBuf) -> Result<PathBuf> {
    if p.exists() {
        Ok(p)
    } else {
        Err(format!("missing: {}", p.display()))
    }
}

fn copy(from: &Path, to: &Path) -> Result {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}

fn read(p: &Path) -> Result<String> {
    fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))
}

fn write(p: &Path, text: &str) -> Result {
    fs::write(p, text).map_err(|e| format!("write {}: {e}", p.display()))
}

fn remove(p: &Path) -> Result {
    let r = if p.is_dir() {
        fs::remove_dir_all(p)
    } else if p.exists() {
        fs::remove_file(p)
    } else {
        return Ok(());
    };
    r.map_err(|e| format!("remove {}: {e}", p.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_abc() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn publish_refuses_stale_or_missing_feed() {
        let dir = std::env::temp_dir().join(format!("luna-feed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let setup = dir.join("setup.exe");
        let feed = dir.join("appcast-windows.xml");
        fs::write(&setup, "x").unwrap();
        assert!(feed_is_fresh(&setup, &feed).is_err(), "missing feed");
        fs::write(&feed, "x").unwrap();
        assert!(feed_is_fresh(&setup, &feed).is_ok());
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(&setup)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(
            feed_is_fresh(&setup, &feed).is_err(),
            "installer newer than feed"
        );
    }

    #[test]
    fn calendar_versions() {
        let oct = (2026, 10);
        assert_eq!(next_version("0.0.0", oct, None).unwrap(), "2026.10.0");
        assert_eq!(next_version("2026.10.0", oct, None).unwrap(), "2026.10.1");
        assert_eq!(next_version("2026.9.4", oct, None).unwrap(), "2026.10.0");
        assert_eq!(
            next_version("2026.10.1", (2027, 1), None).unwrap(),
            "2027.1.0"
        );
        assert_eq!(
            next_version("2026.10.1", oct, Some("2026.10.5")).unwrap(),
            "2026.10.5"
        );
        assert!(next_version("2026.10.1", oct, Some("2026.10.1")).is_err());
        assert!(next_version("2026.10.1", oct, Some("2026.10")).is_err());
        assert!(next_version("2026.11.0", oct, None).is_err());
    }

    #[test]
    fn months_from_days() {
        assert_eq!(year_month(0), (1970, 1));
        assert_eq!(year_month(20_741), (2026, 10)); // 2026-10-15
        assert_eq!(year_month(20_819), (2027, 1)); // 2027-01-01
        assert_eq!(year_month(20_818), (2026, 12)); // 2026-12-31
    }

    #[test]
    fn sets_only_the_workspace_version() {
        let m = "[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\nversion = \"9\"\n";
        let out = set_version(m, "0.2.0").unwrap();
        assert!(out.starts_with("[workspace.package]\nversion = \"0.2.0\"\n"));
        assert!(out.contains("version = \"9\""));
    }
}
