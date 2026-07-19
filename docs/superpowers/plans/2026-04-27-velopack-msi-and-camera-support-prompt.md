# Velopack MSI + Camera-Support Prompt — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Switch Luna's Windows installer to a per-user Velopack MSI with branded install progress, add a top-right toast that points to Settings → Camera Support when ARRI / Sony tooling is missing, and lock down the self-contained runtime guarantee with new CI assertions.

**Architecture:** Build pipeline (`build.ps1` + `.github/workflows/release.yml`) gains `--msi --instLocation PerUser --splashImage --icon` for `vpk pack`. A new `CameraSupportInstallationStatus` singleton probes `ArtCliLocator` + `SonyRawViewerLocator` and exposes a list of missing tools; `MainWindowViewModel.CameraSupport.cs` (new partial) drives a top-right toast that opens the existing Settings → Camera Support panel via the existing `OpenSettingsRequested` event. The Settings install flow already has full UX (progress, errors, Detect button) — we don't duplicate it; we just call `Invalidate()` on the new service after a successful install so the toast auto-hides.

**Tech Stack:** .NET 10, Avalonia 11.3.x, CommunityToolkit.Mvvm, Velopack 0.0.1298 (vpk CLI ships WiX 5), xUnit (new test project), Serilog.

**Spec:** `docs/superpowers/specs/2026-04-27-velopack-msi-and-camera-support-prompt-design.md`

---

## File Structure

**New files:**
- `LunaApp.Tests/LunaApp.Tests.csproj` — xUnit test project for the new code.
- `LunaApp.Tests/CameraSupportInstallationStatusTests.cs` — service unit tests.
- `LunaApp.Tests/CameraSupportToastViewModelTests.cs` — toast-state unit tests.
- `LunaApp.Tests/Fakes/FakeArtCliLocator.cs` — test double.
- `LunaApp.Tests/Fakes/FakeSonyRawViewerLocator.cs` — test double.
- `LunaApp.Tests/Fakes/FakeCameraSupport.cs` — minimal `ICameraSupport` stub.
- `Services/CameraSupport/CameraSupportInstallationStatus.cs` — probe service.
- `ViewModels/MainWindowViewModel.CameraSupport.cs` — toast state + commands (partial).
- `Assets/install-splash.png` — 640×400 PNG derived from `Assets/luna-logo-lg.webp`.

**Modified files:**
- `Models/AppSettings.cs` — add `CameraSupportSnoozeUntil` field.
- `LunaApp.sln` — add test project entries.
- `LunaApp.csproj` — make `CameraSupportInstallationStatus` reachable from test project (`InternalsVisibleTo`); make existing locators non-sealed only if needed for fakes — they're already used via composition so we override via interfaces. *(No csproj test changes expected; if needed they appear in the relevant tasks.)*
- `ServiceRegistration.cs` — register `CameraSupportInstallationStatus` singleton.
- `ViewModels/SettingsViewModel.cs` — inject `CameraSupportInstallationStatus`, call `Invalidate()` after a successful install in `RunInstallCoreAsync` and after a successful re-detect in `DetectAsync`.
- `ViewModels/MainWindowViewModel.cs` — inject `CameraSupportInstallationStatus`, call `SubscribeToCameraSupport()` from constructor.
- `Views/MainWindow.axaml` — add the camera-support toast block beneath the update toast.
- `build.ps1` — extend `vpk pack` invocation for `win-x64` (MSI flags) and `osx*` (icon flag); add CI verification gates.
- `.github/workflows/release.yml` — same flag updates and verification gates as `build.ps1`; update release notes body to reference `.msi` and adjust upload path.
- `docs/INSTALLATION.md` — switch instructions from `Setup.exe` to `.msi`.

**Renames / deletions:** none.

---

## Task 1: Bootstrap xUnit test project

**Files:**
- Create: `LunaApp.Tests/LunaApp.Tests.csproj`
- Create: `LunaApp.Tests/Usings.cs`
- Create: `LunaApp.Tests/SmokeTest.cs`
- Modify: `LunaApp.sln`
- Modify: `LunaApp.csproj`

- [ ] **Step 1: Create the test csproj**

`LunaApp.Tests/LunaApp.Tests.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <IsPackable>false</IsPackable>
    <IsTestProject>true</IsTestProject>
    <!-- Tests don't need trimming; trimming the production app stays on. -->
    <PublishTrimmed>false</PublishTrimmed>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.11.1" />
    <PackageReference Include="xunit" Version="2.9.2" />
    <PackageReference Include="xunit.runner.visualstudio" Version="2.8.2" />
  </ItemGroup>

  <ItemGroup>
    <ProjectReference Include="..\LunaApp.csproj" />
  </ItemGroup>
</Project>
```

- [ ] **Step 2: Add the global usings file**

`LunaApp.Tests/Usings.cs`:

```csharp
global using Xunit;
```

- [ ] **Step 3: Add a smoke test**

`LunaApp.Tests/SmokeTest.cs`:

```csharp
namespace LunaApp.Tests;

public class SmokeTest
{
    [Fact]
    public void Truth() => Assert.True(true);
}
```

- [ ] **Step 4: Wire the test project into the solution**

Modify `LunaApp.sln`. After the existing `Project("…") = "LunaApp", "LunaApp.csproj", …` block (line 5-6), add:

```
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "LunaApp.Tests", "LunaApp.Tests\LunaApp.Tests.csproj", "{B12A1E5F-3F25-4C8F-9E1A-2B7B1D9E5E12}"
EndProject
```

In the `GlobalSection(ProjectConfigurationPlatforms) = postSolution` block, add:

```
{B12A1E5F-3F25-4C8F-9E1A-2B7B1D9E5E12}.Debug|Any CPU.ActiveCfg = Debug|Any CPU
{B12A1E5F-3F25-4C8F-9E1A-2B7B1D9E5E12}.Debug|Any CPU.Build.0 = Debug|Any CPU
{B12A1E5F-3F25-4C8F-9E1A-2B7B1D9E5E12}.Release|Any CPU.ActiveCfg = Release|Any CPU
{B12A1E5F-3F25-4C8F-9E1A-2B7B1D9E5E12}.Release|Any CPU.Build.0 = Release|Any CPU
```

(The GUID is freshly generated; any unique GUID is fine — keep it consistent across both blocks.)

- [ ] **Step 5: Make production internals visible to tests**

Modify `LunaApp.csproj`. Inside the existing top-level `<PropertyGroup>` (after line 41 `<Copyright>…</Copyright>`), add:

```xml
<InternalsVisibleTo>LunaApp.Tests</InternalsVisibleTo>
```

This lets test fakes touch `internal` types without us widening visibility for the rest of the world.

- [ ] **Step 6: Restore + build**

Run: `dotnet build LunaApp.sln -c Debug`
Expected: builds successfully, two projects compiled.

