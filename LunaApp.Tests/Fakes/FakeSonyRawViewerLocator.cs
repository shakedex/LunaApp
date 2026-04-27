using LunaApp.Services.CameraSupport;

namespace LunaApp.Tests.Fakes;

internal sealed class FakeSonyRawViewerInstallProbe : ISonyRawViewerInstallProbe
{
    public bool IsInstalled { get; set; }
    public bool IsSupportedOnThisOs { get; set; } = true;
}
