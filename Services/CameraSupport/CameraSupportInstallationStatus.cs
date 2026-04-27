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

    // Constructor is internal because the probe interfaces are internal
    // (test-only seam). DI within LunaApp resolves this fine; tests reach
    // it via InternalsVisibleTo.
    internal CameraSupportInstallationStatus(
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