- [ ] **Step 7: Run the smoke test**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj -c Debug`
Expected: `Passed: 1`.

- [ ] **Step 8: Commit**

```bash
git add LunaApp.sln LunaApp.csproj LunaApp.Tests/
git commit -m "test: bootstrap xunit test project for camera-support work"
```

---

## Task 2: Add `CameraSupportSnoozeUntil` to `AppSettings`

**Files:**
- Modify: `Models/AppSettings.cs`
- Test: `LunaApp.Tests/AppSettingsTests.cs`

- [ ] **Step 1: Write the failing test**

`LunaApp.Tests/AppSettingsTests.cs`:

```csharp
using System.Text.Json;
using LunaApp.Models;

namespace LunaApp.Tests;

public class AppSettingsTests
{
    [Fact]
    public void CameraSupportSnoozeUntil_IsNullByDefault()
    {
        var settings = new AppSettings();
        Assert.Null(settings.CameraSupportSnoozeUntil);
    }

    [Fact]
    public void CameraSupportSnoozeUntil_RoundTripsThroughJson()
    {
        var when = new DateTime(2026, 4, 27, 14, 30, 0, DateTimeKind.Local);
        var settings = new AppSettings { CameraSupportSnoozeUntil = when };

        // Use the same options the source-gen context uses so tests verify the
        // real serialization path, not a reflection-based one.
        var json = JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = true });
        var restored = JsonSerializer.Deserialize<AppSettings>(json);

        Assert.NotNull(restored);
        Assert.Equal(when, restored!.CameraSupportSnoozeUntil);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj --filter "FullyQualifiedName~AppSettingsTests"`
Expected: FAIL — `CameraSupportSnoozeUntil` doesn't exist on `AppSettings`.

- [ ] **Step 3: Add the property**

Modify `Models/AppSettings.cs`. After the existing `UpdateSnoozeUntil` block (lines 35-39), add:

```csharp
    /// <summary>
    /// When non-null, the camera-support toast is suppressed until this date.
    /// Set by the user choosing "Remind me later" on the toast. Mirrors the
    /// shape of <see cref="UpdateSnoozeUntil"/>.
    /// </summary>
    public DateTime? CameraSupportSnoozeUntil { get; set; }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj --filter "FullyQualifiedName~AppSettingsTests"`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Models/AppSettings.cs LunaApp.Tests/AppSettingsTests.cs
git commit -m "feat(settings): add CameraSupportSnoozeUntil for toast snooze"
```

---

## Task 3: Add fakes for the camera-support locators

**Files:**
- Create: `LunaApp.Tests/Fakes/FakeArtCliLocator.cs`
- Create: `LunaApp.Tests/Fakes/FakeSonyRawViewerLocator.cs`
- Create: `LunaApp.Tests/Fakes/FakeCameraSupport.cs`

The production `ArtCliLocator` and `SonyRawViewerLocator` are sealed concrete classes that probe the disk. To unit-test the new probe service without disk I/O, we wrap them behind tiny abstractions used only by the new service. The existing call sites continue to use the concrete locators directly — we don't widen their interfaces or rewrite their consumers.

- [ ] **Step 1: Add a fake-friendly seam in `CameraSupportInstallationStatus`**

Note: this task only creates the fakes; the seam itself is introduced in Task 4 when the service is written. The fakes here implement two tiny test-only interfaces that the service consumes:

```csharp
// Defined in Task 4 inside Services/CameraSupport/CameraSupportInstallationStatus.cs
internal interface IArtCliInstallProbe { bool IsInstalled { get; } }
internal interface ISonyRawViewerInstallProbe { bool IsInstalled { get; } bool IsSupportedOnThisOs { get; } }
```

- [ ] **Step 2: Create the fakes**

`LunaApp.Tests/Fakes/FakeArtCliLocator.cs`:

```csharp
using LunaApp.Services.CameraSupport;

namespace LunaApp.Tests.Fakes;

internal sealed class FakeArtCliInstallProbe : IArtCliInstallProbe
{
    public bool IsInstalled { get; set; }
}
```

`LunaApp.Tests/Fakes/FakeSonyRawViewerLocator.cs`:

```csharp
using LunaApp.Services.CameraSupport;

namespace LunaApp.Tests.Fakes;

internal sealed class FakeSonyRawViewerInstallProbe : ISonyRawViewerInstallProbe
{
    public bool IsInstalled { get; set; }
    public bool IsSupportedOnThisOs { get; set; } = true;
}
```

`LunaApp.Tests/Fakes/FakeCameraSupport.cs`:

```csharp
using LunaApp.Models;
using LunaApp.Services.CameraSupport;

namespace LunaApp.Tests.Fakes;

internal sealed class FakeCameraSupport : ICameraSupport
{
    public required string Id { get; init; }
    public required string DisplayName { get; init; }
    public IReadOnlySet<string> HandledExtensions { get; init; } = new HashSet<string>();
    public required SupportStatus Status { get; init; }
    public bool CanHandle(string filePath) => false;
    public Task<CameraClip> ProcessAsync(string filePath, bool extractThumbnails, int thumbnailCount, int thumbnailWidth, CancellationToken cancellationToken)
        => throw new NotImplementedException();
}
```

- [ ] **Step 3: Build to confirm the fakes compile**

Run: `dotnet build LunaApp.Tests/LunaApp.Tests.csproj`
Expected: build fails — `IArtCliInstallProbe` / `ISonyRawViewerInstallProbe` not yet defined.

This is intentional: Task 4 creates them. The fakes are committed alongside the service in Task 4 to keep the build green.

- [ ] **Step 4: Stage but don't commit yet**

Don't commit at this step — the fakes only build once Task 4 lands. Move on to Task 4.

---

## Task 4: Implement `CameraSupportInstallationStatus` service

**Files:**
- Create: `Services/CameraSupport/CameraSupportInstallationStatus.cs`
- Test: `LunaApp.Tests/CameraSupportInstallationStatusTests.cs`

- [ ] **Step 1: Write the failing tests**

`LunaApp.Tests/CameraSupportInstallationStatusTests.cs`:

