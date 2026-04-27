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
        var vm = new CameraSupportToastState(status, settings, _ => { });
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
