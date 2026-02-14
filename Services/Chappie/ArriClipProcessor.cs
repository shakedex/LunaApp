using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;
using LunaApp.Models;
using Serilog;
using SkiaSharp;

namespace LunaApp.Services.Chappie;

/// <summary>
/// ARRI clip processor using ART CLI for metadata extraction.
/// Parses metadata.json output from ART tool.
/// </summary>
public sealed class ArriClipProcessor : IClipProcessor
{
    private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mxf", ".mov", ".ari", ".mp4"
    };
    
    public int Priority => 100; // High priority for ARRI files
    
    /// <summary>
    /// Check if file has ARRI sidecar metadata
    /// </summary>
    public static bool HasArriSidecar(string filePath)
    {
        // ARRI clips often have companion files
        var dir = Path.GetDirectoryName(filePath) ?? "";
        var baseName = Path.GetFileNameWithoutExtension(filePath);
        
        // Check for ARRI metadata XML
        var arriXml = Path.Combine(dir, baseName + ".xml");
        if (File.Exists(arriXml))
        {
            try
            {
                var content = File.ReadAllText(arriXml, System.Text.Encoding.UTF8);
                return content.Contains("ARRI", StringComparison.OrdinalIgnoreCase) ||
                       content.Contains("alexa", StringComparison.OrdinalIgnoreCase);
            }
            catch { }
        }
        
        return false;
    }
    
    public bool CanProcess(string filePath)
    {
        var ext = Path.GetExtension(filePath);
        if (!SupportedExtensions.Contains(ext))
            return false;
        
        // Check if ART CLI is available
        var artPath = GetArtCliPath();
        if (artPath == null || !File.Exists(artPath))
            return false;
        
        // If Sony sidecar exists, skip - let Sony processor handle it
        if (SonyClipProcessor.HasSonySidecar(filePath))
            return false;
        
        // If ARRI sidecar exists, definitely try
        if (HasArriSidecar(filePath))
            return true;
        
        // For MXF files without sidecars, check filename patterns that suggest ARRI
        var fileName = Path.GetFileName(filePath).ToUpperInvariant();
        
        // ARRI naming patterns: A001C001_, DW0001C027_, etc.
        // Pattern: letters/digits followed by C + digits (clip number)
        if (fileName.Contains("C0") || fileName.Contains("C1") || fileName.Contains("C2"))
        {
            // Look for typical ARRI reel+clip pattern
            var underscoreIdx = fileName.IndexOf('_');
            if (underscoreIdx > 5)
            {
                var prefix = fileName[..underscoreIdx];
                var cIdx = prefix.LastIndexOf('C');
                if (cIdx > 2 && cIdx < prefix.Length - 2 &&
                    char.IsDigit(prefix[cIdx + 1]))
                {
                    return true;
                }
            }
        }
        
        // For .ari files, always try ARRI
        if (ext.Equals(".ari", StringComparison.OrdinalIgnoreCase))
            return true;
        
        // Don't try other MOV/MP4 files with ART CLI - too slow and usually not ARRI
        return false;
    }
    
    public async Task<CameraClip> ExtractMetadataAsync(string filePath, CancellationToken cancellationToken = default)
    {
        var fileInfo = new FileInfo(filePath);
        var clip = new CameraClip
        {
            FilePath = filePath,
            FileName = fileInfo.Name,
            FileSizeBytes = fileInfo.Length,
            CameraManufacturer = "ARRI"
        };
        
        try
        {
            var artPath = GetArtCliPath();
            var artSuccess = false;
            
            if (artPath != null && File.Exists(artPath))
            {
                Log.Debug("Using ART CLI at {ArtPath} for {FileName}", artPath, clip.FileName);
                
                // Create temp output directory
                var outputDir = Path.Combine(Path.GetTempPath(), $"luna_arri_{Guid.NewGuid()}");
                Directory.CreateDirectory(outputDir);
                
                try
                {
                    // Run ART CLI
                    var success = await RunArtCliAsync(artPath, filePath, outputDir, cancellationToken);
                    
                    if (success)
                    {
                        // Parse metadata.json
                        var metadataPath = Path.Combine(outputDir, "metadata.json");
                        if (File.Exists(metadataPath))
                        {
                            await ParseArriMetadataAsync(metadataPath, clip);
                            artSuccess = true;
                            Log.Information("ART CLI extracted metadata for {FileName}: {Camera} {Lens}", 
                                clip.FileName, clip.CameraModel, clip.Lens);
                        }
                        else
                        {
                            Log.Warning("ART CLI did not produce metadata.json for {FileName}", clip.FileName);
                        }
                    }
                }
                finally
                {
                    // Cleanup temp directory
                    try { Directory.Delete(outputDir, true); } catch { }
                }
            }
            else
            {
                Log.Debug("ART CLI not found at {ArtPath}", artPath ?? "null");
            }
            
            // If ART CLI didn't give us duration/dimensions, use LibVLC as fallback
            if (clip.Duration == TimeSpan.Zero || clip.Width == 0)
            {
                Log.Debug("Using LibVLC fallback for duration/dimensions on {FileName}", clip.FileName);
                await FillMissingMetadataFromLibVlcAsync(clip, cancellationToken);
            }
            
            // If we still don't have basic metadata, try filename parsing
            if (!artSuccess && string.IsNullOrEmpty(clip.ReelName))
            {
                ExtractBasicMetadataFromFilename(clip);
            }
            
            clip.ProcessingState = ClipProcessingState.Completed;
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to extract ARRI metadata from {FilePath}", filePath);
            clip.ProcessingState = ClipProcessingState.Failed;
            clip.ProcessingError = ex.Message;
        }
        
        return clip;
    }
    
    private static async Task FillMissingMetadataFromLibVlcAsync(CameraClip clip, CancellationToken cancellationToken)
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
    
    private static void ExtractBasicMetadataFromFilename(CameraClip clip)
    {
        var fileName = Path.GetFileNameWithoutExtension(clip.FileName);
        
        // ARRI naming: A001C001_XXXXXX or DW0001C027_XXXXXX
        var parts = fileName.Split('_');
        if (parts.Length >= 1)
        {
            var firstPart = parts[0];
            var cIndex = firstPart.IndexOf('C');
            if (cIndex > 0)
            {
                clip.ReelName = firstPart[..cIndex]; // e.g., "A001" or "DW0001"
            }
        }
    }
    
    public async Task<List<ThumbnailFrame>> ExtractThumbnailsAsync(
        string filePath, 
        TimeSpan duration,
        int count = 3,
        int width = 480,
        CancellationToken cancellationToken = default)
    {
        var thumbnails = new List<ThumbnailFrame>();
        var artPath = GetArtCliPath();
        
        if (artPath == null || !File.Exists(artPath))
        {
            Log.Debug("ART CLI not available for thumbnail extraction");
            return thumbnails;
        }
        
        // Use ART CLI process mode to render frames (LibVLC cannot decode ARRIRAW)
        var outputDir = Path.Combine(Path.GetTempPath(), $"luna_arri_thumb_{Guid.NewGuid()}");
        Directory.CreateDirectory(outputDir);
        
        try
        {
            // Render 1 frame using process mode - ART CLI will output to outputDir/clipname/0000000.exr
            var arguments = $"--mode process --input \"{filePath}\" --output \"{outputDir}/\" --duration 1 --skip-audio";
            Log.Debug("Running ART CLI for thumbnail: {Args}", arguments);
            
            var psi = new ProcessStartInfo
            {
                FileName = artPath,
                Arguments = arguments,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };
            
            using var process = Process.Start(psi);
            if (process == null)
            {
                return thumbnails;
            }
            
            // Set timeout for thumbnail generation (30 seconds max)
            using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeoutCts.CancelAfter(TimeSpan.FromSeconds(30));
            
            try
            {
                await process.WaitForExitAsync(timeoutCts.Token);
            }
            catch (OperationCanceledException)
            {
                Log.Warning("ART CLI thumbnail generation timed out for {File}", Path.GetFileName(filePath));
                try { process.Kill(); } catch { }
                return thumbnails;
            }
            
            if (process.ExitCode != 0)
            {
                var stderr = await process.StandardError.ReadToEndAsync(cancellationToken);
                Log.Debug("ART CLI thumbnail render failed with exit code {Code}: {Error}", process.ExitCode, stderr);
                return thumbnails;
            }
            
            // Find exported image files recursively (ART CLI creates subdirectory with clip name)
            var imageFiles = Directory.GetFiles(outputDir, "*.*", SearchOption.AllDirectories)
                .Where(f => f.EndsWith(".exr", StringComparison.OrdinalIgnoreCase) ||
                           f.EndsWith(".tiff", StringComparison.OrdinalIgnoreCase) ||
                           f.EndsWith(".tif", StringComparison.OrdinalIgnoreCase) ||
                           f.EndsWith(".jpg", StringComparison.OrdinalIgnoreCase) ||
                           f.EndsWith(".png", StringComparison.OrdinalIgnoreCase))
                .OrderBy(f => f)
                .Take(count)
                .ToList();
            
            Log.Debug("ART CLI created {Count} image files in {Dir}", imageFiles.Count, outputDir);
            
            if (imageFiles.Count == 0)
            {
                // Check what files exist for debugging
                var allFiles = Directory.GetFiles(outputDir, "*.*", SearchOption.AllDirectories);
                Log.Debug("ART CLI output directory contains: {Files}", string.Join(", ", allFiles.Select(f => Path.GetRelativePath(outputDir, f))));
            }
            
            foreach (var imageFile in imageFiles)
            {
                try
                {
                    var thumbnail = await ConvertToThumbnailAsync(imageFile, width, TimeSpan.Zero);
                    if (thumbnail != null)
                    {
                        thumbnails.Add(thumbnail);
                    }
                }
                catch (Exception ex)
                {
                    Log.Debug(ex, "Failed to convert thumbnail from {File}", imageFile);
                }
            }
            
            Log.Debug("Extracted {Count} ARRI thumbnails via ART CLI", thumbnails.Count);
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "ART CLI thumbnail extraction failed");
        }
        finally
        {
            // Cleanup temp directory
            try
            {
                if (Directory.Exists(outputDir))
                {
                    Directory.Delete(outputDir, recursive: true);
                }
            }
            catch { }
        }
        
        return thumbnails;
    }
    
    private static async Task<ThumbnailFrame?> ConvertToThumbnailAsync(string imagePath, int targetWidth, TimeSpan timestamp)
    {
        return await Task.Run(() =>
        {
            try
            {
                using var originalBitmap = SKBitmap.Decode(imagePath);
                if (originalBitmap == null)
                {
                    Log.Debug("Failed to decode image: {Path}", imagePath);
                    return null;
                }
                
                // Calculate target dimensions
                var aspectRatio = originalBitmap.Width / (double)originalBitmap.Height;
                var targetHeight = (int)(targetWidth / aspectRatio);
                
                // Resize
                using var resized = originalBitmap.Resize(new SKImageInfo(targetWidth, targetHeight), new SKSamplingOptions(SKFilterMode.Linear));
                if (resized == null) return null;
                
                using var image = SKImage.FromBitmap(resized);
                using var data = image.Encode(SKEncodedImageFormat.Webp, 80);
                
                return new ThumbnailFrame
                {
                    Position = 0.0, // First frame
                    ImageBase64 = Convert.ToBase64String(data.ToArray()),
                    FrameNumber = 0
                };
            }
            catch (Exception ex)
            {
                Log.Debug(ex, "Failed to process thumbnail image");
                return null;
            }
        });
    }
    
    private static string? GetArtCliPath()
    {
        var appDir = AppDomain.CurrentDomain.BaseDirectory;
        
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            return Path.Combine(appDir, "tools", "arri", "win-x64", "art-cmd.exe");
        }
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
        {
            return Path.Combine(appDir, "tools", "arri", "osx-arm64", "art-cmd");
        }
        
        return null;
    }
    
    private static async Task<bool> RunArtCliAsync(string artPath, string inputFile, string outputDir, CancellationToken cancellationToken)
    {
        try
        {
            var arguments = $"--mode export --input \"{inputFile}\" --output \"{outputDir}/\" --skip-audio";
            Log.Debug("Running ART CLI: {ArtPath} {Arguments}", artPath, arguments);
            
            var psi = new ProcessStartInfo
            {
                FileName = artPath,
                Arguments = arguments,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };
            
            using var process = Process.Start(psi);
            if (process == null)
            {
                Log.Warning("Failed to start ART CLI process");
                return false;
            }
            
            var stdout = await process.StandardOutput.ReadToEndAsync(cancellationToken);
            var stderr = await process.StandardError.ReadToEndAsync(cancellationToken);
            
            await process.WaitForExitAsync(cancellationToken);
            
            if (process.ExitCode != 0)
            {
                Log.Warning("ART CLI exited with code {ExitCode}. Stderr: {Stderr}", process.ExitCode, stderr);
                return false;
            }
            
            // Check if metadata.json was created
            var metadataPath = Path.Combine(outputDir, "metadata.json");
            if (File.Exists(metadataPath))
            {
                Log.Debug("ART CLI created metadata.json at {Path}", metadataPath);
                return true;
            }
            
            // List output directory contents for debugging
            var files = Directory.GetFiles(outputDir);
            Log.Debug("ART CLI output directory contains: {Files}", string.Join(", ", files.Select(Path.GetFileName)));
            
            return files.Any(f => f.EndsWith(".json", StringComparison.OrdinalIgnoreCase));
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "ART CLI execution failed");
            return false;
        }
    }
    
    private static async Task ParseArriMetadataAsync(string metadataPath, CameraClip clip)
    {
        var json = await File.ReadAllTextAsync(metadataPath);
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;
        
        // Parse clipBasedMetadataSets array
        if (root.TryGetProperty("clipBasedMetadataSets", out var metadataSets))
        {
            foreach (var set in metadataSets.EnumerateArray())
            {
                if (!set.TryGetProperty("metadataSetName", out var setName) ||
                    !set.TryGetProperty("metadataSetPayload", out var payload))
                    continue;
                
                var name = setName.GetString();
                
                switch (name)
                {
                    case "MXF Generic Data":
                        ParseMxfGenericData(payload, clip);
                        break;
                    case "Lens Device":
                        ParseLensDevice(payload, clip);
                        break;
                    case "Slate Info":
                        ParseSlateInfo(payload, clip);
                        break;
                    case "Clip Info":
                        ParseClipInfo(payload, clip);
                        break;
                    case "Image Size":
                        ParseImageSize(payload, clip);
                        break;
                    case "Project Rate":
                        ParseProjectRate(payload, clip);
                        break;
                }
            }
        }
        
        // Parse frame-based metadata
        if (root.TryGetProperty("frameBasedMetadata", out var frameMeta) &&
            frameMeta.TryGetProperty("frames", out var frames) &&
            frames.GetArrayLength() > 0)
        {
            var firstFrame = frames[0];
            
            // Timecode
            if (firstFrame.TryGetProperty("timecode", out var tc))
            {
                clip.Timecode = tc.GetString();
            }
            
            // Parse frame metadata sets
            if (firstFrame.TryGetProperty("frameBasedMetadataSets", out var frameMetaSets))
            {
                // Sensor State
                if (frameMetaSets.TryGetProperty("Sensor State", out var sensorState) && 
                    sensorState.ValueKind != JsonValueKind.Null)
                {
                    if (sensorState.TryGetProperty("exposureIndex", out var ei))
                        clip.Iso = ei.GetInt32();
                    
                    if (sensorState.TryGetProperty("exposureTime", out var expTime))
                        clip.ShutterSpeed = expTime.GetString();
                    
                    if (sensorState.TryGetProperty("sensorSampleRate", out var ssr))
                    {
                        // sensorSampleRate is like "24/1"
                        var ssrStr = ssr.GetString() ?? "";
                        var parts = ssrStr.Split('/');
                        if (parts.Length == 2 && 
                            double.TryParse(parts[0], out var num) && 
                            double.TryParse(parts[1], out var den) && 
                            den != 0)
                        {
                            clip.FrameRate = num / den;
                        }
                    }
                }
                
                // Lens State
                if (frameMetaSets.TryGetProperty("Lens State", out var lensState) &&
                    lensState.ValueKind != JsonValueKind.Null)
                {
                    // Focal length (stored in µm, convert to mm)
                    if (lensState.TryGetProperty("lensFocalLength", out var fl))
                    {
                        var focalMm = fl.GetDouble() / 1000.0;
                        clip.FocalLength = $"{focalMm:F0}mm";
                    }
                    
                    // Iris (stored as 1/1000 stop, T1 = 1000)
                    if (lensState.TryGetProperty("lensIris", out var iris))
                    {
                        var irisValue = iris.GetInt32();
                        if (irisValue > 0)
                        {
                            var tStop = irisValue / 1000.0;
                            clip.TStop = $"T{tStop:F1}";
                        }
                    }
                }
                
                // White Balance
                if (frameMetaSets.TryGetProperty("White Balance", out var wb) &&
                    wb.ValueKind != JsonValueKind.Null)
                {
                    if (wb.TryGetProperty("colorTemperature", out var ct))
                        clip.WhiteBalance = ct.GetInt32();
                }
            }
        }
    }
    
    private static void ParseMxfGenericData(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("nativeIdentificationList", out var idList) &&
            idList.GetArrayLength() > 0)
        {
            var firstId = idList[0];
            if (firstId.TryGetProperty("productName", out var productName))
            {
                clip.CameraModel = productName.GetString();
            }
        }
    }
    
    private static void ParseLensDevice(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("lensModel", out var lensModel))
        {
            clip.Lens = lensModel.GetString();
        }
    }
    
    private static void ParseSlateInfo(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("reelName", out var reelName))
            clip.ReelName = reelName.GetString();
        
        // Could also get: scene, take, production, director, etc.
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
            if (storedSize.TryGetProperty("width", out var w))
                clip.Width = w.GetInt32();
            if (storedSize.TryGetProperty("height", out var h))
                clip.Height = h.GetInt32();
        }
    }
    
    private static void ParseProjectRate(JsonElement payload, CameraClip clip)
    {
        if (payload.TryGetProperty("timebase", out var timebase))
        {
            var tbStr = timebase.GetString() ?? "";
            var parts = tbStr.Split('/');
            if (parts.Length == 2 && 
                double.TryParse(parts[0], out var num) && 
                double.TryParse(parts[1], out var den) && 
                den != 0 && clip.FrameRate == 0)
            {
                clip.FrameRate = num / den;
            }
        }
    }
}