```csharp
using LunaApp.Services.CameraSupport;
using LunaApp.Tests.Fakes;

namespace LunaApp.Tests;

public class CameraSupportInstallationStatusTests
{
    private static (CameraSupportInstallationStatus svc,
                    FakeArtCliInstallProbe arri,
                    FakeSonyRawViewerInstallProbe sony) Build(
        bool arriInstalled = false,
        bool sonyInstalled = false,
        bool sonySupportedOnOs = true)
    {
        var arri = new FakeArtCliInstallProbe { IsInstalled = arriInstalled };
        var sony = new FakeSonyRawViewerInstallProbe
        {
            IsInstalled = sonyInstalled,
            IsSupportedOnThisOs = sonySupportedOnOs,
        };

        var supports = new ICameraSupport[]
        {
            new FakeCameraSupport
            {
                Id = "arri",
                DisplayName = "ARRI ALEXA",
                Status = arriInstalled
                    ? new SupportStatus.Ready("art-cmd 1.0.0", "installed")
                    : new SupportStatus.ComingLater("install ART CLI"),
            },
            new FakeCameraSupport
            {
                Id = "sony-venice",
                DisplayName = "Sony Venice / Burano / FX9",
                Status = sonyInstalled
                    ? new SupportStatus.Ready("rawexporter 5.3", "detected")
                    : new SupportStatus.ComingLater("install Sony RAW Viewer"),
            },
            new FakeCameraSupport
            {
                Id = "blackmagic",
                DisplayName = "Blackmagic RAW",
                // Blackmagic has no installer wired — service must skip it.
                Status = new SupportStatus.ComingLater("license-blocked"),
            },
        };

        var registry = new CameraSupportRegistry(supports);
        var svc = new CameraSupportInstallationStatus(registry, arri, sony);
        return (svc, arri, sony);
    }

    [Fact]
    public void Returns_empty_when_both_tools_installed()
    {
        var (svc, _, _) = Build(arriInstalled: true, sonyInstalled: true);
        Assert.Empty(svc.ResolveMissing());
    }

    [Fact]
    public void Returns_arri_when_art_cli_missing()
    {
        var (svc, _, _) = Build(arriInstalled: false, sonyInstalled: true);
        var missing = svc.ResolveMissing();
        Assert.Single(missing);
        Assert.Equal("arri", missing[0].Id);
        Assert.Equal("ARRI ALEXA", missing[0].DisplayName);
    }

    [Fact]
    public void Returns_sony_when_raw_viewer_missing_on_windows()
    {
        var (svc, _, _) = Build(arriInstalled: true, sonyInstalled: false, sonySupportedOnOs: true);
        var missing = svc.ResolveMissing();
        Assert.Single(missing);
        Assert.Equal("sony-venice", missing[0].Id);
    }

    [Fact]
    public void Skips_sony_on_unsupported_os()
    {
        var (svc, _, _) = Build(arriInstalled: true, sonyInstalled: false, sonySupportedOnOs: false);
        Assert.Empty(svc.ResolveMissing());
    }

    [Fact]
    public void Skips_blackmagic_no_installer_wired()
    {
        var (svc, _, _) = Build(arriInstalled: true, sonyInstalled: true);
        // Blackmagic is registered but has no probe — service must not include it
        // even though its status is ComingLater.
        Assert.DoesNotContain(svc.ResolveMissing(), m => m.Id == "blackmagic");
    }

    [Fact]
    public void Invalidate_raises_status_changed()
    {
        var (svc, _, _) = Build(arriInstalled: false);
        var raised = false;
        svc.StatusChanged += (_, _) => raised = true;
        svc.Invalidate();
        Assert.True(raised);
    }

    [Fact]
    public void Invalidate_re_evaluates_state()
    {
        var (svc, arri, _) = Build(arriInstalled: false, sonyInstalled: true);
        Assert.Single(svc.ResolveMissing());

        arri.IsInstalled = true; // simulate post-install
        svc.Invalidate();

        Assert.Empty(svc.ResolveMissing());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj --filter "FullyQualifiedName~CameraSupportInstallationStatusTests"`
Expected: build error — `CameraSupportInstallationStatus`, `IArtCliInstallProbe`, `ISonyRawViewerInstallProbe` undefined.

- [ ] **Step 3: Implement the service + probe interfaces + concrete adapters**

Create `Services/CameraSupport/CameraSupportInstallationStatus.cs`:

```csharp
using System.Runtime.InteropServices;
using LunaApp.Services.Chappie;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// Probes whether each camera-support tool that ships with an installer is
/// currently installed, and exposes the missing ones so the main window can
/// show a "camera support missing" toast.
///
/// The service is intentionally a thin probe — it does not run installs.
/// Settings → Camera Support already has the full install UX
/// (<see cref="LunaApp.ViewModels.CameraSupportRow"/>); this service tells
/// the toast which entries to surface and re-evaluates state when
/// <see cref="Invalidate"/> is called after a successful install.
/// </summary>
public sealed class CameraSupportInstallationStatus
{
    private readonly CameraSupportRegistry _registry;
    private readonly IArtCliInstallProbe _arriProbe;
    private readonly ISonyRawViewerInstallProbe _sonyProbe;

    public CameraSupportInstallationStatus(
        CameraSupportRegistry registry,
        IArtCliInstallProbe arriProbe,
        ISonyRawViewerInstallProbe sonyProbe)
    {
        _registry = registry;
        _arriProbe = arriProbe;
        _sonyProbe = sonyProbe;
    }

    public sealed record MissingSupport(string Id, string DisplayName);

    public event EventHandler? StatusChanged;

    /// <summary>
    /// Returns the camera-support entries that ship with an installer but
    /// aren't installed yet. Blackmagic is excluded — no installer is wired.
    /// Sony is excluded on platforms where the locator can't probe (macOS).
    /// </summary>
    public IReadOnlyList<MissingSupport> ResolveMissing()
    {
        var result = new List<MissingSupport>(2);

        var arri = _registry.All.FirstOrDefault(s => s.Id == "arri");
        if (arri is not null && !_arriProbe.IsInstalled)
            result.Add(new MissingSupport(arri.Id, arri.DisplayName));

        var sony = _registry.All.FirstOrDefault(s => s.Id == "sony-venice");
        if (sony is not null && _sonyProbe.IsSupportedOnThisOs && !_sonyProbe.IsInstalled)
            result.Add(new MissingSupport(sony.Id, sony.DisplayName));

        return result;
    }

    public void Invalidate() =>
        StatusChanged?.Invoke(this, EventArgs.Empty);
}

internal interface IArtCliInstallProbe
{
    bool IsInstalled { get; }
}

internal interface ISonyRawViewerInstallProbe
{
    bool IsInstalled { get; }
    bool IsSupportedOnThisOs { get; }
}

/// <summary>Adapter over the production <see cref="ArtCliLocator"/>.</summary>
internal sealed class ArtCliInstallProbe : IArtCliInstallProbe
{
    private readonly ArtCliLocator _locator;
    public ArtCliInstallProbe(ArtCliLocator locator) => _locator = locator;
    public bool IsInstalled => _locator.Resolve(forceRefresh: true) is not null;
}

/// <summary>Adapter over the production <see cref="SonyRawViewerLocator"/>.</summary>
internal sealed class SonyRawViewerInstallProbe : ISonyRawViewerInstallProbe
{
    private readonly SonyRawViewerLocator _locator;
    public SonyRawViewerInstallProbe(SonyRawViewerLocator locator) => _locator = locator;
    public bool IsInstalled => _locator.Resolve(forceRefresh: true) is not null;

    // Locator only probes on Windows today (see SonyRawViewerLocator.Probe).
    public bool IsSupportedOnThisOs =>
        RuntimeInformation.IsOSPlatform(OSPlatform.Windows);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj --filter "FullyQualifiedName~CameraSupportInstallationStatusTests"`
