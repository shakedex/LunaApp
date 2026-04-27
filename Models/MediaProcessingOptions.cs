namespace LunaApp.Models;

public sealed record MediaProcessingOptions
{
    public int ThumbnailCount { get; init; } = 3;

    // Source extraction resolution. Reports scale thumbnails up for readability
    // (especially the PDF print path), so the original needs to be larger
    // than the on-screen display size. 1280px wide stays well under 200 KB
    // per WebP at quality 85 — three thumbs × ~100 clips = ~30 MB report,
    // which is fine.
    public int ThumbnailWidth { get; init; } = 1280;
    public int ThumbnailQuality { get; init; } = 85;

    public TimeSpan PerClipMetadataTimeout { get; init; } = TimeSpan.FromMinutes(5);
    public TimeSpan FfmpegThumbnailTimeout { get; init; } = TimeSpan.FromSeconds(30);

    public int MaxConcurrentClips { get; init; } = 1;

    public static MediaProcessingOptions Default { get; } = new();
}
