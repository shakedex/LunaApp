//! Luna's build tasks: `cargo xtask <task>` (`cargo xtask` lists them).
//!
//! The same commands serve day-to-day development (`check`), a local build to
//! try (`package`), the installers people download (`installer`), and cutting
//! a version (`release`). Windows builds `Luna-<version>-windows-x64-setup.exe`
//! with Inno Setup; macOS builds `Luna-<version>-macos.dmg` with Apple's tools.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const HELP: &str = "\
Luna build tasks

  cargo xtask check             fmt check, clippy and tests (before every commit)
  cargo xtask package           build the app into dist/ to try it
  cargo xtask installer         package, then build the installer into dist/
  cargo xtask release <bump>    check, set the version, build the installer,
                                commit and tag. <bump>: patch, minor or 0.4.0

Inputs (environment variables, each with a default):
  BRAW_SDK_DIR         Blackmagic RAW SDK install
  LUNA_R3D_REPO        r3d-sdk checkout (default: ../decoders/r3d-sdk)
  ISCC                 Inno Setup compiler, Windows (default: Program Files)
  LUNA_SIGN_IDENTITY   macOS codesign identity (default: ad-hoc)
";

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => check(),
        Some("package") => package().map(|_| ()),
        Some("installer") => installer().map(|_| ()),
        Some("release") => match args.get(1) {
            Some(bump) => release(bump),
            None => Err("release needs a bump: patch, minor or a version like 0.4.0".into()),
        },
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
        require(out)?
    };
    println!("Installer: {}", out.display());
    Ok(out)
}

fn release(bump: &str) -> Result {
    let root = root();
    let status = output(Command::new("git").args(["status", "--porcelain"]))?;
    if !status.trim().is_empty() {
        return Err(
            "commit or stash your changes first; a release starts from a clean tree".into(),
        );
    }
    let old = version()?;
    let new = bumped(&old, bump)?;
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
         Publish:  git push origin HEAD {tag}\n\
         Mac:      git fetch --tags && git checkout {tag} && cargo xtask installer"
    );
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

fn bumped(old: &str, bump: &str) -> Result<String> {
    let parse = |v: &str| -> Option<[u64; 3]> {
        let mut it = v.split('.').map(|n| n.parse::<u64>().ok());
        let parts = [it.next()??, it.next()??, it.next()??];
        it.next().is_none().then_some(parts)
    };
    let [major, minor, patch] = parse(old).ok_or(format!("bad current version {old}"))?;
    let new = match bump {
        "patch" => [major, minor, patch + 1],
        "minor" => [major, minor + 1, 0],
        exact => parse(exact).ok_or(format!(
            "`{exact}` isn't patch, minor or a version like 0.4.0"
        ))?,
    };
    if new <= [major, minor, patch] {
        return Err(format!(
            "{}.{}.{} isn't newer than {old}",
            new[0], new[1], new[2]
        ));
    }
    Ok(format!("{}.{}.{}", new[0], new[1], new[2]))
}

// ---- helpers ----

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
    fn bumps() {
        assert_eq!(bumped("0.3.4", "patch").unwrap(), "0.3.5");
        assert_eq!(bumped("0.3.4", "minor").unwrap(), "0.4.0");
        assert_eq!(bumped("0.3.4", "0.10.0").unwrap(), "0.10.0");
        assert!(bumped("0.3.4", "0.3.4").is_err());
        assert!(bumped("0.3.4", "0.3").is_err());
        assert!(bumped("0.3.4", "major").is_err());
    }

    #[test]
    fn sets_only_the_workspace_version() {
        let m = "[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\nversion = \"9\"\n";
        let out = set_version(m, "0.2.0").unwrap();
        assert!(out.starts_with("[workspace.package]\nversion = \"0.2.0\"\n"));
        assert!(out.contains("version = \"9\""));
    }
}
