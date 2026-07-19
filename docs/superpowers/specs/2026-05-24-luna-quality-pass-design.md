# Luna Quality Pass — Design

**Date:** 2026-05-24
**Status:** Draft (awaiting user review)
**Author:** Luna devs (shakedex)

## Goal

A targeted quality pass that fixes the most user-visible failure modes, replaces the toast-based notification system with contextual surfaces (state strip + inline banner + hero finish), tightens copy and wording across the main flows, and clears the high-confidence backlog items surfaced by the 2026-05-24 audit.

The pass is bounded — no new features, no UI modernization (glass / acrylic was explicitly de-scoped), no architectural refactors beyond what the notification system requires.

## Non-goals

- Glass / acrylic / Mica visual modernization (deferred to a separate effort).
- Light-mode activation. The light theme dictionary stays scaffolded but unwired.
- Unit tests for the changes in this pass (deferred — captured as backlog item L5).
- Non-modal Settings window (M7, deferred).
- Recent-sources UI surface (M8, deferred).
- User-configurable vendor tool paths (M12, deferred).
- Service-layer test scaffolding (L5, deferred).
- Localization. Strings stay English-only.
- Parallelization of clip processing. `ChappieEngine.ProcessClipsAsync` stays serial; only cancellation honoring is tightened.
- Native resource cleanup audit. Spot-check during fact-check confirmed `FfmpegThumbnailService` already cleans up properly in `finally`; no work needed.
- MediaInfo DI graceful-degrade. Fact-check confirmed the extractor already handles per-file failure via try/catch.
- ARRI installer error capture. Fact-check confirmed the code already assigns `row.InstallError = result.Error` at `SettingsViewModel.cs:359`.

## Section 1 — Notification architecture

Three contextual surfaces replace the toast system. All live in `Views/Controls/` as Avalonia `UserControl`s.

| Surface | Purpose | Location |
|---|---|---|
| **StateStrip** | Last-action result (success / error / info / warning) with optional inline action button | MainWindow Grid.Row=4 — replaces current status bar |
| **BannerStack** (stackable items) | Persistent app-level notices (update available, camera support missing) | MainWindow Grid.Row=1 — drops in below header; stacks if multiple apply |
| **InlineBanner** | Dialog-scoped failure (settings save) | Inside SettingsWindow, top of the scroll area |

**Levels:** `Info` (neutral, `LunaBgTertiary` background), `Success` (`LunaSuccess` accent), `Warning` (`LunaWarning` accent), `Error` (`LunaDanger` accent). Background uses a 14% opacity tint over the surface; foreground text and icon use the full accent color.

**StateStrip behaviour:**
- Auto-clears on the next user action (drop, scan, generate, settings open).
- Manual `×` dismiss only when level is `Warning` or `Error`.
- Action button slot for inline CTAs (e.g. "Open folder", "Retry").
- No animation in / animation out — just appears with the next state push. Avoids the floating-card feel.

**BannerStack behaviour:**
- Slim row (height ~36 px), full width, no shadow, no rounded corners on the outer edge, 1 px bottom border using `LunaBorder`.
- Stackable: each item is a `BannerItem`. Items ordered by priority (`Update > CameraSupport`).
- Each item carries: `Key`, `Level`, `Title`, `Body`, `PrimaryAction (Label + Command)`, `SecondaryAction (optional)`, `IsDismissible`, `OnDismiss`.
- `Key` makes `AddOrReplace` idempotent — existing VM partial logic just pushes a new state through.

**Hero overlay extension:**
- New `OverlayState` enum on `MainWindowViewModel`: `Idle | Processing | SuccessHold | Fading`.
- After a successful `GenerateReportsAsync`, the existing 800 ms tail extends to a ~2 s success hold with the moon at 100% and an inline label "Report saved" plus action buttons "Open folder" / "Open report".
- Clicking either action snaps the overlay closed immediately.
- If the 2 s elapses without click, overlay fades and the state strip carries forward: *"Reports saved to {folder} · Open folder"*.

**ViewModel impact:**
- `StatusText` (string, used throughout) is replaced by a single `State` property of type `StateMessage` — a small record `(Level Level, string Text, string? ActionText, IRelayCommand? ActionCommand, bool IsDismissible)`. The `StateStrip` UserControl binds to this single object and renders its fields. Factory helpers: `StateMessage.Info(text)`, `StateMessage.Success(text, actionText?, actionCommand?)`, `StateMessage.Warning(...)`, `StateMessage.Error(...)`.
- `HasUpdateAvailable` + `HasMissingCameraSupport` bools no longer drive visibility directly. They still exist as VM properties, but their setters push `BannerItem`s into `MainWindowViewModel.Banners` (an `ObservableCollection<BannerItem>`).
- New nullable `_completedReportPaths` and `SuccessLabel` properties driving the hero finish state.