Expected: all 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add Services/CameraSupport/CameraSupportInstallationStatus.cs LunaApp.Tests/CameraSupportInstallationStatusTests.cs LunaApp.Tests/Fakes/
git commit -m "feat(camera-support): add CameraSupportInstallationStatus probe service"
```

---

## Task 5: Register service + wire `Invalidate()` from Settings

**Files:**
- Modify: `ServiceRegistration.cs`
- Modify: `ViewModels/SettingsViewModel.cs`

- [ ] **Step 1: Register the service + probes in DI**

Modify `ServiceRegistration.cs`. After line 46 (`services.AddSingleton<CameraSupportRegistry>();`), add:

```csharp
        // Camera-support installation probes — used by the main-window toast
        // to decide whether to nudge the user to install ARRI/Sony tooling.
        services.AddSingleton<IArtCliInstallProbe, ArtCliInstallProbe>();
        services.AddSingleton<ISonyRawViewerInstallProbe, SonyRawViewerInstallProbe>();
        services.AddSingleton<CameraSupportInstallationStatus>();
```

- [ ] **Step 2: Inject the service into `SettingsViewModel`**

Modify `ViewModels/SettingsViewModel.cs`:

After line 83 (`private readonly CameraSupportRegistry _cameraSupports;`), add:

```csharp
    private readonly CameraSupportInstallationStatus _installStatus;
