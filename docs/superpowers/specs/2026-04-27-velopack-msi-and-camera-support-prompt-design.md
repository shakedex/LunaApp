# Velopack MSI Installer + Camera-Support Prompt — Design

**Date:** 2026-04-27
**Status:** Approved (sections 1–5)
**Author:** Luna devs (shakedex)

## Goal

Bring Luna closer to a stable 1.0 release on Windows by:

1. Switching the Windows installer from Velopack's Squirrel-style `Setup.exe` to a Velopack-generated MSI with a proper Windows Installer wizard, install progress, and Add/Remove Programs entry.
2. Adding a top-right toast (mirroring the update-available toast) that informs the user when optional camera-support tooling (ARRI Reference Tool, Sony RAW Viewer) is missing, and lets them install or snooze the prompt without leaving the main window.
3. Verifying that the installer never prompts the end-user to install the .NET desktop runtime, VC++ redistributables, or any other framework — every binary the app needs (apart from the user-installable third-party camera tools) ships inside the install payload.

The release version stays at `1.0.0` until the installer + update procedure is judged production-ready. Velopack 1.0.0 will be the first proper public release; there is no existing user base to migrate.

## Non-goals

- Per-machine MSI installs (rejected — single-user film-set laptops, would require UAC every install/update).
- Custom hand-authored WiX project (rejected — duplicates work Velopack already does).
- WiX Burn bootstrapper bundling extra redists (deferred — only revisited if manual smoke test surfaces a missing native dep).
- Blackmagic RAW installer integration (out of scope — no installer wired today, license-blocked redistribution).
- DMG window-background styling on macOS (deferred — only adopted if Velopack exposes a single-flag toggle).
- macOS installer changes beyond adding the icon. The DMG flow on `osx-arm64` stays as-is.

## Section 1 — Build pipeline (Velopack MSI)

**Windows packaging:** `vpk pack` for `win-x64` (in both `build.ps1` and `.github/workflows/release.yml`) gains:

```
--msi
--instLocation PerUser
--splashImage Assets/install-splash.png
--icon Assets/luna-logo.ico
```

- `--msi` enables Velopack's WiX 5–authored MSI generation.
- `--instLocation PerUser` puts the install in `%LocalAppData%\Luna`. No UAC prompt at install time. Add/Remove Programs entry is per-user. Velopack's auto-update flow continues to operate on `%LocalAppData%\Luna` exactly as today (Squirrel-compatible directory layout) — MSI bootstraps the same payload, then the runtime updater takes over.
- `--splashImage Assets/install-splash.png` shows Luna branding during MSI extraction.
- `--icon Assets/luna-logo.ico` sets the wizard icon and the Add/Remove Programs icon.

**Splash asset:** new file `Assets/install-splash.png` (640×400 PNG derived from the existing `Assets/luna-logo-lg.webp`). Velopack accepts PNG/JPEG/GIF, not WebP, so a one-time conversion is committed alongside the existing assets.

**macOS packaging:** the `osx-arm64` `vpk pack` invocation gains `--icon Assets/luna-logo.icns` so the `.app` bundle and DMG mount-point inherit the Luna icon. No other macOS changes.

**Outputs:** when `--msi` is set, Velopack emits both an MSI and a `Setup.exe`. The release workflow uploads only the MSI (`Luna-1.0.0-win-x64.msi`) to the GitHub Release; the Setup.exe artifact is dropped from `actions/upload-artifact`.

**CI tooling:** `vpk` 0.0.1298 ships WiX 5 in-tree, so no extra `dotnet tool install wix` step is expected. If the first CI run fails resolving a WiX dependency, the workflow gains a `dotnet tool install wix` step before `vpk pack`.

**Release-notes block:** the `## Installation → Windows` section in `release.yml` swaps `Setup.exe` for `.msi`. SmartScreen guidance stays.

## Section 2 — Camera support detection service

New service `Services/CameraSupport/CameraSupportInstallationStatus.cs`:

```csharp
public sealed class CameraSupportInstallationStatus
{
    public sealed record MissingSupport(
        string Id,           // "arri" | "sony-venice"
        string DisplayName); // "ARRI Reference Tool"

    public IReadOnlyList<MissingSupport> ResolveMissing();
    public event EventHandler? StatusChanged;
    public void Invalidate();
}
```

The service is a thin probe — it answers "what's missing?" so the toast can decide whether to show. The actual install UX (progress, errors, detect button) already lives in Settings → Camera Support via `CameraSupportRow`; this service does not duplicate it.

**Logic:**

- Iterates `CameraSupportRegistry.All`.
- Emits a `MissingSupport` entry for any support whose `Status is SupportStatus.ComingLater` *and* has an installer wired in Settings (i.e. the existing row in `SettingsViewModel.CameraSupports` exposes an `InstallCommand`):
  - ARRI → `ArtCliInstaller.IsSupportedPlatform == true`, `ArtCliLocator.Resolve() is null`.
  - Sony Venice → `SonyRawViewerInstaller.IsSupportedPlatform == true`, `SonyRawViewerLocator.Resolve() is null`.
