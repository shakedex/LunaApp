using LunaApp.Models;
using LunaApp.Services.CameraSupport;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Thin dispatcher. Resolves each file to an <see cref="ICameraSupport"/> via
/// the registry and delegates. Files that match a non-Ready support (e.g. a
/// <c>.ari</c> while ARRI support is still <c>ComingLater</c>) are returned as
/// Unsupported clips carrying a typed notice — never dropped into a fallback
/// decoder that would produce garbage.
/// </summary>
public sealed class ChappieEngine(CameraSupportRegistry registry, MediaProcessingOptions options)
{
    private readonly CameraSupportRegistry _registry = registry;
    private readonly MediaProcessingOptions _options = options;

    public async Task<CameraClip> ProcessClipAsync(
        string filePath,
        bool extractThumbnails = true,
        int? thumbnailCount = null,
        int? thumbnailWidth = null,
        CancellationToken cancellationToken = default)
    {
        var effectiveCount = thumbnailCount ?? _options.ThumbnailCount;
        var effectiveWidth = thumbnailWidth ?? _options.ThumbnailWidth;
        var fileName = Path.GetFileName(filePath);

        var support = _registry.ResolveFor(filePath);
        if (support is null)
        {
            Log.Information("No camera support claims {File} — returning unsupported clip", fileName);
            return CameraSupportHelpers.CreateUnsupportedClip(
                filePath,
                new UnknownFormatSupport());
        }

        if (support.Status is not SupportStatus.Ready)
        {
            Log.Information("{File} routed to {Support} ({Status}) — emitting notice",
                fileName, support.Id, support.Status.Summary);
            return CameraSupportHelpers.CreateUnsupportedClip(filePath, support);
        }

        return await support.ProcessAsync(
            filePath, extractThumbnails, effectiveCount, effectiveWidth, cancellationToken);
    }

    public async Task<List<CameraClip>> ProcessClipsAsync(
        IEnumerable<string> filePaths,
        bool extractThumbnails = true,
        int? thumbnailCount = null,
        int? thumbnailWidth = null,
        IProgress<(int current, int total, string file)>? progress = null,
        CancellationToken cancellationToken = default)
    {
        var paths = filePaths.ToList();
        var clips = new List<CameraClip>(paths.Count);

        for (int i = 0; i < paths.Count; i++)
        {
            cancellationToken.ThrowIfCancellationRequested();

            var path = paths[i];
            progress?.Report((i + 1, paths.Count, Path.GetFileName(path)));

            var clip = await ProcessClipAsync(path, extractThumbnails, thumbnailCount, thumbnailWidth, cancellationToken);
            clips.Add(clip);
        }

        return clips;
    }

    /// <summary>Placeholder support used only to satisfy the "unknown" branch.</summary>
    private sealed class UnknownFormatSupport : ICameraSupport
    {
        public string Id => "unknown";
        public string DisplayName => "Unknown format";
        public IReadOnlySet<string> HandledExtensions { get; } = new HashSet<string>();
        public SupportStatus Status { get; } = new SupportStatus.NotAvailable("No camera support claims this file");
        public bool CanHandle(string filePath) => false;
        public Task<CameraClip> ProcessAsync(string filePath, bool extractThumbnails, int thumbnailCount, int thumbnailWidth, CancellationToken cancellationToken) =>
            throw new NotSupportedException();
    }
}

// Kept for compatibility with ReelDetectionService usage; no longer the primary classification path.
public enum CameraManufacturer
{
    Unknown, Arri, Sony, Red, Blackmagic, Canon, Panasonic, DJI
}
