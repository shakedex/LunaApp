using System.Diagnostics;
using LunaApp.Models;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Thumbnail generator for ARRIRAW (<c>.ari</c>) clips. FFmpeg has no
/// decoder for ARRIRAW; <see cref="ArtCliMetadataEnricher"/> already covers
/// metadata, so this service is the frame-grab side: shell out to
/// <c>art-cmd</c> for one DPX per requested position, then ingest each DPX
/// via <see cref="FfmpegThumbnailService.DecodeSingleImage"/> into the
/// usual WebP-encoded <see cref="ThumbnailFrame"/>.
///
/// Chain priority sits between FFmpeg (0) and Sony rawexporter (100).
/// Returns <see cref="ThumbnailOutcome.NoDecoder"/> for any non-ARRIRAW
/// codec or when ART CLI isn't installed, letting the chain fall through.
///
/// Detect-only — never redistributes ARRI binaries. The installer
/// (<see cref="ArtCliInstaller"/>) downloads them on demand to
/// <c>%LOCALAPPDATA%\Luna\tools\arri\{rid}\</c>.
/// </summary>
public sealed class ArtCliThumbnailService : IThumbnailGenerator
{
    private static readonly TimeSpan PerFrameTimeout = TimeSpan.FromSeconds(45);

    private readonly ArtCliLocator _locator;
    private readonly FfmpegThumbnailService _ffmpeg;

    public ArtCliThumbnailService(ArtCliLocator locator, FfmpegThumbnailService ffmpeg)
    {
        _locator = locator;
        _ffmpeg = ffmpeg;
    }

    public int Priority => 50; // Ahead of Sony, after FFmpeg.

    public bool IsAvailable => _locator.Resolve() is not null;

    public async Task<ThumbnailResult> GenerateAsync(
        string filePath,
        ThumbnailRequest request,
        CancellationToken cancellationToken)
    {
        // ARRIRAW only. ARRI ProRes (MXF/MOV) decodes fine via FFmpeg.
        if (!Path.GetExtension(filePath).Equals(".ari", StringComparison.OrdinalIgnoreCase))
            return ThumbnailResult.NoDecoder("not ARRIRAW (.ari)");

        var install = _locator.Resolve();
        if (install is null)
            return ThumbnailResult.NoDecoder("ARRI Reference Tool (art-cmd) is not installed — install from Settings → Camera Support");

        if (request.Duration <= TimeSpan.Zero)
            return ThumbnailResult.NotAttempted("clip duration is zero");

        var fps = request.FrameRate > 0 ? request.FrameRate : 24.0;
        var totalFrames = Math.Max(1, (int)Math.Round(request.Duration.TotalSeconds * fps));
        var positions = ComputePositions(request.Count);

        var tempDir = Path.Combine(Path.GetTempPath(), $"luna_arri_thumbs_{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);

        var frames = new List<ThumbnailFrame>();
        var failureCount = 0;
        var lastError = "";

        try
        {
            for (var i = 0; i < positions.Length; i++)
            {
                cancellationToken.ThrowIfCancellationRequested();

                var position = positions[i];
                var frameIndex = Math.Clamp((int)(position * (totalFrames - 1)), 0, totalFrames - 1);
                var frameOutDir = Path.Combine(tempDir, $"f{i}");
                Directory.CreateDirectory(frameOutDir);

                var (ok, err) = await ExportFrameAsync(
                    install.ExecutablePath, filePath, frameIndex, frameOutDir, cancellationToken);

                if (!ok)
                {
                    failureCount++;
                    lastError = err;
                    Log.Warning("art-cmd failed on frame {Frame} of {File}: {Detail}",
                        frameIndex, Path.GetFileName(filePath), err);
                    continue;
                }

                // art-cmd places DPX (or other still output) somewhere under
                // frameOutDir — locate the first image we recognise.
                var stillExtensions = new[] { "*.dpx", "*.exr", "*.tif", "*.tiff", "*.png" };
                var producedStill = stillExtensions
                    .SelectMany(p => Directory.EnumerateFiles(frameOutDir, p, SearchOption.AllDirectories))
                    .FirstOrDefault();

                if (producedStill is null)
                {
                    failureCount++;
                    lastError = "art-cmd exited 0 but produced no still image we recognise";
                    continue;
                }

                var thumb = _ffmpeg.DecodeSingleImage(
                    producedStill,
                    request.Width,
                    position,
                    TimeSpan.FromSeconds(position * request.Duration.TotalSeconds));

                if (thumb is null)
                {
                    failureCount++;
                    lastError = $"FFmpeg failed to decode the {Path.GetExtension(producedStill)} art-cmd produced";
                    continue;
                }

                thumb.FrameNumber = frameIndex;
                frames.Add(thumb);
            }

            if (frames.Count == 0)
                return ThumbnailResult.DecodeFailed($"art-cmd produced no usable frames ({lastError})");

            return ThumbnailResult.Success(frames);
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception ex)
        {
            Log.Warning(ex, "ART CLI thumbnail pipeline threw on {File}", Path.GetFileName(filePath));
            return ThumbnailResult.DecodeFailed(ex.Message);
        }
        finally
        {
            try { Directory.Delete(tempDir, recursive: true); } catch { /* best effort */ }
        }
    }

    private static double[] ComputePositions(int count) =>
        count <= 1
            ? [0.5]
            : [.. Enumerable.Range(0, count).Select(i => 0.1 + 0.8 * i / (count - 1))];

    private static async Task<(bool ok, string error)> ExportFrameAsync(
        string exePath,
        string inputFile,
        int frameIndex,
        string outputDir,
        CancellationToken cancellationToken)
    {
        // ART CMD v1.0.0 'process' mode: outputs one decoded image per frame.
        // The output path uses a printf-format specifier; art-cmd substitutes
        // the absolute frame number. We extract one frame via --start +
        // --duration 1 and look for whatever .tif lands in outputDir.
        var outputTemplate = Path.Combine(outputDir, "frame_%07d.tif");
        var arguments =
            $"--mode process --input \"{inputFile}\" --output \"{outputTemplate}\" " +
            $"--start {frameIndex} --duration 1";

        var psi = new ProcessStartInfo
        {
            FileName = exePath,
            Arguments = arguments,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
            WorkingDirectory = Path.GetDirectoryName(exePath) ?? string.Empty,
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