```

Update the constructor signature (line 191-196) to include the new dependency:

```csharp
    public SettingsViewModel(
        CameraSupportRegistry cameraSupports,
        ArtCliInstaller artCliInstaller,
        ArtCliLocator artCliLocator,
        SonyRawViewerInstaller sonyInstaller,
        SonyRawViewerLocator sonyLocator,
        CameraSupportInstallationStatus installStatus)
    {
        _artCliInstaller = artCliInstaller;
        _artCliLocator = artCliLocator;
        _sonyInstaller = sonyInstaller;
        _sonyLocator = sonyLocator;
        _cameraSupports = cameraSupports;
        _installStatus = installStatus;
        _appSettings = AppSettings.Load();
        ...
```

- [ ] **Step 3: Call `Invalidate()` after a successful install**

In `RunInstallCoreAsync` (around line 363), after `support.Status is SupportStatus.Ready` flips a row to Ready, fire the invalidate. Replace the existing `if (support.Status is SupportStatus.Ready)` block (lines 363-374) with:

```csharp
            if (support.Status is SupportStatus.Ready)
            {
                // ART CLI path: zip extract is fully done, locator finds it.
                row.State = SupportStateLabel(support.Status);
                row.Summary = SupportSummary(support.Status);
                _installStatus.Invalidate();
            }
            else
            {
                // Sony path: installer launched but binaries aren't on disk
                // yet. Hand off to the Detect button + helpful message.
                row.IsAwaitingDetect = true;
            }
```

In `DetectAsync` (around line 270), add the same call when the Sony row flips to Ready. Replace the `if (support.Status is SupportStatus.Ready)` block (lines 270-276) with:

```csharp
        if (support.Status is SupportStatus.Ready)
        {
            row.State = SupportStateLabel(support.Status);
            row.Summary = SupportSummary(support.Status);
            row.IsAwaitingDetect = false;
            row.InstallError = null;
            _installStatus.Invalidate();
        }
```

- [ ] **Step 4: Build to confirm DI resolves**

Run: `dotnet build LunaApp.sln -c Debug`
Expected: builds successfully.

- [ ] **Step 5: Commit**

```bash
git add ServiceRegistration.cs ViewModels/SettingsViewModel.cs
git commit -m "feat(settings): invalidate camera-support status on install/detect"
```

---

## Task 6: Add `MainWindowViewModel.CameraSupport.cs` partial

**Files:**
- Create: `ViewModels/MainWindowViewModel.CameraSupport.cs`
- Modify: `ViewModels/MainWindowViewModel.cs`
- Test: `LunaApp.Tests/CameraSupportToastViewModelTests.cs`

- [ ] **Step 1: Write the failing tests**

`LunaApp.Tests/CameraSupportToastViewModelTests.cs`:

```csharp
using LunaApp.Models;
using LunaApp.Services.CameraSupport;
using LunaApp.ViewModels;
using LunaApp.Tests.Fakes;

namespace LunaApp.Tests;

public class CameraSupportToastViewModelTests
{
    private static (CameraSupportToastState vm,
                    FakeArtCliInstallProbe arri,
                    FakeSonyRawViewerInstallProbe sony,
                    AppSettings settings) Build(
        bool arriInstalled = false,
        bool sonyInstalled = false,
        DateTime? snoozeUntil = null)
    {
        var arri = new FakeArtCliInstallProbe { IsInstalled = arriInstalled };
        var sony = new FakeSonyRawViewerInstallProbe { IsInstalled = sonyInstalled, IsSupportedOnThisOs = true };

        var supports = new ICameraSupport[]
        {
            new FakeCameraSupport { Id = "arri", DisplayName = "ARRI Reference Tool",
                Status = arriInstalled ? new SupportStatus.Ready("v", "p") : new SupportStatus.ComingLater("install") },
            new FakeCameraSupport { Id = "sony-venice", DisplayName = "Sony RAW Viewer",
                Status = sonyInstalled ? new SupportStatus.Ready("v", "p") : new SupportStatus.ComingLater("install") },
        };

        var registry = new CameraSupportRegistry(supports);
        var status = new CameraSupportInstallationStatus(registry, arri, sony);
        var settings = new AppSettings { CameraSupportSnoozeUntil = snoozeUntil };
        var vm = new CameraSupportToastState(status, settings);
        vm.Refresh();
        return (vm, arri, sony, settings);
    }

    [Fact]
    public void Toast_hidden_when_no_tools_missing()
    {
        var (vm, _, _, _) = Build(arriInstalled: true, sonyInstalled: true);
        Assert.False(vm.HasMissingCameraSupport);
        Assert.Empty(vm.MissingCameraSupportSummary);
    }

    [Fact]
    public void Summary_lists_single_missing_tool()
    {
        var (vm, _, _, _) = Build(arriInstalled: false, sonyInstalled: true);
        Assert.True(vm.HasMissingCameraSupport);
        Assert.Equal("ARRI Reference Tool", vm.MissingCameraSupportSummary);
    }

    [Fact]
    public void Summary_joins_multiple_missing_tools()
    {
        var (vm, _, _, _) = Build(arriInstalled: false, sonyInstalled: false);
        Assert.True(vm.HasMissingCameraSupport);
        Assert.Equal("ARRI Reference Tool, Sony RAW Viewer", vm.MissingCameraSupportSummary);
    }

    [Fact]
    public void Toast_hides_when_status_changes_to_empty()
    {
        var (vm, arri, _, _) = Build(arriInstalled: false, sonyInstalled: true);
        Assert.True(vm.HasMissingCameraSupport);

        arri.IsInstalled = true;
        vm.OnStatusChanged(); // simulate StatusChanged event handler

        Assert.False(vm.HasMissingCameraSupport);
        Assert.Empty(vm.MissingCameraSupportSummary);
    }

    [Fact]
    public void Toast_suppressed_while_snoozed()
    {
        var (vm, _, _, _) = Build(arriInstalled: false, snoozeUntil: DateTime.Now.AddDays(1));
        Assert.False(vm.HasMissingCameraSupport);
    }

    [Fact]
    public void RemindLater_sets_three_day_snooze()
    {
        var (vm, _, _, settings) = Build(arriInstalled: false);
        var before = DateTime.Now;
        vm.RemindLater();
        Assert.NotNull(settings.CameraSupportSnoozeUntil);
        var delta = settings.CameraSupportSnoozeUntil!.Value - before;
        Assert.True(delta.TotalDays is > 2.9 and < 3.1);
        Assert.False(vm.HasMissingCameraSupport);
    }

    [Fact]
    public void Dismiss_hides_for_session_without_persisting()
    {
        var (vm, _, _, settings) = Build(arriInstalled: false);
        Assert.True(vm.HasMissingCameraSupport);
        vm.Dismiss();
        Assert.False(vm.HasMissingCameraSupport);
        Assert.Null(settings.CameraSupportSnoozeUntil);
    }
}
```

The tests target a small testable seam (`CameraSupportToastState`) instead of the full `MainWindowViewModel`, which has dependencies (`ReportGenerationService`, `UpdateService`) that are awkward to construct in unit tests. The partial wires that seam into the ViewModel.

- [ ] **Step 2: Run tests to verify they fail**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj --filter "FullyQualifiedName~CameraSupportToastViewModelTests"`
Expected: build error — `CameraSupportToastState` undefined.

- [ ] **Step 3: Create the partial + state class**

Create `ViewModels/MainWindowViewModel.CameraSupport.cs`:

```csharp
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using LunaApp.Services.CameraSupport;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Camera-support "missing tool" toast concerns of <see cref="MainWindowViewModel"/>.
/// Mirrors the shape of <c>MainWindowViewModel.Update.cs</c>: a small bit of
/// observable state, snooze persistence on AppSettings, and three commands
/// (Open Settings / Later / Dismiss). The actual install UX lives in
/// Settings → Camera Support; we don't duplicate it here.
/// </summary>
public partial class MainWindowViewModel
{
    private CameraSupportToastState? _cameraSupportToast;

    [ObservableProperty] private bool _hasMissingCameraSupport;
    [ObservableProperty] private string _missingCameraSupportSummary = string.Empty;

    private void SubscribeToCameraSupport(CameraSupportInstallationStatus status)
    {
        _cameraSupportToast = new CameraSupportToastState(status, _appSettings);
        _cameraSupportToast.PropertyChanged += (_, e) =>
        {
            Avalonia.Threading.Dispatcher.UIThread.Post(() =>
            {
                if (e.PropertyName == nameof(CameraSupportToastState.HasMissingCameraSupport))
                    HasMissingCameraSupport = _cameraSupportToast.HasMissingCameraSupport;
                else if (e.PropertyName == nameof(CameraSupportToastState.MissingCameraSupportSummary))
                    MissingCameraSupportSummary = _cameraSupportToast.MissingCameraSupportSummary;
            });
        };
        status.StatusChanged += (_, _) => _cameraSupportToast.OnStatusChanged();
        // Initial probe — off the UI thread because the production probes
        // hit disk and may shell out to --version.
        Task.Run(() => _cameraSupportToast.Refresh());
    }

    [RelayCommand]
    private void OpenSettingsForCameraSupport()
    {
        // Reuse the existing settings open path — same code-behind handles it.
        OpenSettingsRequested?.Invoke();
    }

    [RelayCommand]
    private void RemindCameraSupportLater()
    {
        _cameraSupportToast?.RemindLater();
        Log.Information("Camera-support toast snoozed until {Until}", _appSettings.CameraSupportSnoozeUntil);
    }

    [RelayCommand]
    private void DismissCameraSupport() =>
        _cameraSupportToast?.Dismiss();
}

/// <summary>
/// Observable state for the camera-support toast. Lives outside the
/// <see cref="MainWindowViewModel"/> so it can be unit-tested without
/// needing to construct the full shell view-model.
/// </summary>
public sealed class CameraSupportToastState : ObservableObject
{
    private readonly CameraSupportInstallationStatus _status;
    private readonly AppSettings _settings;
    private bool _dismissedThisSession;

    private bool _hasMissingCameraSupport;
    public bool HasMissingCameraSupport
    {
        get => _hasMissingCameraSupport;
        private set => SetProperty(ref _hasMissingCameraSupport, value);
    }

    private string _missingCameraSupportSummary = string.Empty;
    public string MissingCameraSupportSummary
    {
        get => _missingCameraSupportSummary;
        private set => SetProperty(ref _missingCameraSupportSummary, value);
    }

    public CameraSupportToastState(CameraSupportInstallationStatus status, AppSettings settings)
    {
        _status = status;
        _settings = settings;
    }

    private bool IsSnoozed =>
        _settings.CameraSupportSnoozeUntil is DateTime until && until > DateTime.Now;

    public void Refresh()
    {
        if (_dismissedThisSession || IsSnoozed)
        {
            HasMissingCameraSupport = false;
            MissingCameraSupportSummary = string.Empty;
            return;
        }

        var missing = _status.ResolveMissing();
        if (missing.Count == 0)
        {
            HasMissingCameraSupport = false;
            MissingCameraSupportSummary = string.Empty;
            return;
        }

        MissingCameraSupportSummary = string.Join(", ", missing.Select(m => m.DisplayName));
        HasMissingCameraSupport = true;
    }

    /// <summary>Called when <see cref="CameraSupportInstallationStatus.StatusChanged"/> fires.</summary>
    public void OnStatusChanged() => Refresh();

    public void RemindLater()
    {
        _settings.CameraSupportSnoozeUntil = DateTime.Now.AddDays(3);
        _settings.Save();
        HasMissingCameraSupport = false;
        MissingCameraSupportSummary = string.Empty;
    }

    public void Dismiss()
    {
        _dismissedThisSession = true;
        HasMissingCameraSupport = false;
        MissingCameraSupportSummary = string.Empty;
    }
}
```

- [ ] **Step 4: Inject the service into `MainWindowViewModel` constructor**

Modify `ViewModels/MainWindowViewModel.cs`. Update the constructor (lines 134-152):

```csharp
    public MainWindowViewModel(
        ReportGenerationService reportService,
        UpdateService updateService,
        CameraSupportInstallationStatus cameraSupportStatus)
    {
        _reportService = reportService;
        _updateService = updateService;
        _appSettings = AppSettings.Load();

        GenerateHtml = _appSettings.DefaultReportSettings.GenerateHtml;
        GeneratePdf = _appSettings.DefaultReportSettings.GeneratePdf;
        OpenWhenDone = _appSettings.DefaultReportSettings.OpenReportWhenDone;

        _reportService.ProgressReported += OnProgressReported;

        LogEntries.CollectionChanged += OnLogEntriesChanged;
        SubscribeToUpdateService();
        SubscribeToCameraSupport(cameraSupportStatus);

        StatusText = "Ready - Drop camera footage to begin";
    }
```

Add the `using` at the top of `MainWindowViewModel.cs` if not present:

```csharp
using LunaApp.Services.CameraSupport;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `dotnet test LunaApp.Tests/LunaApp.Tests.csproj`
Expected: all tests pass (existing + 7 new toast tests).

- [ ] **Step 6: Build the full app**

Run: `dotnet build LunaApp.sln -c Debug`
Expected: builds successfully.

- [ ] **Step 7: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs ViewModels/MainWindowViewModel.CameraSupport.cs LunaApp.Tests/CameraSupportToastViewModelTests.cs
git commit -m "feat(ui): add camera-support toast state + commands"
```

---

## Task 7: Add the toast UI in `MainWindow.axaml`

**Files:**
- Modify: `Views/MainWindow.axaml`

- [ ] **Step 1: Insert the toast block after the update toast**

Modify `Views/MainWindow.axaml`. After the closing `</Border>` of the update toast (line 148), insert the new camera-support toast block:

```xml
            <!-- Camera-support missing toast — fades in below the update toast. -->
            <Border Classes="fade"
                    Classes.visible="{Binding HasMissingCameraSupport}"
                    HorizontalAlignment="Right" VerticalAlignment="Top"
                    Background="{DynamicResource LunaBgSecondary}"
                    BorderBrush="{DynamicResource LunaAccentSubtle}"
                    BorderThickness="1"
                    CornerRadius="{DynamicResource LunaRadiusLg}"
                    Padding="16,14" MaxWidth="380"
                    Margin="0,140,8,0"
                    ZIndex="49">
                <StackPanel Spacing="10">
                    <StackPanel Orientation="Horizontal" Spacing="10">
                        <mi:MaterialIcon Kind="CameraOutline" Width="20" Height="20" Foreground="{DynamicResource LunaAccent}"/>
                        <TextBlock Text="Camera support missing"
                                   FontWeight="SemiBold"
                                   VerticalAlignment="Center"
                                   Foreground="{DynamicResource LunaTextPrimary}"/>
                    </StackPanel>

                    <TextBlock TextWrapping="Wrap"
                               FontSize="{DynamicResource LunaFontSizeSm}"
                               Foreground="{DynamicResource LunaTextSecondary}">
                        <Run Text="{Binding MissingCameraSupportSummary}" FontWeight="SemiBold"/>
                        <Run Text="not installed. Some camera formats won't decode until you install them from Settings."/>
                    </TextBlock>

                    <StackPanel Orientation="Horizontal" Spacing="8" HorizontalAlignment="Right">
                        <Button Classes="primary" Padding="12,6"
                                Command="{Binding OpenSettingsForCameraSupportCommand}">
                            <StackPanel Orientation="Horizontal" Spacing="6">
                                <mi:MaterialIcon Kind="Cog" Width="14" Height="14" Foreground="White"/>
                                <TextBlock Text="Open Settings"/>
                            </StackPanel>
                        </Button>
                        <Button Classes="secondary" Padding="12,6"
                                Command="{Binding RemindCameraSupportLaterCommand}"
                                ToolTip.Tip="Hide this for 3 days">
                            <TextBlock Text="Later"/>
                        </Button>
                        <Button Classes="secondary" Padding="8,6"
                                Command="{Binding DismissCameraSupportCommand}"
                                ToolTip.Tip="Dismiss"
                                AutomationProperties.Name="Dismiss camera-support notification">
                            <mi:MaterialIcon Kind="Close" Width="12" Height="12" Foreground="{DynamicResource LunaTextSecondary}"/>
                        </Button>
                    </StackPanel>
                </StackPanel>
            </Border>
```

The `Margin="0,140,8,0"` offsets it below the update toast (which sits at `Margin="0,8,8,0"`) so both stack cleanly when both are visible. Adjust the value during manual smoke testing if the update toast's measured height differs.

- [ ] **Step 2: Build and run**

Run: `dotnet build LunaApp.sln -c Debug`
Expected: builds successfully (no XAML compile errors).

Run: `dotnet run --project LunaApp.csproj` (Debug mode).
Manual sanity check:
- App launches.
- If neither tool is installed, the camera-support toast appears top-right beneath the existing update area.
- Click `Open Settings` → Settings dialog opens at the existing Camera Support section.
- Click `Later` → toast hides; restart app → toast still hidden.
- Click `Dismiss` → toast hides for the session.

- [ ] **Step 3: Commit**

```bash
git add Views/MainWindow.axaml
git commit -m "feat(ui): add camera-support missing toast"
```

---

## Task 8: Convert install-splash asset

**Files:**
- Create: `Assets/install-splash.png`

- [ ] **Step 1: Generate the splash PNG**

The splash is a 640×400 PNG derived from `Assets/luna-logo-lg.webp`. Velopack accepts PNG/JPEG/GIF and rejects WebP, so we commit a one-time conversion.

Run from the project root (PowerShell):

```powershell
# Use ImageMagick if available; otherwise open Assets/luna-logo-lg.webp in any
# image editor and export as PNG at 640x400 with the existing dark Luna
# background, save to Assets/install-splash.png.
magick Assets/luna-logo-lg.webp -resize 640x400 -background "#0a0a14" -gravity center -extent 640x400 Assets/install-splash.png
```

If ImageMagick isn't installed, do the export manually in any image editor. The file must be 640×400 PNG, file path exactly `Assets/install-splash.png`.

- [ ] **Step 2: Verify the file lands**

Run: `dotnet run --project LunaApp.csproj -- --version` *(only if a `--version` arg exists; otherwise just rely on the build).* Sanity check that `Assets/install-splash.png` exists:

```powershell
Test-Path Assets/install-splash.png
```

Expected: `True`.

- [ ] **Step 3: Commit**

```bash
git add Assets/install-splash.png
git commit -m "chore(assets): add install-splash.png for Velopack MSI installer"
```

---

## Task 9: Wire MSI flags + icons into `build.ps1`

**Files:**
- Modify: `build.ps1`

- [ ] **Step 1: Update the Windows `vpk pack` invocation**

Modify `build.ps1`. Replace the existing `vpk pack` block (lines 109-117) with:

```powershell
        # Windows: MSI installer with per-user install (no UAC), branded splash,
        # and Luna icon. macOS path keeps the same vpk pack with --icon set
        # to the .icns asset for proper bundle/DMG branding.
        if ($rid.StartsWith("win")) {
            vpk pack `
                --packId "Luna" `
                --packVersion $Version `
                --packDir $publishPath `
                --mainExe $mainExe `
                --outputDir $releasesDir `
                --packAuthors "Luna" `
                --packTitle "Luna - Camera Report Generator" `
                --icon "Assets/luna-logo.ico" `
                --splashImage "Assets/install-splash.png" `
                --msi `
                --instLocation PerUser
        } else {
            vpk pack `
                --packId "Luna" `
                --packVersion $Version `
                --packDir $publishPath `
                --mainExe $mainExe `
                --outputDir $releasesDir `
                --packAuthors "Luna" `
                --packTitle "Luna - Camera Report Generator" `
                --icon "Assets/luna-logo.icns"
        }
