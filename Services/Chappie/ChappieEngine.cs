using LunaApp.Models;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Chappie Engine - Intelligent clip processing orchestrator.
/// Tries each extractor in order until one successfully extracts metadata.
/// Priority: Sony sidecar → ARRI ART CLI → LibVLC (fallback)
/// </summary>
public sealed class ChappieEngine : IDisposable
{
    private readonly List<IClipProcessor> _processors = [];
    private readonly LibVlcClipProcessor _libVlcFallback = new();
    private bool _disposed;
    
    public ChappieEngine()
    {
        // Register processors in priority order (highest first)
        // Sony first - sidecar check is definitive and fast
        RegisterProcessor(new SonyClipProcessor());
        // ARRI second - uses ART CLI
        RegisterProcessor(new ArriClipProcessor());
        
        Log.Information("Chappie engine initialized with {Count} processors + LibVLC fallback", _processors.Count);
    }
    
    /// <summary>
    /// Register a clip processor
    /// </summary>
    public void RegisterProcessor(IClipProcessor processor)
    {
        _processors.Add(processor);
        // Sort by priority descending
        _processors.Sort((a, b) => b.Priority.CompareTo(a.Priority));
    }
    
    /// <summary>
    /// Detect manufacturer from file path and content
    /// </summary>
    public static CameraManufacturer DetectManufacturer(string filePath)
    {
        var fileName = Path.GetFileName(filePath).ToUpperInvariant();
        var ext = Path.GetExtension(filePath).ToUpperInvariant();
        var directory = Path.GetDirectoryName(filePath)?.ToUpperInvariant() ?? "";
        
        // ARRI patterns
        if (fileName.Contains("_ARRI") || 
            directory.Contains("ARRI") ||
            fileName.StartsWith("A0") || fileName.StartsWith("B0") || fileName.StartsWith("C0") ||
            ArriClipProcessor.HasArriSidecar(filePath))
        {
            return CameraManufacturer.Arri;
        }
        
        // Sony patterns
        if (ext == ".MXF" && SonyClipProcessor.HasSonySidecar(filePath))
        {
            return CameraManufacturer.Sony;
        }
        
        // RED patterns
        if (ext == ".R3D" || directory.Contains("RED"))
        {
            return CameraManufacturer.Red;
        }
        
        // Blackmagic patterns
        if (ext == ".BRAW" || directory.Contains("BRAW") || directory.Contains("BLACKMAGIC"))
        {
            return CameraManufacturer.Blackmagic;
        }
        
        // Canon patterns
        if (directory.Contains("CANON") || fileName.StartsWith("MVI_") || fileName.StartsWith("CLIP"))
        {
            return CameraManufacturer.Canon;
        }
        
        // Panasonic patterns
        if (directory.Contains("AVCHD") || directory.Contains("PRIVATE") && directory.Contains("PANA"))
        {
            return CameraManufacturer.Panasonic;
        }
        
        // DJI patterns
        if (fileName.StartsWith("DJI_") || directory.Contains("DJI"))
        {
            return CameraManufacturer.DJI;
        }
        
        return CameraManufacturer.Unknown;
    }
    
