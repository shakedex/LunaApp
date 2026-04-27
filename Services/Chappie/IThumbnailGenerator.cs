using LunaApp.Models;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Typed result of a thumbnail extraction attempt. Carries both the frames
/// (possibly empty) and a classified <see cref="ThumbnailOutcome"/> so the
/// caller can react to "no decoder" / "seek failed" / "success with 3 frames"
/// without re-parsing free-text log lines.
/// </summary>
public sealed record ThumbnailResult(
    IReadOnlyList<ThumbnailFrame> Frames,
    ThumbnailOutcome Outcome,
    string? Detail)
{
    public static ThumbnailResult NotAttempted(string? detail = null) =>
        new([], ThumbnailOutcome.NotAttempted, detail);

    public static ThumbnailResult NoDecoder(string codecName) =>
        new([], ThumbnailOutcome.NoDecoder, $"FFmpeg has no decoder for codec '{codecName}'");

    public static ThumbnailResult ContainerOpenFailed(string reason) =>
        new([], ThumbnailOutcome.ContainerOpenFailed, reason);

    public static ThumbnailResult SeekFailed(string detail) =>
        new([], ThumbnailOutcome.SeekFailed, detail);

    public static ThumbnailResult DecodeFailed(string detail) =>
        new([], ThumbnailOutcome.DecodeFailed, detail);

    public static ThumbnailResult Success(IReadOnlyList<ThumbnailFrame> frames) =>
        new(frames, ThumbnailOutcome.Success, null);
}

/// <summary>
/// Generates thumbnail frames for a clip. Multiple implementations are
/// composed into a cascade by <see cref="GenericCameraSupport"/>: lower
/// <see cref="Priority"/> runs first, and a result of
/// <see cref="ThumbnailOutcome.NoDecoder"/> falls through to the next
/// generator. Any other outcome stops the chain.
/// </summary>
public interface IThumbnailGenerator
{
    /// <summary>Lower runs first. FFmpeg = 0 (the universal fallback). Vendor decoders &gt; 0.</summary>
    int Priority { get; }

    bool IsAvailable { get; }

    Task<ThumbnailResult> GenerateAsync(
        string filePath,
        ThumbnailRequest request,
        CancellationToken cancellationToken);
}

/// <summary>
/// Bundle of inputs every thumbnail generator needs. Includes the codec
/// string MediaInfo extracted earlier so vendor generators can decide
/// whether to handle this file ("does it look like X-OCN?") without
/// re-probing the container, and the frame rate so frame-index-driven
/// CLI tools (Sony rawexporter etc.) can convert percentage positions
/// to absolute frame numbers without re-reading the container.
/// </summary>
public sealed record ThumbnailRequest(
    TimeSpan Duration,
    int Count,
    int Width,
    string? Codec,
    double FrameRate);
