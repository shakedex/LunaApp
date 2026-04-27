using System.Xml.Linq;
using LunaApp.Models;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Sony metadata enricher that parses NonRealTimeMeta XML sidecar files companion
/// to Sony MXF clips. Adds camera model, ISO, shutter, lens, and white balance on
/// top of the MediaInfo baseline. Silently skips when no sidecar is found.
/// </summary>
public sealed class SonyXmlEnricher : IMetadataEnricher
{
    private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mxf", ".mp4", ".mov"
    };

    public int Priority => 110;

    public bool CanEnrich(string filePath)
    {
        if (!SupportedExtensions.Contains(Path.GetExtension(filePath))) return false;
        return HasSonySidecar(filePath);
    }

    public async Task EnrichAsync(CameraClip clip, CancellationToken cancellationToken)
    {
        var sidecarPath = FindSonySidecar(clip.FilePath);
        if (sidecarPath == null) return;

        clip.CameraManufacturer = "Sony";
        Log.Information("Parsing Sony sidecar: {SidecarPath}", sidecarPath);

        try
        {
            await ParseSonySidecarAsync(sidecarPath, clip, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to parse Sony sidecar {SidecarPath}", sidecarPath);
        }
    }

    public static bool HasSonySidecar(string filePath)
    {
        var sidecarPath = FindSonySidecar(filePath);
        return sidecarPath != null && File.Exists(sidecarPath);
    }

    private static string? FindSonySidecar(string filePath)
    {
        var dir = Path.GetDirectoryName(filePath) ?? "";
        var baseName = Path.GetFileNameWithoutExtension(filePath);

        var patterns = new[]
        {
            baseName + "M01.XML",
            baseName + "M01.xml",
            baseName + ".XML",
            baseName + ".xml",
            baseName + "_metadata.xml",
            baseName + "_metadata.XML",
        };

        foreach (var pattern in patterns)
        {
            var fullPath = Path.Combine(dir, pattern);
            if (File.Exists(fullPath) && VerifySonySidecar(fullPath))
                return fullPath;
        }

        var parentDir = Path.GetDirectoryName(dir);
        if (!string.IsNullOrEmpty(parentDir))
        {
            foreach (var pattern in patterns)
            {
                var fullPath = Path.Combine(parentDir, pattern);
                if (File.Exists(fullPath) && VerifySonySidecar(fullPath))
                    return fullPath;
            }
        }

        // Sony sometimes prefixes with camera model: "VENICE A005C039_201101OZM01.xml"
        try
        {
            var xmlFiles = Directory.GetFiles(dir, "*.xml", SearchOption.TopDirectoryOnly)
                .Concat(Directory.GetFiles(dir, "*.XML", SearchOption.TopDirectoryOnly))
                .Distinct(StringComparer.OrdinalIgnoreCase);

            foreach (var xmlFile in xmlFiles)
            {
                var xmlFileName = Path.GetFileName(xmlFile);
                if (xmlFileName.Contains(baseName, StringComparison.OrdinalIgnoreCase) &&
                    VerifySonySidecar(xmlFile))
                {
                    return xmlFile;
                }

                var xmlBaseName = Path.GetFileNameWithoutExtension(xmlFile);
                if ((xmlBaseName.StartsWith(baseName, StringComparison.OrdinalIgnoreCase) ||
                     baseName.StartsWith(xmlBaseName, StringComparison.OrdinalIgnoreCase)) &&
                    VerifySonySidecar(xmlFile))
                {
                    return xmlFile;
                }
            }
        }
        catch { /* best effort */ }

        return null;
    }

    private static bool VerifySonySidecar(string xmlPath)
    {
        try
        {
            var content = File.ReadAllText(xmlPath);
            return content.Contains("NonRealTimeMeta") ||
                   content.Contains("professionalDisc") ||
                   content.Contains("AcquisitionRecord") ||
                   content.Contains("CameraUnitMetadataSet") ||
                   content.Contains("LensUnitMetadataSet");
        }
        catch
        {
            return false;
        }
    }

    private static async Task ParseSonySidecarAsync(string sidecarPath, CameraClip clip, CancellationToken cancellationToken)
    {
        var xml = await File.ReadAllTextAsync(sidecarPath, cancellationToken);
        var doc = XDocument.Parse(xml);
        var root = doc.Root;
        if (root == null) return;

        var creationDate = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "CreationDate");
        if (creationDate != null)
        {
            var dateValue = creationDate.Attribute("value")?.Value;
            if (DateTime.TryParse(dateValue, out var date))
                clip.RecordedDate = date;
        }

        var videoFormat = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "VideoFormat");
        if (videoFormat != null)
        {
            var videoFrame = videoFormat.Descendants().FirstOrDefault(e => e.Name.LocalName == "VideoFrame");
            if (videoFrame != null)
            {
                var videoCodec = videoFrame.Attribute("videoCodec")?.Value;
                if (!string.IsNullOrEmpty(videoCodec))
                    clip.Codec = videoCodec;

                var captureFps = videoFrame.Attribute("captureFps")?.Value;
                if (!string.IsNullOrEmpty(captureFps) && double.TryParse(captureFps.TrimEnd('p', 'i'), out var fps))
                    clip.FrameRate = fps;
            }

            var videoLayout = videoFormat.Descendants().FirstOrDefault(e => e.Name.LocalName == "VideoLayout");
            if (videoLayout != null)
            {
                var pixelAttr = videoLayout.Attribute("pixel")?.Value;
                var linesAttr = videoLayout.Attribute("numOfVerticalLine")?.Value;

                if (!string.IsNullOrEmpty(pixelAttr) && int.TryParse(pixelAttr, out var width))
                    clip.Width = width;
                if (!string.IsNullOrEmpty(linesAttr) && int.TryParse(linesAttr, out var height))
                    clip.Height = height;
            }
        }

        var acquisitionRecord = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "AcquisitionRecord");
        if (acquisitionRecord != null)
        {
            var lensGroup = acquisitionRecord.Descendants().FirstOrDefault(e =>
                e.Name.LocalName == "Group" &&
                e.Attribute("name")?.Value == "LensUnitMetadataSet");
            if (lensGroup != null) ParseLensMetadata(lensGroup, clip);

            var cameraGroup = acquisitionRecord.Descendants().FirstOrDefault(e =>
                e.Name.LocalName == "Group" &&
                e.Attribute("name")?.Value == "CameraUnitMetadataSet");
            if (cameraGroup != null) ParseCameraMetadata(cameraGroup, clip);
        }

        var ltcChangeTable = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "LtcChangeTable");
        if (ltcChangeTable != null)
        {
            var ltcChange = ltcChangeTable.Descendants().FirstOrDefault(e => e.Name.LocalName == "LtcChange");
            var tcValue = ltcChange?.Attribute("value")?.Value;
            if (!string.IsNullOrEmpty(tcValue))
                clip.Timecode = FormatPackedTimecode(tcValue);
        }
    }

    /// <summary>
    /// Sony's <c>LtcChange/@value</c> is packed SMPTE timecode without
    /// separators (<c>HHMMSSFF</c>, e.g. <c>00595004</c>). Reshape it to the
    /// colon-separated form the rest of the app uses (<c>00:59:50:04</c>).
    /// Falls through unchanged for anything that isn't 8 ASCII digits.
    /// </summary>
    private static string FormatPackedTimecode(string raw)
    {
        if (raw.Length != 8) return raw;
        for (int i = 0; i < 8; i++)
            if (!char.IsAsciiDigit(raw[i])) return raw;
        return $"{raw[..2]}:{raw[2..4]}:{raw[4..6]}:{raw[6..]}";
    }

    private static void ParseLensMetadata(XElement lensGroup, CameraClip clip)
    {
        foreach (var item in lensGroup.Descendants().Where(e => e.Name.LocalName == "Item"))
        {
            var name = item.Attribute("name")?.Value;
            var value = item.Attribute("value")?.Value;
            if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(value)) continue;

            switch (name)
            {
                case "LensZoomActualFocalLength":
                    clip.FocalLength = value;
                    break;
                case "LensZoom35mmStillCameraEquivalent":
                    if (string.IsNullOrEmpty(clip.FocalLength))
                        clip.FocalLength = value;
                    break;
                case "IrisTNumber":
                    clip.TStop = value.StartsWith("T") ? value : $"T{value}";
                    break;
                case "IrisFNumber":
                    if (string.IsNullOrEmpty(clip.TStop))
                        clip.TStop = value.StartsWith("F") ? value : $"F{value}";
                    break;
                case "LensAttributes":
                    if (string.IsNullOrEmpty(clip.Lens) && value.Contains(' ') &&
                        !value.All(c => char.IsDigit(c) || c == '.'))
                    {
                        clip.Lens = value;
                    }
                    break;
            }
        }
    }

    private static void ParseCameraMetadata(XElement cameraGroup, CameraClip clip)
    {
        foreach (var item in cameraGroup.Descendants().Where(e => e.Name.LocalName == "Item"))
        {
            var name = item.Attribute("name")?.Value;
            var value = item.Attribute("value")?.Value;
            if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(value)) continue;

            switch (name)
            {
                case "ISOSensitivity":
                case "ExposureIndexOfPhotoMeter":
                    if ((clip.Iso ?? 0) == 0 && int.TryParse(value, out var iso))
                        clip.Iso = iso;
                    break;
                case "ShutterSpeed_Angle":
                    clip.ShutterAngle = value;
                    break;
                case "ShutterSpeed_Time":
                    clip.ShutterSpeed = value;
                    break;
                case "WhiteBalance":
                    if (int.TryParse(value.TrimEnd('K'), out var wb))
                        clip.WhiteBalance = wb;
                    break;
                case "CaptureFrameRate":
                    if (double.TryParse(value.TrimEnd('f', 'p', 's'), out var cfr))
                        clip.FrameRate = cfr;
                    break;
                case "CameraAttributes":
                    if (string.IsNullOrEmpty(clip.CameraModel))
                        clip.CameraModel = DeriveModelFromAttributes(value);
                    break;
            }
        }
    }

    /// <summary>
    /// Maps Sony service-code prefixes (the "MPC-NNNN" inside
    /// <c>CameraAttributes</c>) to a friendly model name. These are the
    /// codes Sony assigns at the factory; they're stable across firmware.
    /// </summary>
    private static string DeriveModelFromAttributes(string attributes)
    {
        // MPC-3610 = Venice 2, MPC-2610 = Burano (originally also used by
        // some F-series; Burano is current usage). Order matters — Burano's
        // X-OCN_G codec hint is the disambiguator if both ever collide.
        if (attributes.Contains("MPC-3610") || attributes.Contains("3610")) return "Sony VENICE 2";
        if (attributes.Contains("MPC-2610") || attributes.Contains("2610")) return "Sony BURANO";
        if (attributes.Contains("ILME-FX9") || attributes.Contains("FX9")) return "Sony FX9";
        if (attributes.Contains("ILME-FX6") || attributes.Contains("FX6")) return "Sony FX6";
        if (attributes.Contains("ILME-FX3") || attributes.Contains("FX3")) return "Sony FX3";
        if (attributes.Contains("A7S")) return "Sony A7S";
        return attributes;
    }
}