## Section 2 — Exception mapping (H1)

New static utility `Services/ExceptionMapper.cs`:

```csharp
public static class ExceptionMapper
{
    public static StateMessage ToUserMessage(Exception ex, string contextVerb) => ex switch
    {
        OperationCanceledException        => StateMessage.Info($"{contextVerb} cancelled"),
        UnauthorizedAccessException       => StateMessage.Error("Permission denied — check folder access"),
        DirectoryNotFoundException        => StateMessage.Error("Folder not found"),
        FileNotFoundException             => StateMessage.Error("File not found"),
        PathTooLongException              => StateMessage.Error("File path is too long"),
        IOException io when IsDiskFull(io)=> StateMessage.Error("Not enough disk space"),
        IOException                       => StateMessage.Error("File system error — the file may be open in another app"),
        _                                 => StateMessage.Error("Something went wrong")
    };

    private static bool IsDiskFull(IOException io) =>
        (io.HResult & 0xFFFF) is 0x70 or 0x27;  // ERROR_DISK_FULL / ENOSPC
}
```

`StateMessage` is a small record: `(Level Level, string Text)`. Raw `ex` is always logged via `Log.Error(ex, ...)` before mapping — the user sees friendly text, the dev sees the full stack.

**Call sites updated:**
- `MainWindowViewModel.Import.cs:104` — `QuickScanFolderAsync` catch, `contextVerb = "Scan"`.
- `MainWindowViewModel.Import.cs:159` — `StartProcessingAsync` catch, `contextVerb = "Processing"`.
- `MainWindowViewModel.Import.cs:299` — `GenerateReportsAsync` catch, `contextVerb = "Generation"`.
- `MainWindowViewModel.Import.cs:233` — `Process.Start` failure when opening report. New state-strip warning: *"Report saved but couldn't open it. Find it at: {path}"* with "Open folder" action.
- `MainWindowViewModel.ClipActions.cs:58` — `Process.Start` failure when revealing clip. State-strip warning.
- `CreditsViewModel.cs:91` — link open failure. State-strip warning.

## Section 3 — Settings dialog (H2 + M2)

**H2 — save failure surfacing.**

`SettingsViewModel` gains:

```csharp
[ObservableProperty] private InlineBannerState? _saveBanner;
```

In `Save()`:

```csharp
if (_appSettings.Save())
{
    if (clampReport.HasAny)
        SaveBanner = InlineBannerState.Info(clampReport.Describe());
    else
        SaveBanner = null;
    SaveCompleted?.Invoke(clampReport);   // payload changes from () to (ClampReport)
}
else
{
    SaveBanner = InlineBannerState.Error(
        "Couldn't save settings. Check folder permissions or try again.",
        retryCommand: SaveCommand);
}
```

`InlineBanner` control is placed at the top of the scroll area in `SettingsWindow.axaml`, bound to `SaveBanner` with `IsVisible="{Binding SaveBanner, Converter={x:Static ObjectConverters.IsNotNull}}"`.

**M2 — clamp text fields.**

Replace lines 449–454 of `SettingsViewModel.cs`:

```csharp
var clampReport = new ClampReport();

settings.ProjectName       = Clamp(ProjectName, 120, "Project name", clampReport);
settings.ProductionCompany = Clamp(ProductionCompany, 120, "Production company", clampReport);
settings.DitName           = Clamp(DitName, 80, "DIT", clampReport);
settings.Director          = Clamp(Director, 80, "Director", clampReport);
settings.Dp                = Clamp(Dp, 80, "DP", clampReport);
```

Helper:

```csharp
private static string? Clamp(string? value, int max, string label, ClampReport report)
{
    if (string.IsNullOrWhiteSpace(value)) return null;
    var trimmed = value.Trim();
    if (trimmed.Length <= max) return trimmed;
    report.Add(label, trimmed.Length, max);
    return trimmed[..max];
}
```

`ClampReport` collects `{Label, OriginalLength, ClampedTo}` entries. `Describe()`:
- 0 entries → null
- 1 entry → *"Project name trimmed to 120 characters"*
- 2+ entries → *"Project name and DP trimmed to fit"* (oxford comma if 3+)

