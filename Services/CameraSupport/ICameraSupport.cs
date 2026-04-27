using LunaApp.Models;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// A camera family Luna knows how to read (or intends to, once the delivery
/// question for its binaries is answered). One implementation per family.
/// Luna core routes files through the registry to the first support that
/// claims them; adding a new format is writing one class and registering it.
/// </summary>
public interface ICameraSupport
{
    string Id { get; }
    string DisplayName { get; }
    IReadOnlySet<string> HandledExtensions { get; }
    SupportStatus Status { get; }

    bool CanHandle(string filePath);

    Task<CameraClip> ProcessAsync(
        string filePath,
        bool extractThumbnails,
        int thumbnailCount,
        int thumbnailWidth,
        CancellationToken cancellationToken);
}

public abstract record SupportStatus
{
    public sealed record Ready(string Version, string Provenance) : SupportStatus;
    public sealed record NotAvailable(string Reason) : SupportStatus;
    public sealed record ComingLater(string RoadmapNote) : SupportStatus;

    public string Summary => this switch
    {
        Ready r => $"Ready — {r.Version} ({r.Provenance})",
        NotAvailable na => $"Not available — {na.Reason}",
        ComingLater cl => $"Coming later — {cl.RoadmapNote}",
        _ => ToString() ?? "Unknown",
    };
}
