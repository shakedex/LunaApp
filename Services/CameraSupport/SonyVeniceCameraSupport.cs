using LunaApp.Models;
using LunaApp.Services.Chappie;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// Sony Venice / Burano / FX9 / F55 / F65 raw support.
///
/// Sony cameras can record either non-raw (XAVC, XDCAM in MXF — already handled
/// by <see cref="GenericCameraSupport"/> + <see cref="SonyXmlEnricher"/>) or
/// X-OCN raw, which FFmpeg cannot decode. Container ≠ vendor; we don't claim
/// .mxf at the dispatch level (Sony shares MXF with ARRI ProRes, Panasonic,
/// etc.).
///
/// Frame extraction for X-OCN happens through
/// <see cref="SonyRawExporterThumbnailService"/> in the thumbnail-generator
/// chain — it activates whenever Sony RAW Viewer is installed and the file's
/// codec string matches X-OCN markers. This support class exists to surface
/// the install state on the Settings → Camera Support row.
/// </summary>
public sealed class SonyVeniceCameraSupport : ICameraSupport
{
    private readonly SonyRawViewerLocator _locator;

    public SonyVeniceCameraSupport(SonyRawViewerLocator locator)
    {
        _locator = locator;
    }

    public string Id => "sony-venice";
    public string DisplayName => "Sony Venice / Burano / FX9";

    public IReadOnlySet<string> HandledExtensions { get; } =
        new HashSet<string>(StringComparer.OrdinalIgnoreCase); // dispatch handled by Generic + chain

    public SupportStatus Status => _locator.Resolve() is { } install
        ? new SupportStatus.Ready($"rawexporter {install.Version}", "Sony RAW Viewer (detected)")
        : new SupportStatus.ComingLater("Install Sony RAW Viewer to enable X-OCN frame extraction");

    public bool CanHandle(string filePath) => false;

    public Task<CameraClip> ProcessAsync(
        string filePath,
        bool extractThumbnails,
        int thumbnailCount,
        int thumbnailWidth,
        CancellationToken cancellationToken) =>
        Task.FromResult(CameraSupportHelpers.CreateUnsupportedClip(filePath, this));
}
