using System.Xml.Linq;
using LunaApp.Models;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Sony clip processor using XML sidecar files for metadata extraction.
/// Parses NonRealTimeMeta XML files that accompany Sony MXF clips.
/// </summary>
public sealed class SonyClipProcessor : IClipProcessor
{
    private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mxf", ".mp4", ".mov"
    };
    
    private static readonly XNamespace SonyNs = "urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00";
    
    public int Priority => 110; // Higher than ARRI - sidecar check is definitive
    
    /// <summary>
    /// Check if file has Sony sidecar metadata
    /// </summary>
    public static bool HasSonySidecar(string filePath)
    {
        var sidecarPath = FindSonySidecar(filePath);
        return sidecarPath != null && File.Exists(sidecarPath);
    }
    
    /// <summary>
    /// Find Sony sidecar XML file path
    /// </summary>
    private static string? FindSonySidecar(string filePath)
    {
        var dir = Path.GetDirectoryName(filePath) ?? "";
        var baseName = Path.GetFileNameWithoutExtension(filePath);
        
        // Common Sony sidecar naming patterns (case-insensitive search)
        var patterns = new[]
        {
            baseName + "M01.XML",
            baseName + "M01.xml",
            baseName + ".XML",
            baseName + ".xml",
            baseName + "_metadata.xml",
            baseName + "_metadata.XML",
        };
        
        // Check in same directory
        foreach (var pattern in patterns)
        {
            var fullPath = Path.Combine(dir, pattern);
            if (File.Exists(fullPath))
            {
                if (VerifySonySidecar(fullPath))
                    return fullPath;
            }
        }
        
        // Check in parent directory (some card structures have XML in parent)
        var parentDir = Path.GetDirectoryName(dir);
        if (!string.IsNullOrEmpty(parentDir))
        {
            foreach (var pattern in patterns)
            {
                var fullPath = Path.Combine(parentDir, pattern);
                if (File.Exists(fullPath))
                {
                    if (VerifySonySidecar(fullPath))
                        return fullPath;
                }
            }
        }
        
        // Also check for any XML files containing the clip name in same directory
        // Sony sometimes prefixes with camera model: "VENICE A005C039_201101OZM01.xml"
        try
        {
            var xmlFiles = Directory.GetFiles(dir, "*.xml", SearchOption.TopDirectoryOnly)
                .Concat(Directory.GetFiles(dir, "*.XML", SearchOption.TopDirectoryOnly))
                .Distinct(StringComparer.OrdinalIgnoreCase);
            
            foreach (var xmlFile in xmlFiles)
            {
                var xmlFileName = Path.GetFileName(xmlFile);
                
                // Check if XML filename contains the clip base name
                // Handles: "VENICE A005C039_201101OZM01.xml" for clip "A005C039_201101OZ.mxf"
                if (xmlFileName.Contains(baseName, StringComparison.OrdinalIgnoreCase))
                {
                    if (VerifySonySidecar(xmlFile))
                        return xmlFile;
                }
                
                // Also check if clip name starts with XML base (without camera prefix)
                var xmlBaseName = Path.GetFileNameWithoutExtension(xmlFile);
                if (xmlBaseName.StartsWith(baseName, StringComparison.OrdinalIgnoreCase) ||
                    baseName.StartsWith(xmlBaseName, StringComparison.OrdinalIgnoreCase))
                {
                    if (VerifySonySidecar(xmlFile))
                        return xmlFile;
                }
            }
        }
        catch { }
        
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
    
    public bool CanProcess(string filePath)
    {
        var ext = Path.GetExtension(filePath);
        if (!SupportedExtensions.Contains(ext))
            return false;
        
        // Quick check: does a Sony sidecar exist?
        return HasSonySidecar(filePath);
    }
    
    public async Task<CameraClip> ExtractMetadataAsync(string filePath, CancellationToken cancellationToken = default)
    {
        var fileInfo = new FileInfo(filePath);
        var clip = new CameraClip
        {
            FilePath = filePath,
            FileName = fileInfo.Name,
            FileSizeBytes = fileInfo.Length,
            CameraManufacturer = "Sony"
        };
        
        try
        {
            var sidecarPath = FindSonySidecar(filePath);
            
            if (sidecarPath == null)
            {
                Log.Warning("Sony sidecar not found for {FilePath}", filePath);
                
                // Fall back to LibVLC for basic metadata
                await FillMetadataFromLibVlcAsync(clip, cancellationToken);
                clip.ProcessingState = ClipProcessingState.Completed;
                return clip;
            }
            
            Log.Information("Parsing Sony sidecar: {SidecarPath}", sidecarPath);
            await ParseSonySidecarAsync(sidecarPath, clip, cancellationToken);
            
            // If we don't have duration/dimensions from sidecar, use LibVLC
            if (clip.Duration == TimeSpan.Zero || clip.Width == 0)
            {
                await FillMetadataFromLibVlcAsync(clip, cancellationToken);
            }
            
            clip.ProcessingState = ClipProcessingState.Completed;
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to extract Sony metadata from {FilePath}", filePath);
            clip.ProcessingState = ClipProcessingState.Failed;
            clip.ProcessingError = ex.Message;
        }
        
        return clip;
    }
    
    public async Task<List<ThumbnailFrame>> ExtractThumbnailsAsync(
        string filePath, 
        TimeSpan duration,
        int count = 3,
        int width = 480,
        CancellationToken cancellationToken = default)
    {
        // Delegate to LibVLC for thumbnail extraction
        using var libVlc = new LibVlcClipProcessor();
        return await libVlc.ExtractThumbnailsAsync(filePath, duration, count, width, cancellationToken);
    }
    
    private static async Task FillMetadataFromLibVlcAsync(CameraClip clip, CancellationToken cancellationToken)
    {
        try
        {
            using var libVlc = new LibVlcClipProcessor();
            var vlcClip = await libVlc.ExtractMetadataAsync(clip.FilePath, cancellationToken);
            
            // Fill missing fields
            if (clip.Duration == TimeSpan.Zero)
                clip.Duration = vlcClip.Duration;
            if (clip.Width == 0)
                clip.Width = vlcClip.Width;
            if (clip.Height == 0)
                clip.Height = vlcClip.Height;
            if (clip.FrameRate == 0)
                clip.FrameRate = vlcClip.FrameRate;
            if (string.IsNullOrEmpty(clip.Codec))
                clip.Codec = vlcClip.Codec;
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "LibVLC fallback failed for {FileName}", clip.FileName);
        }
    }
    
    private async Task ParseSonySidecarAsync(string sidecarPath, CameraClip clip, CancellationToken cancellationToken)
    {
        var xml = await File.ReadAllTextAsync(sidecarPath, cancellationToken);
        var doc = XDocument.Parse(xml);
        var root = doc.Root;
        
        if (root == null) return;
        
        // Determine namespace
        var ns = root.GetDefaultNamespace();
        if (ns == XNamespace.None)
        {
            ns = SonyNs;
        }
        
        // Duration
        var durationElement = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "Duration");
        if (durationElement != null)
        {
            var durationValue = durationElement.Attribute("value")?.Value;
            if (!string.IsNullOrEmpty(durationValue) && int.TryParse(durationValue, out var frames))
            {
                // Duration is typically in frames, need frame rate to convert
            }
        }
        
        // Creation Date
        var creationDate = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "CreationDate");
        if (creationDate != null)
        {
            var dateValue = creationDate.Attribute("value")?.Value;
            if (DateTime.TryParse(dateValue, out var date))
            {
                clip.RecordedDate = date;
            }
        }
        
        // Video Format
        var videoFormat = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "VideoFormat");
        if (videoFormat != null)
        {
            var videoFrame = videoFormat.Descendants().FirstOrDefault(e => e.Name.LocalName == "VideoFrame");
            if (videoFrame != null)
            {
                // Video codec
                var videoCodec = videoFrame.Attribute("videoCodec")?.Value;
                if (!string.IsNullOrEmpty(videoCodec))
                {
                    clip.Codec = videoCodec;
                }
                
                // Capture frame rate
                var captureFps = videoFrame.Attribute("captureFps")?.Value;
                if (!string.IsNullOrEmpty(captureFps) && double.TryParse(captureFps.TrimEnd('p', 'i'), out var fps))
                {
                    clip.FrameRate = fps;
                }
            }
            
            // Resolution from VideoLayout
            var videoLayout = videoFormat.Descendants().FirstOrDefault(e => e.Name.LocalName == "VideoLayout");
            if (videoLayout != null)
            {
                // pixel = width, numOfVerticalLine = height
                var pixelAttr = videoLayout.Attribute("pixel")?.Value;
                var linesAttr = videoLayout.Attribute("numOfVerticalLine")?.Value;
                
                Log.Debug("Sony VideoLayout: pixel={Pixel}, numOfVerticalLine={Lines}", pixelAttr, linesAttr);
                
                if (!string.IsNullOrEmpty(pixelAttr) && int.TryParse(pixelAttr, out var width))
                    clip.Width = width;
                if (!string.IsNullOrEmpty(linesAttr) && int.TryParse(linesAttr, out var height))
                    clip.Height = height;
            }
            else
            {
                Log.Debug("Sony VideoLayout not found in sidecar");
            }
        }
        
        // Find AcquisitionRecord groups
        var acquisitionRecord = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "AcquisitionRecord");
        if (acquisitionRecord != null)
        {
            // Lens metadata
            var lensGroup = acquisitionRecord.Descendants()
                .FirstOrDefault(e => e.Name.LocalName == "Group" && 
                                     e.Attribute("name")?.Value == "LensUnitMetadataSet");
            if (lensGroup != null)
            {
                ParseLensMetadata(lensGroup, clip);
            }
            
            // Camera metadata
            var cameraGroup = acquisitionRecord.Descendants()
                .FirstOrDefault(e => e.Name.LocalName == "Group" && 
                                     e.Attribute("name")?.Value == "CameraUnitMetadataSet");
            if (cameraGroup != null)
            {
                ParseCameraMetadata(cameraGroup, clip);
            }
        }
        
        // LTC Timecode
        var ltcChangeTable = root.Descendants().FirstOrDefault(e => e.Name.LocalName == "LtcChangeTable");
        if (ltcChangeTable != null)
        {
            var ltcChange = ltcChangeTable.Descendants().FirstOrDefault(e => e.Name.LocalName == "LtcChange");
            if (ltcChange != null)
            {
                var tcValue = ltcChange.Attribute("value")?.Value;
                if (!string.IsNullOrEmpty(tcValue))
                {
                    clip.Timecode = tcValue;
                }
            }
        }
    }
    
    private void ParseLensMetadata(XElement lensGroup, CameraClip clip)
    {
        foreach (var item in lensGroup.Descendants().Where(e => e.Name.LocalName == "Item"))
        {
            var name = item.Attribute("name")?.Value;
            var value = item.Attribute("value")?.Value;
            
            if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(value))
                continue;
            
            Log.Debug("Sony lens metadata: {Name} = {Value}", name, value);
            
            switch (name)
            {
                case "LensZoomActualFocalLength":
                    clip.FocalLength = value; // e.g., "50.00mm"
                    break;
                    
                case "LensZoom35mmStillCameraEquivalent":
                    // Fallback if actual focal length not available
                    if (string.IsNullOrEmpty(clip.FocalLength))
                        clip.FocalLength = value;
                    break;
                    
                case "FocusPositionFromImagePlane":
                    // Focus distance, e.g., "2.924m"
                    // Could store in a notes field
                    break;
                    
                case "IrisTNumber":
                    clip.TStop = value.StartsWith("T") ? value : $"T{value}";
                    break;
                    
                case "IrisFNumber":
                    // Only use F-number if T-stop not already set
                    if (string.IsNullOrEmpty(clip.TStop))
                        clip.TStop = value.StartsWith("F") ? value : $"F{value}";
                    break;
                    
                case "LensAttributes":
                    // LensAttributes is often a serial number like "F08000183" or "2050.0212"
                    // Only use it as lens name if it looks like a real lens name (contains letters and spaces)
                    if (string.IsNullOrEmpty(clip.Lens) && value.Contains(' ') && !value.All(c => char.IsDigit(c) || c == '.'))
                    {
                        clip.Lens = value;
                    }
                    break;
            }
        }
    }
    
    private void ParseCameraMetadata(XElement cameraGroup, CameraClip clip)
    {
        foreach (var item in cameraGroup.Descendants().Where(e => e.Name.LocalName == "Item"))
        {
            var name = item.Attribute("name")?.Value;
            var value = item.Attribute("value")?.Value;
            
            if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(value))
                continue;
            
            Log.Debug("Sony camera metadata: {Name} = {Value}", name, value);
            
            switch (name)
            {
                case "ISOSensitivity":
                case "ExposureIndexOfPhotoMeter":
                    if (clip.Iso == 0 && int.TryParse(value, out var iso))
                    {
                        clip.Iso = iso;
                    }
                    break;
                    
                case "ShutterSpeed_Angle":
                    clip.ShutterAngle = value; // e.g., "180.00deg"
                    break;
                    
                case "ShutterSpeed_Time":
                    clip.ShutterSpeed = value; // e.g., "1/48"
                    break;
                    
                case "WhiteBalance":
                    if (int.TryParse(value.TrimEnd('K'), out var wb))
                    {
                        clip.WhiteBalance = wb;
                    }
                    break;
                    
                case "CaptureFrameRate":
                    if (double.TryParse(value.TrimEnd('f', 'p', 's'), out var cfr))
                    {
                        clip.FrameRate = cfr;
                    }
                    break;
                    
                case "NeutralDensityFilterWheelSetting":
                    // ND filter value, could store
                    break;
                    
                case "CameraAttributes":
                    // e.g., "MPC-3610" - can be used to derive camera model
                    if (string.IsNullOrEmpty(clip.CameraModel))
                    {
                        clip.CameraModel = DeriveModelFromAttributes(value);
                    }
                    break;
            }
        }
    }
    
    private static string DeriveModelFromAttributes(string attributes)
    {
        // Map Sony camera attributes to friendly model names
        if (attributes.Contains("3610")) return "Sony VENICE 2";
        if (attributes.Contains("2610")) return "Sony VENICE";
        if (attributes.Contains("3100")) return "Sony BURANO";
        if (attributes.Contains("FX9")) return "Sony FX9";
        if (attributes.Contains("FX6")) return "Sony FX6";
        if (attributes.Contains("FX3")) return "Sony FX3";
        if (attributes.Contains("A7S")) return "Sony A7S";
        
        return attributes;
    }
}
