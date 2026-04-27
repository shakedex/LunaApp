using System.Diagnostics;
using System.Text.Json;
using LunaApp.Models;
using LunaApp.Services.Chappie;
using Serilog;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// ARRI camera support.
///
/// Container ≠ vendor. ARRI cameras can record ARRIRAW (<c>.ari</c>) or
/// ProRes (wrapped in <c>.mxf</c>, <c>.mov</c>, <c>.mp4</c>). Sony, Canon,
/// and Blackmagic also write <c>.mxf</c> / <c>.mov</c>. Extension alone is
/// not a vendor signal, so this CSP only claims <c>.ari</c> — the one
/// unambiguous ARRI-only container. Everything else routes to
/// <see cref="GenericCameraSupport"/>, where the metadata enrichers
/// (<see cref="ArriQuickTimeEnricher"/> for container tags;
/// <see cref="ArtCliMetadataEnricher"/> when art-cmd is installed) brand
/// any file carrying ARRI signals.
///
/// Status:
/// <list type="bullet">
///   <item>Without ART CLI: <c>.ari</c> processing returns Unsupported,
///         CSP row reads "Coming later — install ART CLI from Settings".</item>
///   <item>With ART CLI: <c>.ari</c> gets full metadata via
///         <see cref="ArtCliMetadataEnricher"/> + thumbnails via
///         <see cref="ArtCliThumbnailService"/>. Status flips to Ready.</item>
/// </list>
/// </summary>
public sealed class ArriCameraSupport : ICameraSupport
{
    private static readonly HashSet<string> Extensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".ari"
    };

    private const string ArtCliRequiredMsg = "ARRIRAW (.ari) needs ART CLI — install from Settings → Camera Support";

    private readonly ArtCliLocator _artCliLocator;
    private readonly IMetadataExtractor _baselineExtractor;
    private readonly IReadOnlyList<IMetadataEnricher> _enrichers;
    private readonly IReadOnlyList<IThumbnailGenerator> _thumbnailChain;

    public ArriCameraSupport(
        ArtCliLocator artCliLocator,
        IMetadataExtractor baselineExtractor,
        IEnumerable<IMetadataEnricher> enrichers,
        IEnumerable<IThumbnailGenerator> thumbnailGenerators)
    {
        _artCliLocator = artCliLocator;
        _baselineExtractor = baselineExtractor;
        _enrichers = enrichers.OrderBy(e => e.Priority).ToArray();
        _thumbnailChain = thumbnailGenerators.OrderBy(g => g.Priority).ToArray();
    }

    public string Id => "arri";
    public string DisplayName => "ARRI ALEXA (ARRIRAW, ProRes, MXF)";
    public IReadOnlySet<string> HandledExtensions => Extensions;

    public SupportStatus Status => _artCliLocator.Resolve() is { } install
        ? new SupportStatus.Ready($"art-cmd {install.Version}", "ARRI Reference Tool (installed)")
        : new SupportStatus.ComingLater("Install the ARRI Reference Tool to enable ARRIRAW + full ARRI MXF metadata");

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

        // Without ART CLI we can't decode ARRIRAW or read its rich metadata.
        // Return a clean Unsupported clip with a typed notice so the UI can
        // prompt the install.
        if (_artCliLocator.Resolve() is null)
        {
            Log.Information("ARRIRAW file {File} — ART CLI not installed; returning Unsupported notice", fileName);
            return new CameraClip
            {
                FilePath = filePath,
                FileName = fileName,
                FileSizeBytes = new FileInfo(filePath).Length,
                Container = "ARI",
                CameraManufacturer = "ARRI",
                ProcessingState = ClipProcessingState.Unsupported,
                UnsupportedNotice = new UnsupportedFormatNotice(
                    CameraSupportId: Id,
                    DisplayName: DisplayName,
                    Reason: ArtCliRequiredMsg),
            };
        }

        // ART CLI is installed — process via baseline + enrichers + thumbnail
        // chain, identical to GenericCameraSupport. ArtCliMetadataEnricher
        // fills in sensor/lens fields; ArtCliThumbnailService produces frames.
        CameraClip clip;
        try
        {
            clip = await _baselineExtractor.ExtractAsync(filePath, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error(ex, "ARRI baseline extraction failed for {File}", fileName);
            return new CameraClip
            {
                FilePath = filePath,
                FileName = fileName,
                FileSizeBytes = new FileInfo(filePath).Length,
                Container = "ARI",
                CameraManufacturer = "ARRI",
                ProcessingState = ClipProcessingState.Failed,
                ProcessingError = ex.Message,
            };
        }

        clip.CameraManufacturer ??= "ARRI";

        foreach (var enricher in _enrichers)
        {
            cancellationToken.ThrowIfCancellationRequested();
            try
            {
                if (!enricher.CanEnrich(filePath)) continue;
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
            foreach (var generator in _thumbnailChain)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var result = await generator.GenerateAsync(filePath, request, cancellationToken);
                clip.ThumbnailOutcome = result.Outcome;
                clip.ThumbnailOutcomeDetail = result.Detail;
                if (result.Outcome == ThumbnailOutcome.NoDecoder) continue;
                clip.Thumbnails = result.Frames.ToList();
                break;
            }
        }

        clip.ProcessingState = ClipProcessingState.Completed;
        return clip;
    }

    // =====================================================================
    //  Path-only ARRI detection — no content probe, so cheap to call from
    //  CanHandle during dispatch.
    // =====================================================================

    /// <summary>
    /// Spots ARRI footage via sidecars next to the file: an <c>.ale</c>
    /// companion file, or sibling <c>CLIPINFO/</c> / <c>ARRIINFO/</c>
    /// directories. These are ARRI-specific — no other vendor emits them.
    /// </summary>
    internal static bool HasArriSidecar(string filePath)
    {
        try
        {
            var dir = Path.GetDirectoryName(filePath) ?? "";
            var baseName = Path.GetFileNameWithoutExtension(filePath);
            if (string.IsNullOrEmpty(dir) || string.IsNullOrEmpty(baseName))
                return false;

            var sidecar = Path.Combine(dir, baseName + ".ale");
            if (File.Exists(sidecar)) return true;

            return Directory.Exists(Path.Combine(dir, "CLIPINFO")) ||
                   Directory.Exists(Path.Combine(dir, "ARRIINFO"));
        }
        catch
        {
            return false;
        }
    }

    // =====================================================================
    //  ART CLI helpers — invoked by ArtCliMetadataEnricher /
    //  ArtCliThumbnailService once the user has installed art-cmd via
    //  ArtCliInstaller. Path discovery lives in ArtCliLocator now.
    // =====================================================================

    /// <summary>
    /// Runs <c>art-cmd --mode export</c> against an ARRI clip and returns the
    /// path to the produced <c>metadata.json</c>. Verified against ART CMD
    /// v1.0.0 (JSON schema v2.1.0). Metadata-only export is selected by
    /// writing to a path with <c>.json</c> extension — art-cmd then skips
    /// audio / look exports.
    /// </summary>
    internal static async Task<string?> RunArtCliAsync(
        string artPath,
        string inputFile,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        var outputDir = Path.Combine(Path.GetTempPath(), $"luna_arri_{Guid.NewGuid():N}");
        Directory.CreateDirectory(outputDir);
        var metadataPath = Path.Combine(outputDir, "metadata.json");

        try
        {
            var arguments = $"--mode export --input \"{inputFile}\" --output \"{metadataPath}\"";
            var psi = new ProcessStartInfo
            {
                FileName = artPath,
                Arguments = arguments,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };

            using var process = Process.Start(psi);
            if (process == null) return null;

            using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeoutCts.CancelAfter(timeout);

            string stderr;
            try
            {
                _ = process.StandardOutput.ReadToEndAsync(timeoutCts.Token);
                stderr = await process.StandardError.ReadToEndAsync(timeoutCts.Token);
                await process.WaitForExitAsync(timeoutCts.Token);
            }
            catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
            {
                Log.Warning("ART CLI timed out after {Timeout}s on {File}",
                    timeout.TotalSeconds, Path.GetFileName(inputFile));
                try { process.Kill(); } catch { /* best effort */ }
                return null;
            }

            if (process.ExitCode != 0)
            {
                Log.Warning("ART CLI exited with code {ExitCode}. Stderr: {Stderr}", process.ExitCode, stderr);
                return null;
            }

            return File.Exists(metadataPath) ? metadataPath : null;
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "ART CLI execution failed");
            return null;
        }
    }

    internal static async Task ParseArriMetadataAsync(string metadataPath, CameraClip clip)
    {
        var json = await File.ReadAllTextAsync(metadataPath);
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;

        if (root.TryGetProperty("clipBasedMetadataSets", out var metadataSets))
        {
            foreach (var set in metadataSets.EnumerateArray())
            {
                if (!set.TryGetProperty("metadataSetName", out var setName) ||
                    !set.TryGetProperty("metadataSetPayload", out var payload))
                    continue;

                switch (setName.GetString())
                {
                    case "MXF Generic Data": ParseMxfGenericData(payload, clip); break;
                    case "Lens Device":      ParseLensDevice(payload, clip); break;
                    case "Slate Info":       ParseSlateInfo(payload, clip); break;
                    case "Clip Info":        ParseClipInfo(payload, clip); break;
                    case "Image Size":       ParseImageSize(payload, clip); break;
                    case "Project Rate":     ParseProjectRate(payload, clip); break;
                }
            }
        }

        if (root.TryGetProperty("frameBasedMetadata", out var frameMeta) &&
            frameMeta.TryGetProperty("frames", out var frames) &&
            frames.GetArrayLength() > 0)
        {
            var firstFrame = frames[0];

            if (firstFrame.TryGetProperty("timecode", out var tc))
                clip.Timecode = tc.GetString();

            if (firstFrame.TryGetProperty("frameBasedMetadataSets", out var frameMetaSets))
            {
                if (frameMetaSets.TryGetProperty("Sensor State", out var sensorState) &&
                    sensorState.ValueKind != JsonValueKind.Null)
                {
                    if (sensorState.TryGetProperty("exposureIndex", out var ei))
                        clip.Iso = ei.GetInt32();

                    if (sensorState.TryGetProperty("exposureTime", out var expTime))
                        clip.ShutterSpeed = expTime.GetString();

                    if (sensorState.TryGetProperty("sensorSampleRate", out var ssr))
                    {
                        var parts = (ssr.GetString() ?? "").Split('/');
                        if (parts.Length == 2 &&
                            double.TryParse(parts[0], out var num) &&
                            double.TryParse(parts[1], out var den) && den != 0)
                        {
                            clip.FrameRate = num / den;
                        }
                    }
                }

                if (frameMetaSets.TryGetProperty("Lens State", out var lensState) &&
                    lensState.ValueKind != JsonValueKind.Null)
                {
                    if (lensState.TryGetProperty("lensFocalLength", out var fl))
                    {
                        var focalMm = fl.GetDouble() / 1000.0;
                        clip.FocalLength = $"{focalMm:F0}mm";
                    }

                    if (lensState.TryGetProperty("lensIris", out var iris))
                    {
                        var irisValue = iris.GetInt32();
                        if (irisValue > 0)
                            clip.TStop = $"T{irisValue / 1000.0:F1}";
                    }
                }

                if (frameMetaSets.TryGetProperty("White Balance", out var wb) &&
                    wb.ValueKind != JsonValueKind.Null &&
                    wb.TryGetProperty("colorTemperature", out var ct))
                {
                    clip.WhiteBalance = ct.GetInt32();
                }
            }
        }
    }

    private static void ParseMxfGenericData(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("nativeIdentificationList", out var idList) &&
            idList.GetArrayLength() > 0 &&
            idList[0].TryGetProperty("productName", out var productName))
        {
            clip.CameraModel = productName.GetString();
        }
    }

    private static void ParseLensDevice(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("lensModel", out var lensModel))
            clip.Lens = lensModel.GetString();
    }

    private static void ParseSlateInfo(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("reelName", out var reelName))
            clip.ReelName = reelName.GetString();
    }

    private static void ParseClipInfo(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("clipDuration", out var duration))
            clip.Duration = TimeSpan.FromSeconds(duration.GetDouble());

        if (payload.TryGetProperty("videoCodec", out var codec))
            clip.Codec = codec.GetString();
    }

    private static void ParseImageSize(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("storedSize", out var storedSize))
        {
            if (storedSize.TryGetProperty("width", out var w)) clip.Width = w.GetInt32();
            if (storedSize.TryGetProperty("height", out var h)) clip.Height = h.GetInt32();
        }
    }

    private static void ParseProjectRate(JsonElement payload, CameraClip clip)
    {
        if (!payload.TryGetProperty("timebase", out var timebase)) return;

        var parts = (timebase.GetString() ?? "").Split('/');
        if (parts.Length == 2 &&
            double.TryParse(parts[0], out var num) &&
            double.TryParse(parts[1], out var den) &&
            den != 0 && clip.FrameRate == 0)
        {
            clip.FrameRate = num / den;
        }
    }
}
