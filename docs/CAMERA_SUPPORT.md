# Camera Support — architecture & how to add a vendor

This doc covers Luna's camera-support layer: how the app routes a camera file
through metadata extraction, frame decoding, and vendor-tool integration —
and what it takes to add a new camera (BRAW, RED REDCODE, Canon, …) without
spelunking the whole codebase.

If you only need the user-facing roadmap (what's supported today, what's
"coming later"), read **Settings → Camera Support** in the running app —
each row is a live `ICameraSupport` registration. This doc is the developer
view.

---

## The one abstraction that matters

Every camera family is a single class implementing
[`ICameraSupport`](../Services/CameraSupport/ICameraSupport.cs).

```csharp
public interface ICameraSupport
{
    string Id { get; }                                 // "generic", "arri", "blackmagic-raw", "sony-venice"
    string DisplayName { get; }                        // "ARRI ALEXA (ARRIRAW, ProRes, MXF)"
    IReadOnlySet<string> HandledExtensions { get; }    // [".ari"]
    SupportStatus Status { get; }                      // Ready / NotAvailable / ComingLater

    bool CanHandle(string filePath);
    Task<CameraClip> ProcessAsync(string filePath, bool extractThumbnails,
                                  int thumbnailCount, int thumbnailWidth,
                                  CancellationToken ct);
}
```

That's the whole API Luna core knows about camera formats. Adding a new
format = writing one class and registering it in
[`ServiceRegistration.cs`](../ServiceRegistration.cs).

The dispatcher is [`CameraSupportRegistry`](../Services/CameraSupport/CameraSupportRegistry.cs):
proprietary supports are checked before the generic fallback so a scaffolded
ARRI / BRAW / RED CSP wins the dispatch for its own files even when its
binaries aren't installed yet — and emits a typed
[`UnsupportedFormatNotice`](../Models/UnsupportedFormatNotice.cs) instead of
silently dropping the file into FFmpeg.

---

## Container ≠ vendor — read the file, not the extension

ARRI ALEXAs record ARRIRAW (`.ari`) **or** ProRes (`.mxf` / `.mov` / `.mp4`).
Sony Venice records X-OCN **or** XAVC, both in `.mxf`. Blackmagic cameras
record BRAW **or** ProRes in MOV. Filename prefixes (A001C001, M001C001) are
shared across vendors.

The rule we enforce throughout: **dispatch by extension only when the
extension is unambiguously vendor-specific** (`.ari`, `.braw`, `.r3d`).
Everything else (`.mxf`, `.mov`, `.mp4`) routes to
[`GenericCameraSupport`](../Services/CameraSupport/GenericCameraSupport.cs)
where content-aware enrichers self-select on real signals:

| Vendor signal | Where | Reads |
|---|---|---|
| `compatible_brands=ARRI` (QuickTime atom) | [`ArriQuickTimeEnricher`](../Services/Chappie/ArriQuickTimeEnricher.cs) | `com.arri.camera.*` ISO/shutter/WB/lens for ARRI MOV |
| `company_name=ARRI` (MXF Identification Descriptor) | same enricher, MXF branch | model + product version for ALEXA 35 / Mini LF MXF |
| `transfer_characteristics=0E17…` (SMPTE UL) | same enricher (via MediaInfo) | Log-C gamma for ARRI MXF |
| `NonRealTimeMeta` XML sidecar | [`SonyXmlEnricher`](../Services/Chappie/SonyXmlEnricher.cs) | Sony Venice/Burano/FX9 metadata |
| `manufacturer=Blackmagic Design` (QT format tag) | (future `BlackmagicEnricher`) | model / clip / lens info from BRAW container |
| ART CLI `metadata.json` | [`ArtCliMetadataEnricher`](../Services/Chappie/ArtCliMetadataEnricher.cs) | per-frame sensor / lens for ARRI MXF when art-cmd is installed |

