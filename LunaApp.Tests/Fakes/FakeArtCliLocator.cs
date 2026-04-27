using LunaApp.Services.CameraSupport;

namespace LunaApp.Tests.Fakes;

internal sealed class FakeArtCliInstallProbe : IArtCliInstallProbe
{
    public bool IsInstalled { get; set; }
}