```

- [ ] **Step 2: Run the build locally on Windows**

Run: `pwsh ./build.ps1 -Version 1.0.0 -Runtime win-x64`
Expected: completes without error; `publish/releases/win-x64/` contains `Luna-1.0.0-win-x64.msi` (and the still-emitted `Luna-1.0.0-win-x64-Setup.exe`).

If this step fails because `vpk` doesn't recognize `--msi`, install / update the CLI:

```powershell
dotnet tool update -g vpk
```

- [ ] **Step 3: Manual install smoke check (build verification only — full smoke deferred to Task 12)**

Run the produced MSI in a Windows VM (or on the dev machine if comfortable). Confirm:
- The Velopack splash shows during install.
- Add/Remove Programs lists "Luna - Camera Report Generator" with the Luna icon.
- The app launches.
- No UAC prompt appears at install or update time.

- [ ] **Step 4: Commit**

```bash
git add build.ps1
git commit -m "build: emit per-user Velopack MSI with Luna branding"
```

---

## Task 10: Mirror the build flags into `release.yml`

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Update the Windows job's `vpk pack` step**

Modify `.github/workflows/release.yml`. Replace the `Create Velopack installer` step in `build-windows` (lines 91-100) with:

```yaml
      - name: Create Velopack MSI installer (per-user, branded)
        run: |
          vpk pack `
            --packId "Luna" `
            --packVersion ${{ needs.resolve-version.outputs.version }} `
            --packDir ./publish/win-x64 `
            --mainExe Luna.exe `
            --outputDir ./releases/win-x64 `
            --packAuthors "Luna" `
            --packTitle "Luna - Camera Report Generator" `
            --icon Assets/luna-logo.ico `
            --splashImage Assets/install-splash.png `
            --msi `
            --instLocation PerUser
```