Adding vendor branding for a new camera = write one `IMetadataEnricher`
that probes the right signal and overlays fields. No dispatch change.

---

## The thumbnail-generator chain

Frames are the hard problem — vendor RAW codecs (X-OCN, ARRIRAW, BRAW,
REDCODE) require vendor decoders. We compose them as a priority-ordered
chain in [`GenericCameraSupport`](../Services/CameraSupport/GenericCameraSupport.cs):

```
FfmpegThumbnailService          (priority 0)   — universal fallback
        ↓ NoDecoder
ArtCliThumbnailService          (priority 50)  — claims .ari, uses art-cmd
        ↓ NoDecoder
SonyRawExporterThumbnailService (priority 100) — claims X-OCN, uses rawexporter
        ↓ NoDecoder  → final outcome on the clip
```

**Cascade rule**: a [`ThumbnailOutcome.NoDecoder`](../Models/ThumbnailOutcome.cs)
falls through to the next generator. Any other outcome (`Success`,
`SeekFailed`, `DecodeFailed`, `ContainerOpenFailed`) stops the chain — those
mean "this generator owns the decision" and cascading would mask real
failures. Each generator advertises its own `Priority` via
[`IThumbnailGenerator`](../Services/Chappie/IThumbnailGenerator.cs).

A vendor-specific generator typically:

1. Returns `NoDecoder("not …")` when the codec string in the
   [`ThumbnailRequest`](../Services/Chappie/IThumbnailGenerator.cs) doesn't
   match its supported list — keeps the chain moving.
2. Returns `NoDecoder("vendor tool not installed")` when its `…Locator`
   can't find the binary — same cascade, so a missing vendor tool doesn't
   break files another generator can handle.
3. On a hit, shells out to the vendor tool to export a still per requested
   position (DPX / TIFF / EXR), then ingests via
   [`FfmpegThumbnailService.DecodeSingleImage`](../Services/Chappie/FfmpegThumbnailService.cs)
   — same WebP-encoding path as every other clip. **Don't reinvent image
   decoding.**

---

## The four payload strategies

Each vendor has its own legal / practical story for how its binaries reach
the user's machine. Pick one of these patterns and copy the closest existing
implementation:

