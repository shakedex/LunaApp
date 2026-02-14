using System.Runtime.InteropServices;
using FFmpeg.AutoGen;
using LunaApp.Models;
using Serilog;
using SkiaSharp;

namespace LunaApp.Services.Chappie;

/// <summary>
/// FFmpeg-based thumbnail extraction service using FFmpeg.AutoGen native bindings.
/// Extracts frames at specific positions from any codec FFmpeg supports.
/// </summary>
public sealed unsafe class FfmpegThumbnailService : IDisposable
{
    private static bool _initialized;
    private static readonly object _initLock = new();
    private bool _disposed;
    
    public bool IsAvailable { get; private set; }
    
    public FfmpegThumbnailService()
    {
        try
        {
            InitializeFfmpeg();
            IsAvailable = true;
            
            // Test that FFmpeg is working
            var version = ffmpeg.av_version_info();
            Log.Information("FFmpeg initialized: {Version}", version);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "FFmpeg initialization failed - thumbnail extraction will use fallback");
            IsAvailable = false;
        }
    }
    
    private static void InitializeFfmpeg()
    {
        lock (_initLock)
        {
            if (_initialized) return;
            
            // Find FFmpeg libraries
            var librariesPath = FindFfmpegLibraries();
            
            if (string.IsNullOrEmpty(librariesPath))
            {
                throw new InvalidOperationException("FFmpeg libraries not found");
            }
            
            Log.Debug("FFmpeg libraries path: {Path}", librariesPath);
            
            // On Windows, add the path to PATH env var for DLL dependency resolution
            if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            {
                var currentPath = Environment.GetEnvironmentVariable("PATH") ?? "";
                if (!currentPath.Contains(librariesPath))
                {
                    Environment.SetEnvironmentVariable("PATH", librariesPath + ";" + currentPath);
                    Log.Debug("Added FFmpeg path to PATH environment variable");
                }
            }
            
            // Set root path for FFmpeg library resolution (used by main ffmpeg class)
            ffmpeg.RootPath = librariesPath;
            
            // Initialize the dynamically loaded bindings
            DynamicallyLoadedBindings.Initialize();
            
            _initialized = true;
        }
    }
    
    private static string? FindFfmpegLibraries()
    {
        var appDir = AppContext.BaseDirectory;
        
        // Check bundled locations first
        var bundledPaths = new[]
        {
            Path.Combine(appDir, "tools", "ffmpeg", "win-x64"),
            Path.Combine(appDir, "tools", "ffmpeg", "osx-arm64"),
            Path.Combine(appDir, "tools", "ffmpeg", "osx-x64"),
            Path.Combine(appDir, "tools", "ffmpeg"),
        };
        
        foreach (var path in bundledPaths)
        {
            if (Directory.Exists(path) && HasFfmpegLibraries(path))
            {
                return path;
            }
        }
        
        // macOS Homebrew locations
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
        {
            var brewPaths = new[]
            {
                "/opt/homebrew/lib",      // Apple Silicon
                "/usr/local/lib"           // Intel
            };
            
            foreach (var path in brewPaths)
            {
                if (Directory.Exists(path) && HasFfmpegLibraries(path))
                {
                    return path;
                }
            }
        }
        
        // Linux system paths
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Linux))
        {
            var linuxPaths = new[]
            {
                "/usr/lib/x86_64-linux-gnu",
                "/usr/lib64",
                "/usr/lib"
            };
            
            foreach (var path in linuxPaths)
            {
                if (Directory.Exists(path))
                    return path;
            }
        }
        
        return null; // Will try system PATH
    }
    
    private static bool HasFfmpegLibraries(string path)
    {
        var pattern = RuntimeInformation.IsOSPlatform(OSPlatform.Windows) 
            ? "avcodec*.dll" 
            : RuntimeInformation.IsOSPlatform(OSPlatform.OSX) 
                ? "libavcodec*.dylib" 
                : "libavcodec.so*";
        
        try
        {
            return Directory.GetFiles(path, pattern).Length > 0;
        }
        catch
        {
            return false;
        }
    }
    
    /// <summary>
    /// Extract thumbnails at specified positions in the clip
    /// </summary>
    public Task<List<ThumbnailFrame>> ExtractThumbnailsAsync(
        string filePath,
        TimeSpan duration,
        int count = 3,
        int width = 480,
        CancellationToken cancellationToken = default)
    {
        if (!IsAvailable || duration <= TimeSpan.Zero)
            return Task.FromResult<List<ThumbnailFrame>>([]);
        
        return Task.Run(() => 
            ExtractThumbnailsInternal(filePath, duration, count, width, cancellationToken),
            cancellationToken);
    }
    
    private unsafe List<ThumbnailFrame> ExtractThumbnailsInternal(
        string filePath,
        TimeSpan duration,
        int count,
        int targetWidth,
        CancellationToken cancellationToken)
    {
        var thumbnails = new List<ThumbnailFrame>();
        var fileName = Path.GetFileName(filePath);
        
        // Calculate positions: 10%, 50%, 90% of duration
        var positions = count == 1
            ? new[] { 0.5 }
            : Enumerable.Range(0, count).Select(i => 0.1 + (0.8 * i / (count - 1))).ToArray();
        
        AVFormatContext* pFormatContext = null;
        AVCodecContext* pCodecContext = null;
        AVFrame* pFrame = null;
        AVFrame* pRgbFrame = null;
        AVPacket* pPacket = null;
        SwsContext* pSwsContext = null;
        byte* rgbBuffer = null;
        
        try
        {
            // Open input file
            pFormatContext = ffmpeg.avformat_alloc_context();
            var pFormatContextLocal = pFormatContext;
            
            if (ffmpeg.avformat_open_input(&pFormatContextLocal, filePath, null, null) != 0)
            {
                Log.Debug("FFmpeg: Failed to open {FileName}", fileName);
                return thumbnails;
            }
            pFormatContext = pFormatContextLocal;
            
            // Find stream info
            if (ffmpeg.avformat_find_stream_info(pFormatContext, null) < 0)
            {
                Log.Debug("FFmpeg: Failed to find stream info for {FileName}", fileName);
                return thumbnails;
            }
            
            // Find video stream
            AVCodec* pCodec = null;
            var videoStreamIndex = ffmpeg.av_find_best_stream(
                pFormatContext, AVMediaType.AVMEDIA_TYPE_VIDEO, -1, -1, &pCodec, 0);
            
            if (videoStreamIndex < 0 || pCodec == null)
            {
                Log.Debug("FFmpeg: No video stream found in {FileName}", fileName);
                return thumbnails;
            }
            
            var pStream = pFormatContext->streams[videoStreamIndex];
            
            // Create codec context
            pCodecContext = ffmpeg.avcodec_alloc_context3(pCodec);
            if (ffmpeg.avcodec_parameters_to_context(pCodecContext, pStream->codecpar) < 0)
            {
                Log.Debug("FFmpeg: Failed to copy codec params for {FileName}", fileName);
                return thumbnails;
            }
            
            if (ffmpeg.avcodec_open2(pCodecContext, pCodec, null) < 0)
            {
                Log.Debug("FFmpeg: Failed to open codec for {FileName}", fileName);
                return thumbnails;
            }
            
            // Calculate output dimensions
            var sourceWidth = pCodecContext->width;
            var sourceHeight = pCodecContext->height;
            var aspectRatio = sourceWidth / (double)sourceHeight;
            var targetHeight = (int)(targetWidth / aspectRatio);
            targetWidth = (targetWidth / 2) * 2;  // Ensure even
            targetHeight = (targetHeight / 2) * 2;
            
            // Allocate frames
            pFrame = ffmpeg.av_frame_alloc();
            pRgbFrame = ffmpeg.av_frame_alloc();
            pPacket = ffmpeg.av_packet_alloc();
            
            // Allocate RGB buffer
            var bufferSize = ffmpeg.av_image_get_buffer_size(
                AVPixelFormat.AV_PIX_FMT_BGRA, targetWidth, targetHeight, 1);
            rgbBuffer = (byte*)ffmpeg.av_malloc((ulong)bufferSize);
            
            var dstData = new byte_ptrArray4();
            var dstLinesize = new int_array4();
            ffmpeg.av_image_fill_arrays(ref dstData, ref dstLinesize, rgbBuffer,
                AVPixelFormat.AV_PIX_FMT_BGRA, targetWidth, targetHeight, 1);
            
            // Create scaler (SWS_BILINEAR = 2)
            pSwsContext = ffmpeg.sws_getContext(
                sourceWidth, sourceHeight, pCodecContext->pix_fmt,
                targetWidth, targetHeight, AVPixelFormat.AV_PIX_FMT_BGRA,
                2, null, null, null);
            
            if (pSwsContext == null)
            {
                Log.Debug("FFmpeg: Failed to create scaler for {FileName}", fileName);
                return thumbnails;
            }
            
            // Extract frame at each position
            foreach (var position in positions)
            {
                if (cancellationToken.IsCancellationRequested)
                    break;
                
                var seekTimeSeconds = position * duration.TotalSeconds;
                var timestamp = (long)(seekTimeSeconds * pStream->time_base.den / pStream->time_base.num);
                
                // Seek to position
                ffmpeg.av_seek_frame(pFormatContext, videoStreamIndex, timestamp, ffmpeg.AVSEEK_FLAG_BACKWARD);
                ffmpeg.avcodec_flush_buffers(pCodecContext);
                
                // Decode until we get a valid frame
                var frameDecoded = false;
                var attempts = 0;
                
                while (!frameDecoded && attempts < 100)
                {
                    attempts++;
                    ffmpeg.av_packet_unref(pPacket);
                    
                    var readResult = ffmpeg.av_read_frame(pFormatContext, pPacket);
                    if (readResult < 0)
                        break;
                    
                    if (pPacket->stream_index != videoStreamIndex)
                        continue;
                    
                    if (ffmpeg.avcodec_send_packet(pCodecContext, pPacket) < 0)
                        continue;
                    
                    var receiveResult = ffmpeg.avcodec_receive_frame(pCodecContext, pFrame);
                    if (receiveResult == 0)
                    {
                        frameDecoded = true;
                    }
                }
                
                if (!frameDecoded)
                {
                    Log.Debug("FFmpeg: Failed to decode frame at {Position} for {FileName}", position, fileName);
                    continue;
                }
                
                // Scale frame to RGB
                ffmpeg.sws_scale(pSwsContext,
                    pFrame->data, pFrame->linesize,
                    0, pFrame->height,
                    dstData, dstLinesize);
                
                // Convert to thumbnail
                var thumbnail = ConvertToThumbnail(
                    rgbBuffer, targetWidth, targetHeight, dstLinesize[0],
                    position, TimeSpan.FromSeconds(seekTimeSeconds));
                
                if (thumbnail != null)
                {
                    thumbnails.Add(thumbnail);
                }
            }
            
            Log.Debug("FFmpeg extracted {Count} thumbnails from {FileName}", thumbnails.Count, fileName);
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "FFmpeg extraction failed for {FileName}", fileName);
        }
        finally
        {
            // Cleanup
            if (rgbBuffer != null) ffmpeg.av_free(rgbBuffer);
            if (pSwsContext != null) ffmpeg.sws_freeContext(pSwsContext);
            if (pPacket != null) { var p = pPacket; ffmpeg.av_packet_free(&p); }
            if (pRgbFrame != null) { var f = pRgbFrame; ffmpeg.av_frame_free(&f); }
            if (pFrame != null) { var f = pFrame; ffmpeg.av_frame_free(&f); }
            if (pCodecContext != null) { var c = pCodecContext; ffmpeg.avcodec_free_context(&c); }
            if (pFormatContext != null) { var f = pFormatContext; ffmpeg.avformat_close_input(&f); }
        }
        
        return thumbnails;
    }
    
    private static unsafe ThumbnailFrame? ConvertToThumbnail(
        byte* buffer, int width, int height, int stride,
        double position, TimeSpan timestamp)
    {
        try
        {
            var imageInfo = new SKImageInfo(width, height, SKColorType.Bgra8888, SKAlphaType.Opaque);
            
            using var bitmap = new SKBitmap();
            bitmap.InstallPixels(imageInfo, (IntPtr)buffer, stride);
            
            using var image = SKImage.FromBitmap(bitmap);
            using var data = image.Encode(SKEncodedImageFormat.Webp, 80);
            
            return new ThumbnailFrame
            {
                Position = position,
                Timecode = timestamp.ToString(@"hh\:mm\:ss\:ff"),
                FrameNumber = (int)(timestamp.TotalSeconds * 24),
                ImageBase64 = Convert.ToBase64String(data.ToArray()),
                Width = width,
                Height = height
            };
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "Failed to convert frame to thumbnail");
            return null;
        }
    }
    
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        // FFmpeg cleanup is handled per-operation
    }
}