- [ ] **Step 2: Restrict the upload artifact to the MSI + Velopack support files**

Replace the `Upload Windows installer artifacts` step (lines 102-107) with:

```yaml
      - name: Upload Windows MSI artifacts
        uses: actions/upload-artifact@v4
        with:
          name: luna-win-x64-installer
          path: |
            ./releases/win-x64/*.msi
            ./releases/win-x64/RELEASES
            ./releases/win-x64/*.nupkg
          retention-days: 30
```

(The `RELEASES` file + `.nupkg` are still needed for Velopack's auto-update flow.)

- [ ] **Step 3: Update the macOS job's `vpk pack` step**

Modify the `Create Velopack installer (DMG)` step in `build-macos-arm` (lines 155-164):

```yaml
      - name: Create Velopack installer (DMG, branded icon)
        run: |
          vpk pack \
            --packId "Luna" \
            --packVersion ${{ needs.resolve-version.outputs.version }} \
            --packDir ./publish/osx-arm64 \
            --mainExe Luna \
            --outputDir ./releases/osx-arm64 \
            --packAuthors "Luna" \
            --packTitle "Luna - Camera Report Generator" \
            --icon Assets/luna-logo.icns
```

- [ ] **Step 4: Update the release notes body**

Replace the `body:` block in `Create GitHub Release` (lines 210-230) with:

```yaml
          body: |
            ## Installation

            ### Windows
            1. Download `Luna-${{ needs.resolve-version.outputs.version }}-win-x64.msi`
            2. Run the installer - if SmartScreen appears, click "More info" → "Run anyway"
            3. Follow the wizard; Luna installs to your user profile (no admin required)

            ### macOS (Apple Silicon)
            1. Download `Luna-${{ needs.resolve-version.outputs.version }}-osx-arm64.dmg`
            2. Open the DMG and drag Luna to Applications
            3. First launch: Right-click Luna → Open → Click "Open" in the dialog
            4. **Important:** Run this command in Terminal to enable bundled tools:
               ```
               xattr -cr /Applications/Luna.app
               ```

            See [Installation Guide](https://github.com/shakedex/LunaApp/blob/master/docs/INSTALLATION.md) for detailed instructions.

            ---
            *Built with .NET 10 runtime included - no additional installation required.*
```

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: emit per-user MSI + branded DMG via vpk pack"
```

---

## Task 11: Add CI verification gates

**Files:**
- Modify: `build.ps1`
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Add the assembly + native deps assertion to `build.ps1`**

Modify `build.ps1`. After the existing runtime check (line 79, after `Write-Host "✓ .NET runtime bundled successfully"`), add:

```powershell
    # ---- Bundled-runtime + native-deps verification (catches trim regressions) ----
    if ($rid.StartsWith("win")) {
        $required = @(
            "Luna.exe", "Avalonia.dll", "SkiaSharp.dll",
            "FFmpeg.AutoGen.dll", "MediaInfo.Wrapper.Core.dll",
            "QuestPDF.dll", "Velopack.dll",
            "libSkiaSharp.dll", "MediaInfo.dll"
        )
        foreach ($name in $required) {
            $p = Join-Path $publishPath $name
            if (-not (Test-Path $p)) {
                Write-Error "✗ Required file missing from publish output: $name"
                exit 1
            }
        }

        # FFmpeg DLLs ship under tools/ffmpeg/win-x64/.
        $ffmpegDlls = @("avcodec-61.dll", "avformat-61.dll", "avutil-59.dll", "swresample-5.dll", "swscale-8.dll")
        foreach ($name in $ffmpegDlls) {
            $p = Join-Path $publishPath "tools/ffmpeg/win-x64" | Join-Path -ChildPath $name
            if (-not (Test-Path $p)) {
                Write-Error "✗ FFmpeg DLL missing from publish output: $name"
                exit 1
            }
        }

        # Avalonia.Diagnostics must NOT ship in Release builds.
        if ($Configuration -eq "Release") {
            $diag = Join-Path $publishPath "Avalonia.Diagnostics.dll"
            if (Test-Path $diag) {
                Write-Error "✗ Avalonia.Diagnostics.dll leaked into Release publish output"
                exit 1
            }
        }
        Write-Host "✓ Bundled assemblies + native deps verified" -ForegroundColor Green
    }
```

If any of the FFmpeg DLL filenames don't match what `tools/ffmpeg/win-x64/` actually contains, list the directory once and adjust the array — they're version-pinned by the FFmpeg.AutoGen package and may differ. The build will tell you which ones are missing.

- [ ] **Step 2: Mirror the gate into `release.yml`**

Modify `.github/workflows/release.yml`. After the `Verify runtime is bundled` step in `build-windows` (lines 82-89), add a new step:

```yaml
      - name: Verify bundled assemblies + native deps
        shell: pwsh
        run: |
          $publishPath = "./publish/win-x64"
          $required = @(
            "Luna.exe", "Avalonia.dll", "SkiaSharp.dll",
            "FFmpeg.AutoGen.dll", "MediaInfo.Wrapper.Core.dll",
            "QuestPDF.dll", "Velopack.dll",
            "libSkiaSharp.dll", "MediaInfo.dll"
          )
          foreach ($name in $required) {
            $p = Join-Path $publishPath $name
            if (-not (Test-Path $p)) {
              Write-Error "Required file missing from publish output: $name"
              exit 1
            }
          }
          $ffmpegDlls = @("avcodec-61.dll", "avformat-61.dll", "avutil-59.dll", "swresample-5.dll", "swscale-8.dll")
          foreach ($name in $ffmpegDlls) {
            $p = Join-Path $publishPath "tools/ffmpeg/win-x64" | Join-Path -ChildPath $name
            if (-not (Test-Path $p)) {
              Write-Error "FFmpeg DLL missing from publish output: $name"
              exit 1
            }
          }
          if (Test-Path (Join-Path $publishPath "Avalonia.Diagnostics.dll")) {
            Write-Error "Avalonia.Diagnostics.dll leaked into Release publish output"
            exit 1
          }
          Write-Host "Bundled assemblies + native deps verified"
```

- [ ] **Step 3: Run `build.ps1` locally to confirm the gate passes**

Run: `pwsh ./build.ps1 -Version 1.0.0 -Runtime win-x64`
Expected: "Bundled assemblies + native deps verified" line near the end; build succeeds.

- [ ] **Step 4: Commit**

```bash
git add build.ps1 .github/workflows/release.yml
git commit -m "ci: assert bundled .NET runtime + native deps before packaging"
```

---

## Task 12: Update INSTALLATION.md

**Files:**
- Modify: `docs/INSTALLATION.md`

- [ ] **Step 1: Read the current file**

Read `docs/INSTALLATION.md` end-to-end and identify any reference to `Setup.exe`. Replace each with the MSI flow.

- [ ] **Step 2: Replace the Windows section**

In `docs/INSTALLATION.md`, the Windows install instructions should read:

```markdown
## Windows

1. Download `Luna-<version>-win-x64.msi` from the [latest release](https://github.com/shakedex/LunaApp/releases).
2. Run the installer.
   - If Windows SmartScreen warns about an unrecognized publisher, click **More info → Run anyway**.
3. The installer shows a Luna splash and runs to completion in a few seconds. Luna installs to your user profile (`%LocalAppData%\Luna`) — no admin rights required.
4. Launch Luna from the Start menu.

### Updating
Luna checks for new releases automatically. When one is available, an in-app banner offers to download and restart. Updates do not require admin rights.

### Uninstalling
Open **Settings → Apps → Installed apps**, find **Luna - Camera Report Generator**, and click **Uninstall**.
```

(Keep any pre-existing macOS section unchanged — Task 10 already updated the release-body equivalent.)

- [ ] **Step 3: Commit**

```bash
git add docs/INSTALLATION.md
git commit -m "docs: switch Windows install instructions to MSI"
```

Note: `docs/` is gitignored (`.gitignore:492`). If `git add` errors with `paths are ignored by .gitignore`, decide with the human whether to keep the docs file local-only (matching the existing pattern with the spec) or carve out an exception with `git add -f`. Default: skip the commit and leave the file uncommitted on disk.

---

## Task 13: Manual end-to-end smoke test (human-driven)

**Files:** none (manual verification only)

This task is a checklist for the human running the release. Do not auto-complete.

- [ ] Fresh Windows 11 VM, no .NET 10 runtime installed.
- [ ] Install `Luna-1.0.0-win-x64.msi`. Confirm: no UAC, no .NET / VC++ prompt, ARP entry exists with the Luna icon, app launches.
- [ ] Camera-support toast appears at startup, listing both missing tools.
- [ ] Click `Open Settings` → Settings opens at Camera Support; install ARRI from the existing row → toast updates to show only Sony.
- [ ] Install Sony from Settings → toast hides once Sony's locator confirms install (Detect button).
- [ ] Drop a folder containing a `.ari` file *before* installing ART CLI → toast re-appears.
- [ ] Click `Later` → toast hides → restart → toast still hidden (3-day snooze).
- [ ] Trigger update flow via dev banner → both toasts visible top-right, stacked, no overlap.
- [ ] Uninstall via Settings → Apps → confirm install dir cleaned, shortcuts gone.

Report any issues; iterate on toast margin / Velopack flags as needed before tagging the release.

---

## Self-Review

**Spec coverage check:**

- ✅ Section 1 (Velopack MSI build pipeline) → Tasks 8, 9, 10.
- ✅ Section 1 (icon branding for MSI + DMG) → Tasks 8, 9, 10.
- ✅ Section 2 (`CameraSupportInstallationStatus` service) → Tasks 3, 4.
- ✅ Section 2 (Settings invokes `Invalidate()`) → Task 5.
- ✅ Section 3 (toast UI + commands + snooze) → Tasks 2, 6, 7.
- ✅ Section 4 (CI verification gates: managed assemblies, native deps, Avalonia.Diagnostics absence) → Task 11.
- ⚠️ Section 4 (manifest sanity + MSI prerequisite check) → not a separate task; both are passive checks. The manifest is verified by inspecting `app.manifest` for `<compatibility>` GUIDs covering Win10/Win11 during code review of Task 9. The MSI prerequisite check is a one-time inspection by the human after Task 9 produces the first MSI; if a `.NETFramework` LaunchCondition is found, a follow-up plan addresses it.
- ✅ Section 5 (unit tests) → Tasks 1, 2, 4, 6.
- ✅ Section 5 (manual smoke test) → Task 13.
- ✅ Section 5 (rollout, no version bump) → Tasks 10, 12 use `1.0.0`.

**Placeholder scan:** none of the steps say "TBD", "implement later", or "add appropriate error handling." Every code step contains the actual code.

**Type consistency:**
- Service: `CameraSupportInstallationStatus`, method `ResolveMissing()`, event `StatusChanged`, method `Invalidate()`. Used identically in Tasks 4, 5, 6.
- Probe interfaces: `IArtCliInstallProbe.IsInstalled`, `ISonyRawViewerInstallProbe.IsInstalled` + `IsSupportedOnThisOs`. Used identically in Tasks 3, 4, 6.
- ViewModel state: `HasMissingCameraSupport`, `MissingCameraSupportSummary`. Same names in Tasks 6 (state class), 6 (ViewModel partial), 7 (XAML bindings).
- Commands: `OpenSettingsForCameraSupportCommand`, `RemindCameraSupportLaterCommand`, `DismissCameraSupportCommand`. Same names in Tasks 6 + 7.
- Settings field: `CameraSupportSnoozeUntil`. Same name in Tasks 2, 6.

**Open hand-offs noted in plan:**
- Task 3 commits at end of Task 4 (intentional — fakes don't compile until the service ships).
- Task 12 may not commit if `docs/` stays gitignored — decision deferred to human.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-27-velopack-msi-and-camera-support-prompt.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session using executing-plans, batched with checkpoints.

Which approach?