| Strategy | Example | Locator | Installer | Thumbnail / metadata service |
|---|---|---|---|---|
| **managed-extract** — public CDN, stable URL, we extract a zip | ART CLI | [`ArtCliLocator`](../Services/Chappie/ArtCliLocator.cs) — probes `%LOCALAPPDATA%\Luna\tools\<vendor>\<rid>\` | [`ArtCliInstaller`](../Services/Chappie/ArtCliInstaller.cs) — pinned URL + SHA-256, downloads, verifies, extracts | [`ArtCliMetadataEnricher`](../Services/Chappie/ArtCliMetadataEnricher.cs) + [`ArtCliThumbnailService`](../Services/Chappie/ArtCliThumbnailService.cs) |
| **vendor-installer-launch** — public CDN, stable URL, contents is an interactive installer (.exe / .pkg / .dmg) | Sony RAW Viewer | [`SonyRawViewerLocator`](../Services/Chappie/SonyRawViewerLocator.cs) — probes vendor's standard install path (`C:\Program Files\Sony\RAW Viewer\`) | [`SonyRawViewerInstaller`](../Services/Chappie/SonyRawViewerInstaller.cs) — downloads, verifies, **launches** the .exe with `UseShellExecute=true Verb=runas`. **Does not WaitForExit** (interactive installers fork elevated children). | [`SonyRawExporterThumbnailService`](../Services/Chappie/SonyRawExporterThumbnailService.cs) |
| **detect-existing** — vendor binaries arrive only as part of a separately-installed app (DaVinci Resolve, Catalyst Browse). We never download — only detect. | (future BRAW via Resolve) | probes Resolve / RAW Player / Speed Test install dirs | n/a | reads vendor SDK from the detected install |
| **link-out** — vendor gates downloads behind an account or unscriptable EULA. We can't auto-install. | (future, hopefully none) | same as detect-existing | a button that opens the vendor's download page in the user's browser | activates after detect-existing succeeds |

Sony RAW Viewer's site uses an EULA-scroll JS gate on `www.sony.com` and
Akamai-blocks bots — but the **actual binary lives on `download.pro.sony`
which is unauthenticated**. Always check whether a vendor's static asset CDN
is reachable before assuming you're stuck with link-out.

---

## Adding a new vendor — concrete recipe

Working example: adding Blackmagic RAW SDK. Replace "BlackmagicRawSdk" with
your vendor's name throughout.

### 1. Locator + Installer (pick a strategy from the table above)

Create `Services/Chappie/BlackmagicRawSdkLocator.cs` — mirror
`SonyRawViewerLocator`. Probe the standard install paths, run a `--version`
to confirm, cache the result, expose `Resolve(forceRefresh)` and
`Invalidate()`.

Create `Services/Chappie/BlackmagicRawSdkInstaller.cs` — pick whichever
parent fits the strategy. For managed-extract, copy `ArtCliInstaller`. For
vendor-installer-launch, copy `SonyRawViewerInstaller`. Pin URL + SHA-256
per RID. Register both in
[`ServiceRegistration.cs`](../ServiceRegistration.cs).

### 2. Wire the install button

[`SettingsViewModel.BuildRow`](../ViewModels/SettingsViewModel.cs) is a
switch on `support.Id`. Add a branch:

```csharp
"blackmagic-raw" => (
    BuildSizeLabel("Install Blackmagic RAW SDK", _brawInstaller.CurrentRelease?.DownloadSizeBytes),
    new AsyncRelayCommand(() =>
        RunInstallAsync(support.Id,
                        p => _brawInstaller.InstallAsync(p),
                        () => _brawLocator.Invalidate()))),
```

The shared [`RunInstallCoreAsync`](../ViewModels/SettingsViewModel.cs)
drives the row's `IsInstalling` / `InstallProgress` / `InstallError` /
`IsAwaitingDetect` observables, plus the Detect button on rows where the
locator can't confirm completion synchronously — no extra UI work.

### 3. Metadata enricher

If the vendor's tool produces metadata (CLI export, SDK reads, sidecar),
write an `IMetadataEnricher` that consumes it and overlays
[`CameraClip`](../Models/CameraClip.cs) fields. Set its `Priority` higher
than `ArriQuickTimeEnricher` (120) so it overrides container-tag values.
Inject the locator and gate `CanEnrich` on whether the binary is installed.
Register as `IMetadataEnricher` in `ServiceRegistration` —
`GenericCameraSupport` picks it up automatically through
`IEnumerable<IMetadataEnricher>` injection.

### 4. Thumbnail service

If the vendor's tool can decode frames, write an `IThumbnailGenerator`.

```csharp
public sealed class BlackmagicRawSdkThumbnailService : IThumbnailGenerator
{
    public int Priority => 75;   // pick a slot in the chain
    public bool IsAvailable => _locator.Resolve() is not null;