- Skips Sony on non-Windows because `SonyRawViewerLocator.Probe()` returns null on macOS today (out of scope until the macOS RAW Viewer probe lands).
- Skips Blackmagic — no installer is wired and license terms preclude redistribution; the existing `ComingLater` row in Settings remains the surface for that.

**Wiring:**

- Registered as a singleton in `ServiceRegistration.cs`.
- `Invalidate()` is called when Settings finishes an install (hook into the existing post-install path in `SettingsViewModel`) so the next probe re-runs the locators and raises `StatusChanged`. The toast hides itself once `ResolveMissing()` returns empty.

**Startup probe:** called once during `MainWindowViewModel` construction (after services load), off the UI thread (`Task.Run` — locators do disk I/O and may shell out to `--version`).

**On-demand probe (Q2 trigger B):** when an `UnsupportedFormatNotice` is emitted by the engine for a support whose installer is wired, `Invalidate()` is called and the toast re-shows. The unsupported-clip notice in the report stays informational; the toast is the call-to-action.

## Section 3 — UI: camera-support notice toast

**Scoped down:** the toast is a passive notice that points the user at the existing Settings → Camera Support panel. No inline progress, no per-row install button, no detect button — Settings already has all of that via `CameraSupportRow` and we don't duplicate it.

**Location:** sibling of the update toast in `Views/MainWindow.axaml`, top-right corner, stacked below the update toast when both are visible. New ViewModel partial `ViewModels/MainWindowViewModel.CameraSupport.cs`.

**Visual shape:**

```
┌──────────────────────────────────────────────┐
│ 📷  Camera support missing               ✕  │
│                                              │
│ ARRI Reference Tool, Sony RAW Viewer not     │
│ installed. Some camera formats won't decode  │
│ until you install them from Settings.        │
│                                              │
│ [Open Settings]   [Later (3 days)]           │
└──────────────────────────────────────────────┘
```

The body text lists the missing tools by display name (joined with commas). When only one is missing, the wording adjusts ("ARRI Reference Tool isn't installed…").

**ViewModel state (`MainWindowViewModel.CameraSupport.cs`):**

```csharp
[ObservableProperty] bool _hasMissingCameraSupport;
[ObservableProperty] string _missingCameraSupportSummary = string.Empty;
```

`MissingCameraSupportSummary` is the rendered list ("ARRI Reference Tool, Sony RAW Viewer"). Recomputed whenever `CameraSupportInstallationStatus.StatusChanged` fires.

**Commands:**

- `OpenSettingsForCameraSupportCommand` — opens `SettingsWindow` and scrolls/focuses the Camera Support section. The user installs from there using the existing UX. Toast stays visible (or hides if the install completes and `ResolveMissing()` empties).
- `RemindCameraSupportLaterCommand` — sets `_appSettings.CameraSupportSnoozeUntil = DateTime.Now.AddDays(3)`, hides the toast.
- `DismissCameraSupportCommand` — hides for the current session only (no persistence).

**Snooze gate:** mirrors the existing `IsSnoozed` pattern in `ViewModels/MainWindowViewModel.Update.cs:21-22`. The toast is suppressed at startup if `CameraSupportSnoozeUntil` is in the future.

**Persistence (`Models/AppSettings.cs`):** new field `DateTime? CameraSupportSnoozeUntil`. Loaded/saved alongside `UpdateSnoozeUntil`.

**Auto-hide on success:** when the user installs a tool from the Settings panel, the existing post-install path calls `CameraSupportInstallationStatus.Invalidate()`. The toast subscribes to `StatusChanged`, recomputes `ResolveMissing()`, and either updates `MissingCameraSupportSummary` or sets `HasMissingCameraSupport = false` if the list empties.

**Styling:** reuses `LunaBgSecondary`, `LunaAccent`, `LunaAccentSubtle`, `LunaBorder`, the `fade` animation class, and the `LunaRadiusLg` corner radius — identical brush set to the update toast so both cards stack cleanly.

## Section 4 — Bundled-runtime guarantee + verification gates

**Existing guarantees (verified during exploration):**

- `LunaApp.csproj` sets `SelfContained=true`, `PublishTrimmed=true` (partial), `IncludeNativeLibrariesForSelfExtract=true`. The .NET 10 runtime DLLs ship inside the install dir.
- `build.ps1:73-79` and `release.yml:82-89` already assert `System.Private.CoreLib.dll` exists post-publish and abort the build if missing.
- Skia, MediaInfo, FFmpeg native DLLs ship via NuGet `runtimes/win-x64/native/` and are copied into publish output by the SDK.

