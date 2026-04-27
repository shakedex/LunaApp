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