Existing `Math.Clamp(ThumbnailsPerClip, 0, 10)` at line 460 stays as-is.

**Hand-off to MainWindow:**
`SettingsViewModel.SaveCompleted` event signature changes from `Action?` to `Action<ClampReport?>?`. `MainWindow` code-behind subscribes and passes the report to a new `MainWindowViewModel.OnSettingsSaved(ClampReport? clampReport)` method, which hydrates the state strip (Info level) when `clampReport?.HasAny == true`. If `clampReport` is null or empty, no state-strip push happens.

## Section 4 — Hero finish state (M6)

`MainWindowViewModel` adds:

```csharp
private OverlayState _overlayState = OverlayState.Idle;
private IReadOnlyList<string>? _completedReportPaths;
[ObservableProperty] private string? _successLabel;

private static readonly TimeSpan SuccessHoldDelay = TimeSpan.FromMilliseconds(2000);
```

`GenerateReportsAsync` happy path:

```csharp
var outputPaths = await _reportService.GenerateReportsAsync(settings, cts.Token);
_completedReportPaths = outputPaths;
_overlayState = OverlayState.SuccessHold;
SuccessLabel = "Report saved";
State = StateMessage.Success(
    $"Reports saved to {settings.OutputFolder}",
    actionText: "Open folder",
    actionCommand: OpenOutputFolderCommand);
```

`EndOperationAsync` becomes phase-aware:

```csharp
if (_overlayState == OverlayState.SuccessHold)
{
    OverallProgress = 100;
    await Task.Delay(SuccessHoldDelay).ConfigureAwait(true);
}
else
{
    await Task.Delay(EndTailDelay).ConfigureAwait(true);
}

_overlayState = OverlayState.Idle;
SuccessLabel = null;
// existing teardown follows
```

**Overlay XAML changes** in `MainWindow.axaml` — the existing processing overlay panel adds a sibling layer (visible only when `OverlayState == SuccessHold`) showing `SuccessLabel` + two `Button`s (`OpenOutputFolderCommand` / `OpenLastReportCommand`). Clicking either: sets a `_userClosedOverlay = true` flag, which short-circuits the remaining `SuccessHoldDelay`.

## Section 5 — Partial file cleanup (H5)

New method on `IReportGenerationService`:

```csharp
Task CleanupPartialOutputAsync(ReportSettings settings, IReadOnlyList<string> partialPaths);
```

Implementation: deletes the files in `partialPaths` plus any sibling `.tmp` files in the same output folder. Safe when paths are empty. Failures are logged but never thrown — cleanup is best-effort.

`ReportGenerationService` gains a `LastWrittenPaths` property — an `IReadOnlyList<string>` populated incrementally as the service writes HTML, then PDF, then per-reel reports. Reset to empty at the start of each `GenerateReportsAsync` call.

`MainWindowViewModel.Import.cs` cancellation handler:

```csharp
catch (OperationCanceledException)
{
    if (_reportService.LastWrittenPaths is { Count: > 0 } partials)
        await _reportService.CleanupPartialOutputAsync(settings, partials);

    State = StateMessage.Info("Generation cancelled");
}
```

Knowledge of "what got written" stays where the writing happens — the VM only orchestrates.

## Section 6 — Camera-support dismissal (M1) + banner migration

**M1 — persist dismiss.**

`Models/AppSettings.cs` gains:

```csharp
public DateTime? CameraSupportDismissedUntil { get; set; }
```

`AppSettingsJsonContext` is regenerated for the new property.

`CameraSupportToastState.Dismiss()`:

```csharp
public void Dismiss()
{
    _settings.CameraSupportDismissedUntil = DateTime.MaxValue;
    _save(_settings);
    HasMissingCameraSupport = false;
    MissingCameraSupportSummary = string.Empty;
}
```

`IsSnoozed` check widens:

```csharp
private bool IsSnoozed =>
    (_settings.CameraSupportSnoozeUntil is DateTime snooze && snooze > DateTime.Now) ||
    (_settings.CameraSupportDismissedUntil is DateTime dismiss && dismiss > DateTime.Now);
```

`OnStatusChanged` clears `CameraSupportDismissedUntil` when the *previously missing* tools are now installed, so the user isn't permanently hidden from re-discovering changed state. Saves settings on clear.

**Banner-stack migration.**

