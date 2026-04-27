using LunaApp.Models;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// Scaffolded Blackmagic RAW support. Claims <c>.braw</c> so the engine
/// surfaces a "coming later" notice. Real implementation will probe for an
/// existing DaVinci Resolve / Blackmagic RAW Player / Speed Test install
/// (all ship the BRAW API) or chain-run Blackmagic's own installer.
/// </summary>
public sealed class BlackmagicRawCameraSupport : ICameraSupport
{
    private static readonly HashSet<string> Extensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".braw"
    };

    private const string RoadmapNote = "Blackmagic RAW SDK integration is planned";

    public string Id => "blackmagic-raw";
    public string DisplayName => "Blackmagic RAW (URSA, Pocket 6K/12K)";
    public IReadOnlySet<string> HandledExtensions => Extensions;
    public SupportStatus Status { get; } = new SupportStatus.ComingLater(RoadmapNote);

    public bool CanHandle(string filePath) =>
        Extensions.Contains(Path.GetExtension(filePath));

    public Task<CameraClip> ProcessAsync(
        string filePath,
        bool extractThumbnails,
        int thumbnailCount,
        int thumbnailWidth,
        CancellationToken cancellationToken) =>
        Task.FromResult(CameraSupportHelpers.CreateUnsupportedClip(filePath, this));
}
