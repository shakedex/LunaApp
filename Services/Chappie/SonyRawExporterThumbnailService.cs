using System.Diagnostics;
using LunaApp.Models;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Thumbnail generator for Sony X-OCN / F55-raw / F65-raw clips. FFmpeg has
/// no decoder for these, so we shell out to <c>rawexporter.exe</c> from
/// Sony's RAW Viewer install. Each requested position becomes one CLI
/// invocation that exports a single DPX frame; <see cref="FfmpegThumbnailService.DecodeSingleImage"/>
/// then ingests the DPX (FFmpeg reads DPX natively) and produces a
/// <see cref="ThumbnailFrame"/> using the same WebP-encoding path as every
/// other clip.
///
/// Positions are computed in <em>frame indices</em> (rawexporter's <c>--in</c>
/// takes a 0-based frame number, learned via the dry-run probe). Frame count
/// = duration × fps; we pick frames at 10 / 50 / 90 % of that range.
///
/// We never redistribute Sony binaries — detect-only via
/// <see cref="SonyRawViewerLocator"/>.
/// </summary>
public sealed class SonyRawExporterThumbnailService : IThumbnailGenerator
{
    private static readonly TimeSpan PerFrameTimeout = TimeSpan.FromSeconds(20);

    /// <summary>Codec strings (from MediaInfo) that route to rawexporter. Conservative — we only claim formats we know it handles.</summary>
    private static readonly string[] SupportedCodecMarkers =
    {
        "X-OCN",   // F55_X-OCN_*, F65_X-OCN_*, X-OCN ST/LT/XT
        "RAW SQ",  // alternate Sony raw labels
        "F65 RAW",
    };

    private readonly SonyRawViewerLocator _locator;
    private readonly FfmpegThumbnailService _ffmpeg;

    public SonyRawExporterThumbnailService(SonyRawViewerLocator locator, FfmpegThumbnailService ffmpeg)
    {
        _locator = locator;
        _ffmpeg = ffmpeg;
    }

    public int Priority => 100; // After FFmpeg.

    public bool IsAvailable => _locator.Resolve() is not null;

    public async Task<ThumbnailResult> GenerateAsync(
        string filePath,
        ThumbnailRequest request,
        CancellationToken cancellationToken)
    {
        // Only claim Sony-raw codecs. Anything else cascades to the next
        // generator (or back to the chain's "best previous" result).
        if (!LooksLikeSonyRaw(request.Codec))
            return ThumbnailResult.NoDecoder($"not a Sony raw codec ({request.Codec ?? "unknown"})");

        var install = _locator.Resolve();
        if (install is null)
        {
            return ThumbnailResult.NoDecoder(
                "Sony RAW Viewer is not installed — install it from sony.com/rawviewer to enable X-OCN frames");
        }

        if (request.Duration <= TimeSpan.Zero)
            return ThumbnailResult.NotAttempted("clip duration is zero");

        // rawexporter's --in is a frame index. Convert percentage positions to
        // absolute frames using the clip's actual frame rate; fall back to 24
        // if MediaInfo didn't supply one.
        var fps = request.FrameRate > 0 ? request.FrameRate : 24.0;
        var totalFrames = Math.Max(1, (int)Math.Round(request.Duration.TotalSeconds * fps));

        var positions = ComputePositions(request.Count);
        var tempDir = Path.Combine(Path.GetTempPath(), $"luna_sony_{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);

        var frames = new List<ThumbnailFrame>();
        var failureCount = 0;
        var lastError = "";

        try
        {
            for (int i = 0; i < positions.Length; i++)
            {
                cancellationToken.ThrowIfCancellationRequested();

                var position = positions[i];
                var frameIndex = Math.Clamp((int)(position * (totalFrames - 1)), 0, totalFrames - 1);
                var outputBase = $"luna_thumb_{i}";

                var (ok, errorDetail) = await RunRawExporterAsync(
                    install.ExecutablePath, filePath, frameIndex, tempDir, outputBase, cancellationToken);

                if (!ok)
                {
                    failureCount++;
                    lastError = errorDetail;
                    Log.Warning("rawexporter failed at frame {Frame} for {File}: {Detail}",
                        frameIndex, Path.GetFileName(filePath), errorDetail);
                    continue;
                }

                // rawexporter writes "<base>.00000.dpx" — fixed 5-digit padding
                // regardless of --digits. Find the produced DPX.
                var dpx = Directory.GetFiles(tempDir, $"{outputBase}.*.dpx").FirstOrDefault();
                if (dpx is null)
                {
                    failureCount++;
                    lastError = "rawexporter exited 0 but produced no DPX";
                    continue;
                }

                var thumb = _ffmpeg.DecodeSingleImage(
                    dpx,
                    request.Width,
                    position,
                    TimeSpan.FromSeconds(position * request.Duration.TotalSeconds));

                if (thumb is null)
                {
                    failureCount++;
                    lastError = "FFmpeg failed to decode the DPX rawexporter produced";
                    continue;
                }

                // Backfill the frame-index the regular FFmpeg path fills in.
                thumb.FrameNumber = frameIndex;
                frames.Add(thumb);
            }

            if (frames.Count == 0)
                return ThumbnailResult.DecodeFailed($"rawexporter produced no usable frames ({lastError})");

            return ThumbnailResult.Success(frames);
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception ex)
        {
            Log.Warning(ex, "Sony rawexporter pipeline threw on {File}", Path.GetFileName(filePath));
            return ThumbnailResult.DecodeFailed(ex.Message);
        }
        finally
        {
            try { Directory.Delete(tempDir, recursive: true); } catch { /* best effort */ }
        }
    }

    private static bool LooksLikeSonyRaw(string? codec)
    {
        if (string.IsNullOrWhiteSpace(codec)) return false;
        foreach (var marker in SupportedCodecMarkers)
            if (codec.Contains(marker, StringComparison.OrdinalIgnoreCase))
                return true;
        return false;
    }

    private static double[] ComputePositions(int count) =>
        count <= 1
            ? [0.5]
            : [.. Enumerable.Range(0, count).Select(i => 0.1 + 0.8 * i / (count - 1))];

    private static async Task<(bool ok, string error)> RunRawExporterAsync(
        string exePath,
        string inputFile,
        int frameIndex,
        string outputDir,
        string outputBase,
        CancellationToken cancellationToken)
    {
        var arguments =
            $"-I \"{inputFile}\" --in {frameIndex} --duration 1 " +
            $"-D \"{outputDir}\" -O {outputBase} " +
            "-V DPX --bake INPUT --resolution HD --display 0";

        var psi = new ProcessStartInfo
        {
            FileName = exePath,
            Arguments = arguments,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };

        using var proc = Process.Start(psi);
        if (proc is null) return (false, "Process.Start returned null");

        using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeoutCts.CancelAfter(PerFrameTimeout);

        string stderr = "";
        try
        {
            _ = proc.StandardOutput.ReadToEndAsync(timeoutCts.Token);
            stderr = await proc.StandardError.ReadToEndAsync(timeoutCts.Token);
            await proc.WaitForExitAsync(timeoutCts.Token);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            try { proc.Kill(); } catch { /* best effort */ }
            return (false, $"timed out after {PerFrameTimeout.TotalSeconds}s");
        }

        if (proc.ExitCode != 0)
            return (false, string.IsNullOrWhiteSpace(stderr) ? $"exit code {proc.ExitCode}" : stderr.Trim());

        return (true, "");
    }
}