`Views/MainWindow.axaml` Grid.Row=1 currently has `<Border Height="0"/>` (placeholder). Replace with `<controls:BannerStack Items="{Binding Banners}"/>`. The existing update toast block and the camera-support toast block inside Grid.Row=2 are removed entirely — their state now flows through `BannerItem`s.

`MainWindowViewModel.Update.cs` partial — `OnHasUpdateAvailableChanged` populates a `BannerItem`:

```csharp
partial void OnHasUpdateAvailableChanged(bool value)
{
    if (value)
    {
        Banners.AddOrReplace(new BannerItem
        {
            Key = "update",
            Level = Level.Info,
            Title = $"Luna {UpdateVersion} is available",
            Body = IsDownloadingUpdate ? $"Downloading… {UpdateDownloadProgress}%" : null,
            PrimaryAction = new(IsUpdateReady ? "Restart Now" : "Download",
                                IsUpdateReady ? ApplyUpdateCommand : DownloadUpdateCommand),
            SecondaryAction = new("Later", RemindUpdateLaterCommand),
            IsDismissible = true,
            OnDismiss = DismissUpdateCommand,
        });
    }
    else
    {
        Banners.RemoveByKey("update");
    }
}
```

Same shape for camera-support in `MainWindowViewModel.CameraSupport.cs`, with `Key = "camera-support"`, `Level = Level.Warning`, title *"Camera support missing"*, body *"{MissingCameraSupportSummary} not installed. To decode these formats, install the tools from Settings."* and primary action *"Open Settings"*.

## Section 7 — Copy and wording (Wave 3)

Pure XAML edits except where noted.

| Location | Current | New |
|---|---|---|
| `MainWindow.axaml:212` | *"Drop camera footage here"* | *"Drop a folder of camera footage"* |
| `MainWindow.axaml:213` | *"or click to browse for a folder"* | *"or browse"* |
| New caption below `:217` | — | *"ARRI · BRAW · Sony VENICE · ProRes · H.264/265"* (`LunaTextMuted`, `LunaFontSizeXs`) |
| `MainWindow.axaml:243` | *"video clips found"* | *"video clips ready to scan"* |
| `MainWindow.axaml:255` button | *"Create Report"* | *"Scan & Continue"* |
| `MainWindow.axaml:261` hint | *"Clicking 'Create Report' will extract metadata and thumbnails from your footage"* | *"We'll read metadata and generate thumbnails. You'll review before exporting."* |
| `MainWindow.axaml:497` "Clear & Start Over" | fires immediately | Confirmation dialog: *"Clear all loaded reels and start over?"* with Cancel / Clear |
| `SettingsWindow.axaml:103-112` | *"DIT" / "Director" / "DP"* | Append *" (Optional)"* in `LunaTextMuted` next to each label |
| `SettingsWindow.axaml:177` | *"Which camera families Luna can read on this install. Proprietary formats arrive through future releases; nothing to configure here."* | *"Install vendor SDKs to enable proprietary camera formats. Luna detects them automatically."* |
| `SettingsViewModel.cs:283` detect-fail | *"Still not detected. Make sure the installer finished — if it's still running, wait for it. If it's done and Luna can't find the install, restart Luna."* | *"Still not detected. Finish the vendor installer, then try Detect again. If it keeps failing, restart Luna."* |

The camera-support banner color change (was `LunaAccent`, now `LunaWarning`) is handled automatically by setting `Level = Level.Warning` on the `BannerItem` in Section 6 — `BannerStack` resolves the color from the level. No separate color swap needed.

**Thumbnail failure suffixes** — applied where `ThumbnailIssueSummary` is built in `Models/CameraClip.cs:73-80`:

| Outcome | New text |
|---|---|
| `NoDecoder` | *"Frames unavailable — install vendor support in Settings."* |
| `SeekFailed` | *"Frames unavailable — the file may still be copying. Try again after copy completes."* |
| `DecodeFailed` | *"Frames unavailable — decoder couldn't process this file."* |
| `ContainerOpenFailed` | *"Frames unavailable — couldn't open this file. It may be corrupted or in use."* |

The `NoDecoder` summary becomes clickable in the clip-row tooltip — click routes to Settings via `OpenSettingsForCameraSupportCommand`. Implemented via a `HyperlinkButton` (or styled `Button`) in the row template.

## Section 8 — FFmpeg cancellation (H3)

Single targeted edit inside `FfmpegThumbnailService.ExtractThumbnailsInternal`. At the top of the per-position decode while-loop (`Services/Chappie/FfmpegThumbnailService.cs:446`):