    public async Task<ThumbnailResult> GenerateAsync(
        string filePath, ThumbnailRequest request, CancellationToken ct)
    {
        // 1. Bail with NoDecoder for files this vendor doesn't own.
        if (!IsBlackmagicRawCodec(request.Codec))
            return ThumbnailResult.NoDecoder("not BRAW");

        // 2. Bail with NoDecoder if the tool isn't installed.
        if (!IsAvailable)
            return ThumbnailResult.NoDecoder("BRAW SDK not installed");

        // 3. Export a frame per requested position to a temp dir,
        //    then decode the resulting image via FfmpegThumbnailService.DecodeSingleImage.
        // 4. Return Success(frames) or DecodeFailed(detail).
    }
}
```

Register as `IThumbnailGenerator` — the chain picks it up by priority. Pick
the priority slot deliberately: lower than another vendor that should win on
shared codecs (none today), higher than FFmpeg (which is the universal
fallback at 0).

### 5. Camera support class

Create `Services/CameraSupport/BlackmagicRawCameraSupport.cs`. The two
patterns to follow:

- **`.ari`-style scaffold** — for unambiguous proprietary extensions.
  Returns a typed `Unsupported` clip when the locator can't find the tool;
  delegates to baseline + enricher chain + thumbnail chain when it can. See
  [`ArriCameraSupport`](../Services/CameraSupport/ArriCameraSupport.cs) — the
  Status property flips based on locator detection, and `ProcessAsync`
  switches between `Unsupported` and full-pipeline.
- **No-extension scaffold** — for vendors whose files share extensions with
  others (Sony X-OCN sits in plain `.mxf`). The CSP claims nothing
  (`HandledExtensions = empty`) and exists only so its row appears in
  Settings — the actual handling rides on the enricher + thumbnail chain in
  Generic. See
  [`SonyVeniceCameraSupport`](../Services/CameraSupport/SonyVeniceCameraSupport.cs).

Register as `ICameraSupport` in `ServiceRegistration`. Done.

---

## Helpful invariants

- `RunInstallCoreAsync` always either flips a row to `Ready` (when the
  locator finds the binary post-install) or to `IsAwaitingDetect = true`
  (when it doesn't — "installer is running externally, click Detect when
  done"). This handles both the synchronous and asynchronous install
  shapes uniformly.
- Locators have `Invalidate()` so post-install re-probes don't hit a stale
  cache. Always call it before re-resolving.
- Pinned SHA-256s for every download. Never trust a CDN — verify the bytes.
  Mismatch aborts the install with a clear error.
- Vendor binaries land in `%LOCALAPPDATA%\Luna\tools\<vendor>\<rid>\` (or
  the platform equivalent). Never write to Program Files (no admin), never
  write next to Luna.exe (Velopack updates wipe it).
- Status messages are user-facing prose, not log strings: *"Install Sony
  RAW Viewer to enable X-OCN frame extraction"*, not *"locator returned
  null"*. They show up in Settings.

---

## What lives where

- [`Models/`](../Models/) — `CameraClip`, `CameraReel`, `ThumbnailFrame`,
  `ThumbnailOutcome`, `UnsupportedFormatNotice`. Pure data, no behaviour.
- [`Services/CameraSupport/`](../Services/CameraSupport/) — `ICameraSupport`,
  `CameraSupportRegistry`, one `…CameraSupport.cs` per camera family.
- [`Services/Chappie/`](../Services/Chappie/) — extractors / enrichers /
  thumbnail generators / locators / installers. Per-vendor files prefixed
  with the vendor name (`Arri…`, `Sony…`, `ArtCli…`, future `BlackmagicRaw…`).
- [`Services/ReportGenerationService.cs`](../Services/ReportGenerationService.cs)
  — orchestrates scan → reel detection → report writers. Doesn't touch the
  camera-support layer directly; consumes finished `CameraClip` objects.
- [`ServiceRegistration.cs`](../ServiceRegistration.cs) — every `ICameraSupport`,
  `IMetadataEnricher`, `IThumbnailGenerator` registers here. Scan it when
  you're trying to figure out what's wired.
- [`ViewModels/SettingsViewModel.cs`](../ViewModels/SettingsViewModel.cs)
  — install / detect UI plumbing. The `BuildRow` switch and
  `RunInstallCoreAsync` / `DetectAsync` are the contact points new vendors
  hook into.

---

## When in doubt

Pattern-match against the closest existing vendor — ARRI for managed-extract,
Sony for vendor-installer-launch, the (future) BRAW work for
detect-existing. Copy the file, rename, repin the URL/SHA, swap the codec
markers. The architecture rewards consistency; resist inventing a new
shape unless the vendor genuinely demands one.