**New CI verification gates (fail-fast, added to both `build.ps1` and `release.yml`):**

1. **Managed assembly assertion** — assert the following exist in `publish/win-x64/`: `Luna.exe`, `Avalonia.dll`, `SkiaSharp.dll`, `FFmpeg.AutoGen.dll`, `MediaInfo.Wrapper.Core.dll`, `QuestPDF.dll`, `Velopack.dll`. Build fails if any is missing — catches trimmer regressions before they ship.
2. **Native deps assertion** — assert the following exist next to `Luna.exe`:
   - `libSkiaSharp.dll`
   - `MediaInfo.dll`
   - `tools/ffmpeg/win-x64/avcodec-61.dll` (and the matching `avformat`, `avutil`, `swresample`, `swscale` siblings shipped via the `tools/ffmpeg/**` `<None Include>` payload).
   - `av_libglesv2.dll` (Avalonia ANGLE) — only if Avalonia 11.3.x ships ANGLE; otherwise the assertion is a soft warning.
3. **Avalonia.Diagnostics absence** — assert `Avalonia.Diagnostics.dll` is *not* present in Release publish output. The csproj already conditions it out, but a CI assertion catches accidental re-inclusions.
4. **Manifest sanity** — `app.manifest` is verified to contain `<compatibility>` GUIDs covering Win10 and Win11. If missing, the manifest is patched.
5. **MSI prerequisite check** — after the first `vpk pack --msi` run, decompile the resulting MSI (`dark.exe Luna.msi -o decompiled.wxs`) and grep for `LaunchCondition` and `.NETFramework` references. Velopack MSIs authored via WiX 5 do not inject .NET prerequisite checks by default (the MSI bootstraps Velopack which extracts the self-contained payload), but verifying once before going public is cheap. If a launch condition is found, suppress via the appropriate Velopack flag or post-process.

The user runs the manual end-to-end smoke test (clean Win11 VM, install MSI, click through, launch). CI just enforces the static guarantees so regressions can't slip silently.

## Section 5 — Testing & rollout

**Unit tests** (test project location confirmed during plan phase — added inline if no `Tests/` project exists):

- `CameraSupportInstallationStatusTests`:
  - Returns empty when both ART CLI and Sony RAW Viewer are detected.
  - Returns the ARRI entry only when the ART CLI locator returns null.
  - Returns the Sony entry only when the Sony locator returns null on Windows.
  - Returns empty for Sony on non-Windows platforms.
  - Skips Blackmagic (no installer wired).
  - `Invalidate()` triggers a re-probe and raises `StatusChanged`.
- ViewModel toast tests:
  - `MissingCameraSupportSummary` is rebuilt correctly for 1, 2, or 0 missing tools.
  - `HasMissingCameraSupport` flips to false when `ResolveMissing()` returns empty after `StatusChanged`.
  - Snooze gate suppresses the toast when `CameraSupportSnoozeUntil` is in the future, mirroring whatever pattern the update-snooze tests use (or establishing the pattern if none exists).

**Manual smoke test (user-driven, per dropped Q4):**

1. Fresh Windows 11 VM. Install the MSI. Confirm: no UAC prompt, no .NET / VC++ prompt, an Add/Remove Programs entry exists, the app launches.
2. Camera-support toast appears at startup listing both missing tools in the summary line.
3. Click `Open Settings` → Settings window opens at the Camera Support section. Install ARRI from the existing row → toast summary updates to Sony only, then hides once Sony is installed too.
4. Drop a folder containing a `.ari` file *before* installing ART CLI → toast re-appears (on-demand trigger path).
5. Click `Later` → toast hides → restart the app → toast still hidden (3-day snooze persists).
6. Trigger update flow via dev banner → both toasts visible top-right, stacked, no overlap.
7. Uninstall via Add/Remove Programs → install dir cleaned, shortcuts gone, no leftover registry keys.

**Rollout:**

- Release version stays at `1.0.0`. Velopack 1.0.0 is the first proper public release; the previous `Setup.exe` builds were dev iterations only.
- Tag `v1.0.0` after the manual smoke test passes; run the release workflow.
- Update `docs/INSTALLATION.md` to reference `.msi` instead of `Setup.exe`.
- README screenshot updated to show the camera-support toast.

**Risk:** none on the migration axis — there is no existing public install base. Future MSI-version transitions will rely on Velopack's auto-update flow targeting the per-user `%LocalAppData%\Luna` payload, which is unchanged by the installer-format swap.

## Open questions

- WiX 5 toolchain availability inside `vpk` 0.0.1298 — confirmed by Velopack docs but verified empirically on first CI run; if missing, the workflow gains a `dotnet tool install wix` step.
- Whether Velopack's DMG generator exposes a one-flag toggle for a custom drag-to-Applications background image. If yes, opportunistically adopted; otherwise deferred.
