using LunaApp.Models;
using LunaApp.Services.Chappie;
using Serilog;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// The built-in camera support. Handles anything MediaInfo can read and
/// FFmpeg can decode — H.264 / H.265 / ProRes / DNxHD, plus non-raw MXF
/// from Sony (XAVC, XDCAM) which also picks up Sony XML sidecar enrichment
/// when present. Always <see cref="SupportStatus.Ready"/> because its
/// binaries (FFmpeg LGPL libs, MediaInfo) ship with Luna.
/// </summary>
public sealed class GenericCameraSupport : ICameraSupport
{
    // Anything our baseline stack (MediaInfo + FFmpeg) might sensibly read.
    // Proprietary-only extensions (.ari, .braw, .r3d) are deliberately absent
    // here so they route to their dedicated support classes instead.
    private static readonly HashSet<string> Extensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mov", ".mp4", ".m4v", ".mxf", ".mkv", ".avi", ".mts", ".m2ts", ".ts", ".webm"
    };

    private readonly IMetadataExtractor _extractor;
    private readonly IReadOnlyList<IMetadataEnricher> _enrichers;
    private readonly IReadOnlyList<IThumbnailGenerator> _thumbnailChain;

    public GenericCameraSupport(
        IMetadataExtractor extractor,
        IEnumerable<IMetadataEnricher> enrichers,
        IEnumerable<IThumbnailGenerator> thumbnailGenerators)
    {
        _extractor = extractor;
        _enrichers = enrichers.OrderBy(e => e.Priority).ToArray();
        _thumbnailChain = thumbnailGenerators.OrderBy(g => g.Priority).ToArray();
    }

    public string Id => "generic";
    public string DisplayName => "Generic (H.264 / H.265 / ProRes / DNxHD)";
    public IReadOnlySet<string> HandledExtensions => Extensions;

    public SupportStatus Status => _thumbnailChain.Any(g => g.IsAvailable)
        ? new SupportStatus.Ready("FFmpeg 7.1.3", "bundled")
        : new SupportStatus.NotAvailable("FFmpeg libraries are missing — check tools/ffmpeg next to the app");

    public bool CanHandle(string filePath) =>
        Extensions.Contains(Path.GetExtension(filePath));

    public async Task<CameraClip> ProcessAsync(
        string filePath,
        bool extractThumbnails,
        int thumbnailCount,
        int thumbnailWidth,
        CancellationToken cancellationToken)
    {
        var fileName = Path.GetFileName(filePath);

        CameraClip clip;
        try
        {
            clip = await _extractor.ExtractAsync(filePath, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Metadata extraction failed for {File}", fileName);
            return CreateFailedClip(filePath, ex.Message);
        }

        foreach (var enricher in _enrichers)
        {
            cancellationToken.ThrowIfCancellationRequested();
            try
            {
                if (!enricher.CanEnrich(filePath)) continue;
                Log.Debug("Running enricher {Enricher} for {File}", enricher.GetType().Name, fileName);
                await enricher.EnrichAsync(clip, cancellationToken);
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex)
            {
                Log.Warning(ex, "Enricher {Enricher} failed for {File} (continuing)", enricher.GetType().Name, fileName);
            }
        }

        if (extractThumbnails)
        {
            var request = new ThumbnailRequest(clip.Duration, thumbnailCount, thumbnailWidth, clip.Codec, clip.FrameRate);
            var result = await TryThumbnailChainAsync(filePath, fileName, request, cancellationToken);

            clip.Thumbnails = result.Frames.ToList();
            clip.ThumbnailOutcome = result.Outcome;
            clip.ThumbnailOutcomeDetail = result.Detail;

            // When FFmpeg couldn't identify the codec (AV_CODEC_ID_NONE for
            // most proprietary RAW), prefer the MediaInfo codec string so the
            // user sees "F55_X-OCN_LT_6K_3:2" instead of raw FFmpeg-speak.
            if (result.Outcome == ThumbnailOutcome.NoDecoder
                && !string.IsNullOrWhiteSpace(clip.Codec)
                && (result.Detail?.Contains("AV_CODEC_ID_NONE", StringComparison.Ordinal) ?? false))
            {
                clip.ThumbnailOutcomeDetail = $"No installed decoder handles {clip.Codec}";
            }

            if (result.Outcome != ThumbnailOutcome.Success)
            {
                Log.Information("Thumbnails for {File}: {Outcome} — {Detail}",
                    fileName, result.Outcome, clip.ThumbnailOutcomeDetail);
            }
        }

        clip.ProcessingState = ClipProcessingState.Completed;
        return clip;
    }

    /// <summary>
    /// Run the thumbnail-generator chain in priority order. Cascade rule:
    /// <see cref="ThumbnailOutcome.NoDecoder"/> falls through to the next
    /// generator (the current one didn't recognize the codec). Any other
    /// outcome stops the chain — those mean "this generator owns the
    /// decision" and cascading would mask real failures.
    /// </summary>
    private async Task<ThumbnailResult> TryThumbnailChainAsync(
        string filePath,
        string fileName,
        ThumbnailRequest request,
        CancellationToken cancellationToken)
    {
        ThumbnailResult? lastResult = null;

        foreach (var generator in _thumbnailChain)
        {
            cancellationToken.ThrowIfCancellationRequested();
            try
            {
                var result = await generator.GenerateAsync(filePath, request, cancellationToken);
                lastResult = result;

                if (result.Outcome == ThumbnailOutcome.NoDecoder)
                {
                    Log.Debug("Thumbnail generator {Generator} declined {File} ({Detail}); cascading",
                        generator.GetType().Name, fileName, result.Detail);
                    continue;
                }

                return result;
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception ex)
            {
                Log.Warning(ex, "Thumbnail generator {Generator} threw on {File} (continuing chain)",
                    generator.GetType().Name, fileName);
                lastResult = ThumbnailResult.DecodeFailed($"{generator.GetType().Name}: {ex.Message}");
            }
        }

        return lastResult ?? ThumbnailResult.NotAttempted("no thumbnail generators registered");
    }

    private static CameraClip CreateFailedClip(string filePath, string error)
    {
        var fileInfo = new FileInfo(filePath);
        return new CameraClip
        {
            FilePath = filePath,
            FileName = fileInfo.Name,
            FileSizeBytes = fileInfo.Exists ? fileInfo.Length : 0,
            ProcessingState = ClipProcessingState.Failed,
            ProcessingError = error
        };
    }
}
