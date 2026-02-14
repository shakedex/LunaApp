using System.Runtime.InteropServices;
using LibVLCSharp.Shared;
using LunaApp.Models;
using Serilog;
using SkiaSharp;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Generic clip processor using LibVLC for metadata and thumbnail extraction.
/// Serves as fallback for any format not handled by manufacturer-specific processors.
/// </summary>
public sealed class LibVlcClipProcessor : IClipProcessor, IDisposable
{
    private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mov", ".mp4", ".mxf", ".avi", ".mkv", ".m4v",
        ".mts", ".m2ts", ".3gp", ".webm", ".wmv", ".flv",
        ".r3d", ".braw"
    };
    
    // Common FourCC codec codes mapped to human-readable names
    private static readonly Dictionary<uint, string> CommonCodecs = new()
    {
        // ProRes family
        [0x61706368] = "ProRes 422 HQ",    // apch
        [0x6170636E] = "ProRes 422",       // apcn
        [0x61706373] = "ProRes 422 LT",    // apcs
        [0x6170636F] = "ProRes 422 Proxy", // apco  
        [0x61703468] = "ProRes 4444",      // ap4h
        [0x61703478] = "ProRes 4444 XQ",   // ap4x
        
        // H.264/AVC
        [0x61766331] = "H.264",            // avc1
        [0x48323634] = "H.264",            // H264
        [0x68323634] = "H.264",            // h264
        [0x78323634] = "H.264",            // x264
        
        // H.265/HEVC
        [0x68657631] = "H.265",            // hev1
        [0x68766331] = "H.265",            // hvc1
        [0x48455643] = "H.265",            // HEVC
        
        // DNxHD/HR
        [0x41564448] = "DNxHD",            // AVDH
        [0x41564452] = "DNxHR",            // AVDR
        
        // XAVC
        [0x58415643] = "XAVC",             // XAVC
        
        // ARRIRAW
        [0x41524949] = "ARRIRAW",          // ARII
        
        // Common others
        [0x6D6A7067] = "MJPEG",            // mjpg
        [0x4D4A5047] = "MJPEG",            // MJPG
    };
    
    private static bool _coreInitialized;
    private static readonly object _initLock = new();
    private LibVLC? _libVLC;
    private bool _disposed;
    
    public int Priority => 0; // Lowest priority - fallback
    
    public LibVlcClipProcessor()
    {
        InitializeCore();
        _libVLC = new LibVLC(
            enableDebugLogs: false,
            "--no-audio",
            "--no-video-title-show",
            "--no-stats",
            "--no-snapshot-preview",
            "--vout=none"
        );
    }
    
    private static void InitializeCore()
    {
        lock (_initLock)
        {
            if (_coreInitialized) return;
            Core.Initialize();
            _coreInitialized = true;
        }
    }
    
    public bool CanProcess(string filePath)
    {
        var ext = Path.GetExtension(filePath);
        return SupportedExtensions.Contains(ext);
    }
    
    public async Task<CameraClip> ExtractMetadataAsync(string filePath, CancellationToken cancellationToken = default)
    {
        var fileInfo = new FileInfo(filePath);
        var clip = new CameraClip
        {
            FilePath = filePath,
            FileName = fileInfo.Name,
            FileSizeBytes = fileInfo.Length
        };
        
        if (_libVLC == null)
        {
            clip.ProcessingState = ClipProcessingState.Failed;
            clip.ProcessingError = "LibVLC not initialized";
            return clip;
        }
        
        try
        {
            using var media = new Media(_libVLC, new Uri(filePath));
            
            var parseResult = await media.Parse(MediaParseOptions.ParseLocal | MediaParseOptions.FetchLocal);
            
            if (parseResult != MediaParsedStatus.Done)
            {
                Log.Warning("Failed to parse media: {FilePath}, Status: {Status}", filePath, parseResult);
                clip.ProcessingState = ClipProcessingState.Failed;
                clip.ProcessingError = $"Parse failed: {parseResult}";
                return clip;
            }
            
            // Duration
            clip.Duration = TimeSpan.FromMilliseconds(media.Duration);
            
            // Video track info
            var videoTrack = media.Tracks.FirstOrDefault(t => t.TrackType == TrackType.Video);
            if (videoTrack.TrackType == TrackType.Video)
            {
                var data = videoTrack.Data.Video;
                clip.Width = (int)data.Width;
                clip.Height = (int)data.Height;
                clip.FrameRate = data.FrameRateNum / (double)Math.Max(1, data.FrameRateDen);
                clip.Codec = DecodeCodec(videoTrack.Codec, videoTrack.Description);
            }
            
            // Standard metadata
            ExtractMediaMeta(media, clip);
            
            clip.ProcessingState = ClipProcessingState.Completed;
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to extract metadata from {FilePath}", filePath);
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
        if (_libVLC == null || _disposed)
            return [];
        
        return await Task.Run(() =>
            ExtractThumbnailsInternal(filePath, duration, count, width, cancellationToken),
            cancellationToken);
    }
    
    private List<ThumbnailFrame> ExtractThumbnailsInternal(
        string filePath,
        TimeSpan duration,
        int count,
        int targetWidth,
        CancellationToken cancellationToken)
    {
        var thumbnails = new List<ThumbnailFrame>();
        var fileName = Path.GetFileName(filePath);
        
        if (_libVLC == null || count <= 0 || duration <= TimeSpan.Zero)
        {
            Log.Debug("Thumbnail extraction skipped for {FileName}: libVLC={LibVLC}, count={Count}, duration={Duration}", 
                fileName, _libVLC != null, count, duration);
            return thumbnails;
        }
        
        // For complex codecs that don't support seeking well, just capture the first N frames
        // that successfully decode, without seeking
        Log.Debug("Extracting {Count} thumbnails from first frames of {FileName}", count, fileName);
        
        var extractedThumbnails = ExtractFirstFrames(filePath, count, targetWidth, cancellationToken);
        thumbnails.AddRange(extractedThumbnails);
        
        return thumbnails;
    }
    
    /// <summary>
    /// Extract first N frames without seeking - more reliable for complex codecs like ARRIRAW
    /// </summary>
    private List<ThumbnailFrame> ExtractFirstFrames(
        string filePath,
        int count,
        int targetWidth,
        CancellationToken cancellationToken)
    {
        var thumbnails = new List<ThumbnailFrame>();
        var fileName = Path.GetFileName(filePath);
        
        if (_libVLC == null) return thumbnails;
        
        try
        {
            using var media = new Media(_libVLC, new Uri(filePath));
            var parseResult = media.Parse(MediaParseOptions.ParseLocal).GetAwaiter().GetResult();
            
            if (parseResult != MediaParsedStatus.Done)
            {
                Log.Debug("Media parse failed for {FileName}: {Status}", fileName, parseResult);
                return thumbnails;
            }
            
            var videoTrack = media.Tracks.FirstOrDefault(t => t.TrackType == TrackType.Video);
            if (videoTrack.TrackType != TrackType.Video)
            {
                Log.Debug("No video track found for {FileName}", fileName);
                return thumbnails;
            }
            
            var videoData = videoTrack.Data.Video;
            var sourceWidth = (int)videoData.Width;
            var sourceHeight = (int)videoData.Height;
            
            if (sourceWidth == 0 || sourceHeight == 0)
            {
                Log.Debug("Invalid video dimensions for {FileName}", fileName);
                return thumbnails;
            }
            
            var aspectRatio = sourceWidth / (double)sourceHeight;
            var targetHeight = (int)(targetWidth / aspectRatio);
            targetWidth = (targetWidth / 2) * 2;
            targetHeight = (targetHeight / 2) * 2;
            
            if (targetWidth <= 0 || targetHeight <= 0)
                return thumbnails;
            
            var pitch = targetWidth * 4;
            var bufferSize = pitch * targetHeight;
            var frameBuffer = new byte[bufferSize];
            var capturedFrames = new List<(byte[] data, long timeMs)>();
            var frameCount = 0;
            var lockObj = new object();
            GCHandle bufferHandle = default;
            
            using var player = new MediaPlayer(media);
            
            try
            {
                bufferHandle = GCHandle.Alloc(frameBuffer, GCHandleType.Pinned);
                var bufferPtr = bufferHandle.AddrOfPinnedObject();
                
                player.SetVideoFormat("BGRA", (uint)targetWidth, (uint)targetHeight, (uint)pitch);
                player.SetVideoCallbacks(
                    (opaque, planes) =>
                    {
                        Marshal.WriteIntPtr(planes, bufferPtr);
                        return IntPtr.Zero;
                    },
                    (opaque, picture, planes) =>
                    {
                        lock (lockObj)
                        {
                            // Capture frames at intervals (every ~12 frames = ~0.5s at 24fps)
                            frameCount++;
                            if (capturedFrames.Count < count && (frameCount == 1 || frameCount % 12 == 0))
                            {
                                var copy = new byte[bufferSize];
                                Buffer.BlockCopy(frameBuffer, 0, copy, 0, bufferSize);
                                capturedFrames.Add((copy, player.Time));
                            }
                        }
                    },
                    null
                );
                
                player.Play();
                
                // Wait for enough frames or timeout
                var startTime = DateTime.UtcNow;
                var timeout = TimeSpan.FromSeconds(30);
                
                while (capturedFrames.Count < count && 
                       DateTime.UtcNow - startTime < timeout &&
                       !cancellationToken.IsCancellationRequested)
                {
                    Thread.Sleep(100);
                }
                
                player.Stop();
                
                Log.Debug("Captured {Count} frames from {FileName}", capturedFrames.Count, fileName);
                
                // Convert captured frames to thumbnails
                foreach (var (data, timeMs) in capturedFrames)
                {
                    var thumbnail = ConvertBufferToThumbnail(data, targetWidth, targetHeight, TimeSpan.FromMilliseconds(timeMs));
                    if (thumbnail != null)
                    {
                        thumbnails.Add(thumbnail);
                    }
                }
            }
            finally
            {
                if (bufferHandle.IsAllocated)
                    bufferHandle.Free();
            }
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "Failed to extract first frames from {FileName}", fileName);
        }
        
        return thumbnails;
    }
    
    private ThumbnailFrame? ExtractFrameAtPosition(
        string filePath,
        double position,
        TimeSpan duration,
        int targetWidth)
    {
        if (_libVLC == null) return null;
        var fileName = Path.GetFileName(filePath);
        
        try
        {
            using var media = new Media(_libVLC, new Uri(filePath));
            var parseResult = media.Parse(MediaParseOptions.ParseLocal).GetAwaiter().GetResult();
            
            if (parseResult != MediaParsedStatus.Done)
            {
                Log.Debug("Media parse failed for {FileName}: {Status}", fileName, parseResult);
                return null;
            }
            
            var videoTrack = media.Tracks.FirstOrDefault(t => t.TrackType == TrackType.Video);
            if (videoTrack.TrackType != TrackType.Video)
            {
                Log.Debug("No video track found for {FileName}", fileName);
                return null;
            }
            
            var videoData = videoTrack.Data.Video;
            var sourceWidth = (int)videoData.Width;
            var sourceHeight = (int)videoData.Height;
            
            if (sourceWidth == 0 || sourceHeight == 0)
            {
                Log.Debug("Invalid video dimensions for {FileName}: {Width}x{Height}", fileName, sourceWidth, sourceHeight);
                return null;
            }
            
            var aspectRatio = sourceWidth / (double)sourceHeight;
            var targetHeight = (int)(targetWidth / aspectRatio);
            
            targetWidth = (targetWidth / 2) * 2;
            targetHeight = (targetHeight / 2) * 2;
            
            if (targetWidth <= 0 || targetHeight <= 0)
                return null;
            
            var pitch = targetWidth * 4;
            var bufferSize = pitch * targetHeight;
            var frameBuffer = new byte[bufferSize];
            var frameReady = new ManualResetEventSlim(false);
            var frameCaptured = false;
            GCHandle bufferHandle = default;
            
            using var player = new MediaPlayer(media);
            
            try
            {
                bufferHandle = GCHandle.Alloc(frameBuffer, GCHandleType.Pinned);
                var bufferPtr = bufferHandle.AddrOfPinnedObject();
                
                player.SetVideoFormat("BGRA", (uint)targetWidth, (uint)targetHeight, (uint)pitch);
                player.SetVideoCallbacks(
                    (opaque, planes) =>
                    {
                        Marshal.WriteIntPtr(planes, bufferPtr);
                        return IntPtr.Zero;
                    },
                    (opaque, picture, planes) =>
                    {
                        if (!frameCaptured)
                        {
                            frameCaptured = true;
                            frameReady.Set();
                        }
                    },
                    null
                );
                
                // For seeking, set position BEFORE playing (works better for some codecs)
                var targetTimeMs = (long)(position * duration.TotalMilliseconds);
                
                player.Play();
                
                // Wait for playback to start
                if (!SpinWait.SpinUntil(() => player.IsPlaying, TimeSpan.FromSeconds(10)))
                {
                    Log.Debug("Playback failed to start for {FileName}", fileName);
                    return null;
                }
                
                // Seek using position (0.0-1.0) instead of time - more reliable for some formats
                player.Position = (float)position;
                
                // Wait for frame with extended timeout for complex codecs like ARRIRAW
                if (!frameReady.Wait(TimeSpan.FromSeconds(15)))
                {
                    Log.Debug("Frame capture timed out for {FileName} at position {Position}", fileName, position);
                    return null;
                }
                
                Thread.Sleep(50);
                player.Stop();
                
                var imageBase64 = ConvertBufferToWebP(frameBuffer, targetWidth, targetHeight, pitch);
                
                if (imageBase64 == null)
                    return null;
                
                var frameTime = TimeSpan.FromMilliseconds(targetTimeMs);
                
                return new ThumbnailFrame
                {
                    Position = position,
                    Timecode = frameTime.ToString(@"hh\:mm\:ss\:ff"),
                    FrameNumber = (int)(position * duration.TotalSeconds * 24),
                    ImageBase64 = imageBase64,
                    Width = targetWidth,
                    Height = targetHeight
                };
            }
            finally
            {
                if (bufferHandle.IsAllocated)
                    bufferHandle.Free();
            }
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "Error extracting frame at position {Position}", position);
            return null;
        }
    }
    
    private void ExtractMediaMeta(Media media, CameraClip clip)
    {
        var date = media.Meta(MetadataType.Date);
        if (!string.IsNullOrEmpty(date) && DateTime.TryParse(date, out var recordedDate))
        {
            clip.RecordedDate = recordedDate;
        }
        
        var encodedBy = media.Meta(MetadataType.EncodedBy);
        if (!string.IsNullOrEmpty(encodedBy))
        {
            clip.CameraManufacturer = encodedBy;
        }
    }
    
    private static string? ConvertBufferToWebP(byte[] buffer, int width, int height, int stride)
    {
        try
        {
            using var bitmap = new SKBitmap(width, height, SKColorType.Bgra8888, SKAlphaType.Premul);
            
            var dstSpan = bitmap.GetPixelSpan();
            var bitmapStride = bitmap.RowBytes;
            
            for (int y = 0; y < height; y++)
            {
                var srcOffset = y * stride;
                var dstOffset = y * bitmapStride;
                var copyBytes = Math.Min(stride, bitmapStride);
                
                if (srcOffset + copyBytes <= buffer.Length && dstOffset + copyBytes <= dstSpan.Length)
                {
                    buffer.AsSpan(srcOffset, copyBytes).CopyTo(dstSpan.Slice(dstOffset, copyBytes));
                }
            }
            
            using var image = SKImage.FromBitmap(bitmap);
            using var data = image.Encode(SKEncodedImageFormat.Webp, 80);
            
            return Convert.ToBase64String(data.ToArray());
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to convert buffer to WebP");
            return null;
        }
    }
    
    /// <summary>
    /// Convert frame buffer to ThumbnailFrame
    /// </summary>
    private ThumbnailFrame? ConvertBufferToThumbnail(byte[] buffer, int width, int height, TimeSpan timestamp)
    {
        var pitch = width * 4;
        var imageBase64 = ConvertBufferToWebP(buffer, width, height, pitch);
        
        if (imageBase64 == null)
            return null;
        
        return new ThumbnailFrame
        {
            Position = timestamp.TotalSeconds,
            Timecode = timestamp.ToString(@"hh\:mm\:ss\:ff"),
            FrameNumber = (int)(timestamp.TotalSeconds * 24),
            ImageBase64 = imageBase64,
            Width = width,
            Height = height
        };
    }
    
    /// <summary>
    /// Decode codec ID to human-readable name
    /// </summary>
    private static string DecodeCodec(uint codecId, string? description)
    {
        // Use description if it looks reasonable
        if (!string.IsNullOrEmpty(description) && 
            !description.All(char.IsDigit) &&
            description.Length > 2)
        {
            return description;
        }
        
        // Check known codecs
        if (CommonCodecs.TryGetValue(codecId, out var known))
        {
            return known;
        }
        
        // Try to decode as FourCC (4 ASCII characters)
        if (codecId > 0)
        {
            try
            {
                // FourCC can be stored in different byte orders
                // Try both big-endian and little-endian
                var bytes = BitConverter.GetBytes(codecId);
                
                // Little-endian (most common in Windows/Intel)
                var fourccLE = new string(bytes.Select(b => b >= 32 && b < 127 ? (char)b : '\0').ToArray()).Trim('\0');
                
                // Big-endian 
                var bytesBE = bytes.Reverse().ToArray();
                var fourccBE = new string(bytesBE.Select(b => b >= 32 && b < 127 ? (char)b : '\0').ToArray()).Trim('\0');
                
                // Map common FourCC strings
                var fourcc = fourccLE.Length >= fourccBE.Length ? fourccLE : fourccBE;
                
                return fourcc.ToUpperInvariant() switch
                {
                    "APCN" => "ProRes 422",
                    "APCH" => "ProRes 422 HQ",
                    "APCS" => "ProRes 422 LT",
                    "APCO" => "ProRes 422 Proxy",
                    "AP4H" => "ProRes 4444",
                    "AP4X" => "ProRes 4444 XQ",
                    "AVC1" => "H.264",
                    "H264" => "H.264",
                    "HEV1" => "H.265",
                    "HVC1" => "H.265",
                    "HEVC" => "H.265",
                    _ when fourcc.Length >= 3 => fourcc,
                    _ => $"Codec {codecId}"
                };
            }
            catch { }
        }
        
        // Last resort: just format the number
        return $"Codec {codecId}";
    }
    
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        
        _libVLC?.Dispose();
        _libVLC = null;
    }
}