    /// <summary>
    /// Process a single clip - try each extractor until one succeeds
    /// </summary>
    public async Task<CameraClip> ProcessClipAsync(
        string filePath,
        bool extractThumbnails = true,
        int thumbnailCount = 3,
        int thumbnailWidth = 480,
        CancellationToken cancellationToken = default)
    {
        var fileName = Path.GetFileName(filePath);
        CameraClip? clip = null;
        IClipProcessor? successfulProcessor = null;
        
        // Try each processor in priority order
        foreach (var processor in _processors)
        {
            try
            {
                // Quick check - does processor think it can handle this?
                if (!processor.CanProcess(filePath))
                    continue;
                
                Log.Debug("Trying {Processor} for {FileName}", processor.GetType().Name, fileName);
                
                var result = await processor.ExtractMetadataAsync(filePath, cancellationToken);
                
                // Consider success if we got meaningful metadata (camera model or duration)
                if (result.ProcessingState == ClipProcessingState.Completed &&
                    (!string.IsNullOrEmpty(result.CameraModel) || result.Duration > TimeSpan.Zero))
                {
                    clip = result;
                    successfulProcessor = processor;
                    Log.Information("{Processor} succeeded for {FileName}", processor.GetType().Name, fileName);
                    break;
                }
            }
            catch (Exception ex)
            {
                Log.Debug(ex, "{Processor} failed for {FileName}", processor.GetType().Name, fileName);
            }
        }
        
        // Fallback to LibVLC if no processor succeeded
        if (clip == null)
        {
            Log.Debug("Using LibVLC fallback for {FileName}", fileName);
            try
            {
                clip = await _libVlcFallback.ExtractMetadataAsync(filePath, cancellationToken);
                successfulProcessor = _libVlcFallback;
            }
            catch (Exception ex)
            {
                Log.Error(ex, "LibVLC fallback failed for {FileName}", fileName);
                return CreateFailedClip(filePath, ex.Message);
            }
        }
        
        // Fill missing metadata from LibVLC if needed
        if (clip.Duration == TimeSpan.Zero || clip.Width == 0)
        {
            await FillMissingMetadataAsync(clip, cancellationToken);
        }
        
        // Extract thumbnails
        if (extractThumbnails && clip.Duration > TimeSpan.Zero)
        {
            try
            {
                Log.Debug("Extracting thumbnails for {FileName} (duration: {Duration})", fileName, clip.Duration);
                
                // Use the successful processor's thumbnail extraction (ARRI uses ART CLI)
                // Fall back to LibVLC for other formats
                if (successfulProcessor != null && successfulProcessor != _libVlcFallback)
                {
                    clip.Thumbnails = await successfulProcessor.ExtractThumbnailsAsync(
                        filePath, clip.Duration, thumbnailCount, thumbnailWidth, cancellationToken);
                }
                
                // If processor didn't extract thumbnails, try LibVLC as fallback
                if (clip.Thumbnails == null || clip.Thumbnails.Count == 0)
                {
                    clip.Thumbnails = await _libVlcFallback.ExtractThumbnailsAsync(
                        filePath, clip.Duration, thumbnailCount, thumbnailWidth, cancellationToken);
                }
                
                Log.Debug("Extracted {Count} thumbnails for {FileName}", clip.Thumbnails.Count, fileName);
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Thumbnail extraction failed for {FileName}", fileName);
                clip.Thumbnails = [];
            }
        }
        else if (extractThumbnails)
        {
            Log.Warning("Skipping thumbnails for {FileName} - duration is zero", fileName);
            clip.Thumbnails = [];
        }
        
        clip.ProcessingState = ClipProcessingState.Completed;
        return clip;
    }
    
    private async Task FillMissingMetadataAsync(CameraClip clip, CancellationToken cancellationToken)
    {
        try
        {
            var vlcClip = await _libVlcFallback.ExtractMetadataAsync(clip.FilePath, cancellationToken);
            
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
            Log.Debug(ex, "Failed to fill missing metadata for {FileName}", clip.FileName);
        }
    }
    
    /// <summary>
    /// Process multiple clips with progress reporting
    /// </summary>
    public async Task<List<CameraClip>> ProcessClipsAsync(
        IEnumerable<string> filePaths,
        bool extractThumbnails = true,
        int thumbnailCount = 3,
        int thumbnailWidth = 480,
        IProgress<(int current, int total, string file)>? progress = null,
        CancellationToken cancellationToken = default)
    {
        var paths = filePaths.ToList();
        var clips = new List<CameraClip>(paths.Count);
        
        for (int i = 0; i < paths.Count; i++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            
            var path = paths[i];
            progress?.Report((i + 1, paths.Count, Path.GetFileName(path)));
            
            var clip = await ProcessClipAsync(path, extractThumbnails, thumbnailCount, thumbnailWidth, cancellationToken);
            clips.Add(clip);
        }
        
        return clips;
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
    
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        
        _libVlcFallback.Dispose();
        
        foreach (var processor in _processors)
        {
            if (processor is IDisposable disposable)
            {
                disposable.Dispose();
            }
        }
        
        _processors.Clear();
    }
}
