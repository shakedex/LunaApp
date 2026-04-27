using LunaApp.Models;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Enriches clips with ARRI-specific metadata read straight from the
/// container. ARRI ALEXAs leave vendor markers in two distinct places
/// depending on the wrapper they're recording into:
///
/// 1. **QuickTime / MP4** (ALEXA Mini, Mini LF, 35 in Apple ProRes mode) —
///    <c>compatible_brands</c> contains <c>ARRI</c> and a rich
///    <c>com.arri.camera.*</c> key-value set carries camera model + serial,
///    ISO, shutter angle, white balance, reel, gamma, look, lens info.
///
/// 2. **MXF** (ALEXA 35, ALEXA Mini LF in MXF/ProRes or MXF/ARRIRAW) — the
///    QuickTime atoms aren't there. Instead the standard MXF Identification
///    Descriptor exposes <c>company_name=ARRI</c> + <c>product_name</c> (e.g.
///    "ALEXA 35") + <c>product_version</c>. Less rich than the QT path but
///    enough to brand the clip and surface the camera model.
///
/// Either signal is definitive — no other vendor writes them. The MXF path
/// here covers what we can without the ART CLI; per-frame sensor/lens data
/// lights up later when ART CLI distribution is resolved (see
/// <see cref="LunaApp.Services.CameraSupport.ArriCameraSupport"/>).
/// </summary>
public sealed class ArriQuickTimeEnricher : IMetadataEnricher
{
    private static readonly HashSet<string> Extensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mov", ".mp4", ".mxf", ".m4v"
    };

    private const string ArriTagPrefix = "com.arri.camera.";

    private readonly FfmpegFormatReader _reader;

    public ArriQuickTimeEnricher(FfmpegFormatReader reader)
    {
        _reader = reader;
    }

    // Run after the Sony XML enricher (Priority 110) so when both signals
    // appear, we don't clobber Sony with a false ARRI positive. ARRI detection
    // itself is strict (compatible_brands or com.arri.*) so collisions are
    // unlikely — this is just hygienic ordering.
    public int Priority => 120;

    public bool CanEnrich(string filePath) =>
        Extensions.Contains(Path.GetExtension(filePath));

    public Task EnrichAsync(CameraClip clip, CancellationToken cancellationToken)
    {
        var meta = _reader.ReadMetadata(clip.FilePath);
        if (meta is null) return Task.CompletedTask;

        // Path 1: QuickTime / MP4 — ARRI in compatible_brands, or any com.arri.camera.* tag.
        var hasQtSignals = meta.CompatibleBrands.Any(b => b.Equals("ARRI", StringComparison.OrdinalIgnoreCase))
                        || meta.Tags.Keys.Any(k => k.StartsWith(ArriTagPrefix, StringComparison.OrdinalIgnoreCase));

        // Path 2: MXF — Identification Descriptor's company_name. ARRI's ALEXA
        // 35 and Mini LF in MXF mode don't write the QT atoms, so this is the
        // only signal we get short of running ART CLI.
        var hasMxfSignal = meta.Tags.TryGetValue("company_name", out var companyName)
                       && companyName.Equals("ARRI", StringComparison.OrdinalIgnoreCase);

        if (!hasQtSignals && !hasMxfSignal) return Task.CompletedTask;

        clip.CameraManufacturer = "ARRI";

        // QuickTime-only fields (only present in MOV/MP4 ARRI files).
        if (TryGet(meta.Tags, "CameraModel", out var model))     clip.CameraModel = model;
        if (TryGet(meta.Tags, "CameraSerialNumber", out var sn)) clip.CameraSerial = sn;
        if (TryGet(meta.Tags, "ReelName", out var reel))         clip.ReelName ??= reel;
        if (TryGet(meta.Tags, "ColorGammaSxS", out var gamma))   clip.Gamma ??= gamma;

        if (TryGet(meta.Tags, "ExposureIndexAsa", out var iso) && int.TryParse(iso, out var isoValue))
            clip.Iso = isoValue;

        if (TryGet(meta.Tags, "WhiteBalanceKelvin", out var wb) && int.TryParse(wb, out var wbValue))
            clip.WhiteBalance = wbValue;

        // ARRI writes shutter angle in thousandths of a degree (1728 -> 172.8°).
        if (TryGet(meta.Tags, "ShutterAngle", out var sa) && int.TryParse(sa, out var saThousandths))
            clip.ShutterAngle = $"{saThousandths / 10.0:F1}°";

        if (TryGet(meta.Tags, "look.name", out var look))
            clip.LookName = look;

        if (TryGet(meta.Tags, "LensType", out var lensType) && !string.IsNullOrWhiteSpace(lensType))
            clip.Lens ??= lensType;

        // MXF Identification Descriptor fields. product_name is "ALEXA 35",
        // "ALEXA Mini LF", etc. — overrides MediaInfo's generic codec string.
        if (string.IsNullOrEmpty(clip.CameraModel)
            && meta.Tags.TryGetValue("product_name", out var productName)
            && !string.IsNullOrWhiteSpace(productName))
        {
            clip.CameraModel = productName.Trim();
        }

        // For ARRI MXF, read the transfer_characteristics SMPTE UL via
        // MediaInfo (FFmpeg reports "unknown" for ARRI's vendor ULs).
        // The 0E17... family is ARRI's registered space — Log-C 4 / LogC3.
        // This is the one field we can recover from MXF without ART CLI.
        if (hasMxfSignal && string.IsNullOrEmpty(clip.Gamma))
        {
            var ul = ReadMediaInfoVideoField(clip.FilePath, "transfer_characteristics");
            if (!string.IsNullOrEmpty(ul) && ul.StartsWith("0E17", StringComparison.OrdinalIgnoreCase))
                clip.Gamma = "Log-C";
        }

        // Timecode lives at container level as well as on the tmcd stream; prefer
        // whatever MediaInfo already set, but fall back to the container value.
        if (string.IsNullOrEmpty(clip.Timecode) && meta.Tags.TryGetValue("timecode", out var tc))
            clip.Timecode = tc;

        Log.Information("ARRI enrichment for {File}: {Model} · ISO {Iso} · {Shutter}",
            clip.FileName, clip.CameraModel ?? "ALEXA", clip.Iso, clip.ShutterAngle);

        return Task.CompletedTask;
    }

    /// <summary>Looks up a short ARRI tag name, fully-qualifying it with the <c>com.arri.camera.</c> prefix.</summary>
    private static bool TryGet(IReadOnlyDictionary<string, string> tags, string shortKey, out string value)
    {
        if (tags.TryGetValue(ArriTagPrefix + shortKey, out var v) && !string.IsNullOrWhiteSpace(v))
        {
            value = v;
            return true;
        }
        value = "";
        return false;
    }

    /// <summary>
    /// One-shot read of a single video-stream field via the MediaInfo native
    /// API. Used for ARRI's transfer_characteristics SMPTE UL, which FFmpeg
    /// can't decode but MediaInfo surfaces as a hex string.
    /// </summary>
    private static string? ReadMediaInfoVideoField(string filePath, string fieldName)
    {
        try
        {
            using var mi = new MediaInfo.MediaInfo();
            if (mi.Open(filePath) == 0) return null;
            try
            {
                var value = mi.Get(MediaInfo.StreamKind.Video, 0, fieldName);
                return string.IsNullOrWhiteSpace(value) ? null : value;
            }
            finally { mi.Close(); }
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "MediaInfo single-field probe failed for {Field} on {File}", fieldName, Path.GetFileName(filePath));
            return null;
        }
    }
}