```csharp
while (!frameDecoded && attempts < MaxDecodeAttemptsPerPosition)
{
    cancellationToken.ThrowIfCancellationRequested();   // NEW
    attempts++;
    ffmpeg.av_packet_unref(pPacket);
    // ... existing decode logic ...
}
```

Reorder catches at line 516 so `OperationCanceledException` propagates rather than turning into a `DecodeFailed` result:

```csharp
catch (OperationCanceledException)
{
    throw;
}
catch (Exception ex)
{
    Log.Debug(ex, "FFmpeg extraction failed for {FileName}", fileName);
    return ThumbnailResult.DecodeFailed(ex.Message);
}
```

The `finally` block still runs and cleans the native pointers — no resource-leak risk added by the early throw.

## Section 9 — Backlog (Wave 5)

| # | Change | File |
|---|---|---|
| M3 | `ReportNamePattern` token allow-list. Regex validates `{project|reel|date|time}` only. On `Save()`, if pattern contains other tokens, `ClampReport` records *"Report name pattern reset to default"* and the field reverts to the default value | `Models/ReportSettings.cs`, `ViewModels/SettingsViewModel.cs` |
| M4 | Debounce `RebuildFilteredReels`. `DispatcherTimer` with 300 ms interval; reset on every `OnSearchTextChanged`. Tick fires the rebuild | `ViewModels/MainWindowViewModel.cs:209-234` |
| M5 | Auto-fill collision. Replace `_autoFilledReportName` string match with `_userEditedReportName` bool. `_settingAutoFilledReportName` guard flag ensures programmatic assignment in `QuickScanFolderAsync` doesn't trip the user-edited flag in `OnReportNameChanged` | `ViewModels/MainWindowViewModel.Import.cs:88-93`, `ViewModels/MainWindowViewModel.cs` |
| M10 | ETA smoothing. Suppress ETA display until 2 items have been processed (`EtaText = null`). After that, use moving average: `msPerItem = oldAvg * 0.7 + newSample * 0.3` | `ViewModels/MainWindowViewModel.cs:389-408` |
| M11 | Windows-only guard. Wrap `yield return @"C:\Program Files\..."` in `if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))` | `Services/Chappie/SonyRawViewerLocator.cs:70` |
| L1 | Disable cancel during tail. New `_isInTail` flag set at the start of `EndOperationAsync`; `CanCancel` computed property factors it in. `CancelProcessing` guards `cts.Cancel()` with null check and `IsCancellationRequested` check (already partially in place) | `ViewModels/MainWindowViewModel.Import.cs:181-198, 208-236` |
| L2 | Atomic `AppSettings.Save()`. Write to `settings.json.tmp` then `File.Move(tmp, real, overwrite: true)`. Wrapped in try/catch — failure returns false (existing contract). On success, removes the tmp implicitly via `File.Move` | `Models/AppSettings.cs` |

## Section 10 — Implementation order

The work splits into three phases. Phases A and B must land in order; phase C items are independent.

**Phase A: Foundation**
1. `ExceptionMapper` + `StateMessage` record (`Services/ExceptionMapper.cs`)
2. `StateStrip` control + `StateStripVm` (`Views/Controls/StateStrip.axaml`, ViewModel folder)
3. `BannerStack` control + `BannerItem` + `BannerStackVm` + `Banners` collection on `MainWindowViewModel`
4. `InlineBanner` control + `InlineBannerState`
5. `OverlayState` enum + processing overlay hero-finish XAML extension

**Phase B: Behaviour fixes using the foundation**
6. H1 — exception mapping at call sites
7. H2 — Settings save inline banner
8. M2 — `ClampReport` + clamping helper
9. H5 — partial file cleanup + `LastWrittenPaths`
10. M1 — `CameraSupportDismissedUntil` persistence
11. M6 — hero success state population + `GenerateReportsAsync` happy path
12. Convert `MainWindowViewModel.Update.cs` to push `BannerItem`s
13. Convert `MainWindowViewModel.CameraSupport.cs` to push `BannerItem`s
14. Wave 3 — XAML copy edits + thumbnail failure suffixes + `LunaWarning` color swap

**Phase C: Independent**
15. H3 — FFmpeg cancellation token in decode loop
16. Wave 5 backlog: M3, M4, M5, M10, M11, L1, L2

Manual smoke test between A and B. Manual smoke test of the full happy path (drop folder → scan → generate → success hold → state strip) at the end of B. Phase C items can be small individual commits.

## Open questions

None — all design choices were resolved during the brainstorming session on 2026-05-24.
