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
public sealed unsafe class FfmpegThumbnailService : IThumbnailGenerator, IDisposable
{
    private static bool _initialized;
    private static readonly object _initLock = new();
    private readonly MediaProcessingOptions _options;
    private bool _disposed;

    public bool IsAvailable { get; private set; }

    public FfmpegThumbnailService(MediaProcessingOptions options)
    {
        _options = options;
        try
        {
            EnsureInitialized();
            IsAvailable = true;

            // Test that FFmpeg is working
            var version = ffmpeg.av_version_info();
            Log.Information("FFmpeg initialized: {Version}", version);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "FFmpeg initialization failed — thumbnails will not be generated. " +
                            "Ensure the tools/ffmpeg folder is present and contains matching 7.x libraries.");
            IsAvailable = false;
        }
    }

    /// <summary>
    /// Locates and initializes the FFmpeg native libraries. Safe to call from
    /// any service that needs FFmpeg — idempotent under lock. Throws if the
    /// libraries can't be found.
    /// </summary>
    public static void EnsureInitialized()
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
    
    /// <summary>
    /// Decodes a little-endian FOURCC into a printable 4-char tag (e.g. 0x78347061 → "ap4x").
    /// Returns null when the tag has no printable bytes — the caller can then fall back
    /// to something more informative than a string of question marks.
    /// </summary>
    private static string? FourCcToString(uint fourCc)
    {
        Span<char> chars = stackalloc char[4];
        var printableCount = 0;
        for (int i = 0; i < 4; i++)
        {
            var b = (byte)(fourCc >> (i * 8));
            if (b >= 32 && b < 127)
            {
                chars[i] = (char)b;
                printableCount++;
            }
            else
            {
                chars[i] = '?';
            }
        }
        if (printableCount == 0) return null;
        return new string(chars).TrimEnd();
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
    
    // Cap per-position decode attempts. Enough headroom for MXF files whose
    // index lands us on a non-keyframe and we have to walk forward, but low
    // enough that truly undecodable packets fail fast instead of burning 300
    // iterations on every frame of every file. If we need more than this for
    // a real file, the seek strategy is wrong, not the cap.
    private const int MaxDecodeAttemptsPerPosition = 60;

    public int Priority => 0; // Universal first-line decoder.

    /// <summary>
    /// Extract thumbnails at specified positions in the clip. Returns a typed
    /// <see cref="ThumbnailResult"/> that carries the outcome reason when
    /// zero frames are produced — callers can distinguish NoDecoder (ARRIRAW,
    /// X-OCN, BRAW, REDCODE) from real failure modes.
    /// </summary>
    public Task<ThumbnailResult> GenerateAsync(
        string filePath,
        ThumbnailRequest request,
        CancellationToken cancellationToken)
    {
        if (!IsAvailable)
            return Task.FromResult(ThumbnailResult.NotAttempted("FFmpeg is not initialized"));

        if (request.Duration <= TimeSpan.Zero)
            return Task.FromResult(ThumbnailResult.NotAttempted("clip duration is zero"));

        return Task.Run(() =>
            ExtractThumbnailsInternal(filePath, request.Duration, request.Count, request.Width, cancellationToken),
            cancellationToken);
    }

    private unsafe ThumbnailResult ExtractThumbnailsInternal(
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
        AVDictionary* options = null;

        try
        {
            // Open input file with MXF-friendly options
            pFormatContext = ffmpeg.avformat_alloc_context();
            var pFormatContextLocal = pFormatContext;

            // Increase probe size for MXF files to properly parse index
            ffmpeg.av_dict_set(&options, "probesize", "100000000", 0); // 100MB
            ffmpeg.av_dict_set(&options, "analyzeduration", "100000000", 0); // 100 seconds

            if (ffmpeg.avformat_open_input(&pFormatContextLocal, filePath, null, &options) != 0)
            {
                Log.Debug("FFmpeg: Failed to open {FileName}", fileName);
                ffmpeg.av_dict_free(&options);
                return ThumbnailResult.ContainerOpenFailed("avformat_open_input rejected the file");
            }
            pFormatContext = pFormatContextLocal;
            ffmpeg.av_dict_free(&options);

            // Find stream info with extended analysis
            pFormatContext->max_analyze_duration = 100 * ffmpeg.AV_TIME_BASE; // 100 seconds
            if (ffmpeg.avformat_find_stream_info(pFormatContext, null) < 0)
            {
                Log.Debug("FFmpeg: Failed to find stream info for {FileName}", fileName);
                return ThumbnailResult.ContainerOpenFailed("avformat_find_stream_info failed");
            }

            // Find a video stream with a decoder FFmpeg knows. av_find_best_stream
            // skips streams whose codec is unregistered, so X-OCN / ARRIRAW MXF
            // come back as "no best stream" even though a video stream exists.
            // Walk the streams ourselves to tell "no video at all" apart from
            // "video with a codec FFmpeg has no decoder for".
            AVCodec* pCodec = null;
            var videoStreamIndex = ffmpeg.av_find_best_stream(
                pFormatContext, AVMediaType.AVMEDIA_TYPE_VIDEO, -1, -1, &pCodec, 0);

            if (videoStreamIndex < 0 || pCodec == null)
            {
                // Scan for any video-typed stream, even if FFmpeg can't decode it.
                int firstVideoIdx = -1;
                for (var i = 0; i < (int)pFormatContext->nb_streams; i++)
                {
                    if (pFormatContext->streams[i]->codecpar->codec_type == AVMediaType.AVMEDIA_TYPE_VIDEO)
                    {
                        firstVideoIdx = i;
                        break;
                    }
                }

                if (firstVideoIdx < 0)
                {
                    Log.Debug("FFmpeg: No video stream of any kind in {FileName}", fileName);
                    return ThumbnailResult.ContainerOpenFailed("no video stream");
                }

                // We have a video stream but no decoder — classify as NoDecoder
                // and report the codec tag so the UI can show something like
                // "F55_X-OCN_LT_6K_3:2 — needs Sony SDK".
                var codecpar = pFormatContext->streams[firstVideoIdx]->codecpar;
                var codecName = ffmpeg.avcodec_get_name(codecpar->codec_id);
                if (string.IsNullOrWhiteSpace(codecName) || codecName == "none")
                {
                    codecName = FourCcToString(codecpar->codec_tag)
                                ?? $"codec_id {codecpar->codec_id}";
                }

                Log.Information("FFmpeg: no decoder for video codec {Codec} in {File}", codecName, fileName);
                return ThumbnailResult.NoDecoder(codecName);
            }
            
            var pStream = pFormatContext->streams[videoStreamIndex];
            
            // Debug: Check actual duration from format context
            var ffmpegDuration = pFormatContext->duration > 0 
                ? TimeSpan.FromSeconds(pFormatContext->duration / (double)ffmpeg.AV_TIME_BASE)
                : duration;
            var streamDuration = pStream->duration > 0
                ? TimeSpan.FromSeconds(pStream->duration * ffmpeg.av_q2d(pStream->time_base))
                : TimeSpan.Zero;
            var nbFrames = pStream->nb_frames;
            var frameRate = ffmpeg.av_q2d(pStream->avg_frame_rate);
            var calculatedDuration = nbFrames > 0 && frameRate > 0 
                ? TimeSpan.FromSeconds(nbFrames / frameRate) 
                : TimeSpan.Zero;
            
            Log.Debug("FFmpeg: Duration format={FormatDur:g}, stream={StreamDur:g}, nb_frames={NbFrames}, fps={Fps:F2}, calculated={CalcDur:g}, provided={Provided:g} for {FileName}",
                ffmpegDuration, streamDuration, nbFrames, frameRate, calculatedDuration, duration, fileName);
            
            // Use the best available duration - prefer calculated from nb_frames, then provided, then FFmpeg
            var effectiveDuration = calculatedDuration > TimeSpan.Zero ? calculatedDuration 
                : duration > TimeSpan.Zero ? duration 
                : ffmpegDuration;
            
            // Create codec context
            pCodecContext = ffmpeg.avcodec_alloc_context3(pCodec);
            if (ffmpeg.avcodec_parameters_to_context(pCodecContext, pStream->codecpar) < 0)
            {
                Log.Debug("FFmpeg: Failed to copy codec params for {FileName}", fileName);
                return ThumbnailResult.DecodeFailed("avcodec_parameters_to_context failed");
            }

            if (ffmpeg.avcodec_open2(pCodecContext, pCodec, null) < 0)
            {
                Log.Debug("FFmpeg: Failed to open codec for {FileName}", fileName);
                return ThumbnailResult.DecodeFailed("avcodec_open2 failed");
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
                return ThumbnailResult.DecodeFailed("sws_getContext returned null");
            }

            // Get file size for byte-based seeking fallback
            var fileSize = pFormatContext->pb != null ? ffmpeg.avio_size(pFormatContext->pb) : 0L;

            // Track per-position failure reasons so the final outcome classifies
            // correctly when we end up with zero frames.
            var seekFailures = 0;
            var decodeFailures = 0;

            // Extract frame at each position
            foreach (var position in positions)
            {
                if (cancellationToken.IsCancellationRequested)
                    break;
                
                var seekTimeSeconds = position * effectiveDuration.TotalSeconds;
                
                // Convert seek time to stream time_base units
                var timeBase = pStream->time_base;
                var targetPts = (long)(seekTimeSeconds * timeBase.den / timeBase.num);
                
                Log.Debug("FFmpeg: Seeking to {Position:P0} ({SeekTime:F2}s, pts={Pts}, timebase={Num}/{Den}) for {FileName}", 
                    position, seekTimeSeconds, targetPts, timeBase.num, timeBase.den, fileName);
                
                // Try byte-based seek first for MXF (more reliable when index is incomplete)
                var seekResult = -1;
                if (fileSize > 0)
                {
                    var bytePosition = (long)(position * fileSize * 0.95); // 95% to avoid seeking past end
                    seekResult = ffmpeg.av_seek_frame(pFormatContext, -1, bytePosition, ffmpeg.AVSEEK_FLAG_BYTE);
                    if (seekResult >= 0)
                    {
                        Log.Debug("FFmpeg: Byte seek to {BytePos} succeeded", bytePosition);
                    }
                }
                
                // Fallback to timestamp-based seeking
                if (seekResult < 0)
                {
                    seekResult = ffmpeg.av_seek_frame(pFormatContext, videoStreamIndex, targetPts, 
                        ffmpeg.AVSEEK_FLAG_BACKWARD);
                }
                
                if (seekResult < 0)
                {
                    // Try AV_TIME_BASE seek
                    var seekTarget = (long)(seekTimeSeconds * ffmpeg.AV_TIME_BASE);
                    seekResult = ffmpeg.av_seek_frame(pFormatContext, -1, seekTarget, ffmpeg.AVSEEK_FLAG_BACKWARD);
                }
                
                if (seekResult < 0)
                {
                    Log.Debug("FFmpeg: All seek methods failed for {FileName} at {Position}", fileName, position);
                    seekFailures++;
                    continue;
                }

                ffmpeg.avcodec_flush_buffers(pCodecContext);

                // Decode frames until we get a valid frame
                var frameDecoded = false;
                var attempts = 0;

                while (!frameDecoded && attempts < MaxDecodeAttemptsPerPosition)
                {
                    attempts++;
                    ffmpeg.av_packet_unref(pPacket);
                    
                    var readResult = ffmpeg.av_read_frame(pFormatContext, pPacket);
                    if (readResult < 0)
                    {
                        Log.Debug("FFmpeg: av_read_frame returned {Result} (AVERROR_EOF={EOF})", 
                            readResult, ffmpeg.AVERROR_EOF);
                        break;
                    }
                    
                    if (pPacket->stream_index != videoStreamIndex)
                        continue;
                    
                    var sendResult = ffmpeg.avcodec_send_packet(pCodecContext, pPacket);
                    if (sendResult < 0)
                    {
                        Log.Debug("FFmpeg: avcodec_send_packet returned {Result}", sendResult);
                        continue;
                    }
                    
                    var receiveResult = ffmpeg.avcodec_receive_frame(pCodecContext, pFrame);
                    if (receiveResult == 0)
                    {
                        // Accept first successfully decoded frame after seek
                        frameDecoded = true;
                        var framePts = pFrame->pts != ffmpeg.AV_NOPTS_VALUE ? pFrame->pts : pFrame->best_effort_timestamp;
                        Log.Debug("FFmpeg: Got frame at pts={FramePts} (target={TargetPts}) after {Attempts} attempts", 
                            framePts, targetPts, attempts);
                    }
                }
                
                if (!frameDecoded)
                {
                    Log.Debug("FFmpeg: Failed to decode frame at {Position} for {FileName}", position, fileName);
                    decodeFailures++;
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

            if (thumbnails.Count > 0)
                return ThumbnailResult.Success(thumbnails);

            // Got zero frames despite a usable decoder. Classify by whichever
            // failure mode dominated across the requested positions.
            var detail = $"seek failures: {seekFailures}, decode failures: {decodeFailures}";
            return seekFailures >= decodeFailures
                ? ThumbnailResult.SeekFailed(detail)
                : ThumbnailResult.DecodeFailed(detail);
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "FFmpeg extraction failed for {FileName}", fileName);
            return ThumbnailResult.DecodeFailed(ex.Message);
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
    }
    
    /// <summary>
    /// Decodes a single still-image file (DPX, PNG, JPG, EXR, anything FFmpeg's
    /// image demuxer recognizes) into a <see cref="ThumbnailFrame"/> at the
    /// given target width. Used by vendor thumbnail services that produce
    /// intermediate stills (e.g. <c>SonyRawExporterThumbnailService</c> writes
    /// DPX via Sony's CLI and asks us to ingest them).
    /// </summary>
    public unsafe ThumbnailFrame? DecodeSingleImage(
        string imagePath,
        int targetWidth,
        double position,
        TimeSpan timestamp)
    {
        if (!IsAvailable) return null;

        AVFormatContext* pFormatContext = null;
        AVCodecContext* pCodecContext = null;
        AVFrame* pFrame = null;
        AVPacket* pPacket = null;
        SwsContext* pSwsContext = null;
        byte* rgbBuffer = null;

        try
        {
            pFormatContext = ffmpeg.avformat_alloc_context();
            var pLocal = pFormatContext;
            if (ffmpeg.avformat_open_input(&pLocal, imagePath, null, null) != 0)
            {
                Log.Debug("FFmpeg image decoder: failed to open {File}", Path.GetFileName(imagePath));
                return null;
            }
            pFormatContext = pLocal;

            if (ffmpeg.avformat_find_stream_info(pFormatContext, null) < 0) return null;

            AVCodec* pCodec = null;
            var streamIdx = ffmpeg.av_find_best_stream(
                pFormatContext, AVMediaType.AVMEDIA_TYPE_VIDEO, -1, -1, &pCodec, 0);
            if (streamIdx < 0 || pCodec == null) return null;

            var pStream = pFormatContext->streams[streamIdx];
            pCodecContext = ffmpeg.avcodec_alloc_context3(pCodec);
            if (ffmpeg.avcodec_parameters_to_context(pCodecContext, pStream->codecpar) < 0) return null;
            if (ffmpeg.avcodec_open2(pCodecContext, pCodec, null) < 0) return null;

            var sourceWidth = pCodecContext->width;
            var sourceHeight = pCodecContext->height;
            if (sourceWidth <= 0 || sourceHeight <= 0) return null;
            var aspectRatio = sourceWidth / (double)sourceHeight;
            var targetHeight = (int)(targetWidth / aspectRatio);
            targetWidth = (targetWidth / 2) * 2;
            targetHeight = (targetHeight / 2) * 2;

            pFrame = ffmpeg.av_frame_alloc();
            pPacket = ffmpeg.av_packet_alloc();

            var bufferSize = ffmpeg.av_image_get_buffer_size(
                AVPixelFormat.AV_PIX_FMT_BGRA, targetWidth, targetHeight, 1);
            rgbBuffer = (byte*)ffmpeg.av_malloc((ulong)bufferSize);

            var dstData = new byte_ptrArray4();
            var dstLinesize = new int_array4();
            ffmpeg.av_image_fill_arrays(ref dstData, ref dstLinesize, rgbBuffer,
                AVPixelFormat.AV_PIX_FMT_BGRA, targetWidth, targetHeight, 1);

            pSwsContext = ffmpeg.sws_getContext(
                sourceWidth, sourceHeight, pCodecContext->pix_fmt,
                targetWidth, targetHeight, AVPixelFormat.AV_PIX_FMT_BGRA,
                2, null, null, null);
            if (pSwsContext == null) return null;

            // Read packets until we get one decodable frame. For an image file
            // the first packet is the whole picture, so this loops at most a
            // couple of times.
            for (int attempts = 0; attempts < 16; attempts++)
            {
                ffmpeg.av_packet_unref(pPacket);
                var read = ffmpeg.av_read_frame(pFormatContext, pPacket);
                if (read < 0) break;
                if (pPacket->stream_index != streamIdx) continue;
                if (ffmpeg.avcodec_send_packet(pCodecContext, pPacket) < 0) continue;
                var recv = ffmpeg.avcodec_receive_frame(pCodecContext, pFrame);
                if (recv != 0) continue;

                ffmpeg.sws_scale(pSwsContext,
                    pFrame->data, pFrame->linesize,
                    0, pFrame->height,
                    dstData, dstLinesize);

                return ConvertToThumbnail(rgbBuffer, targetWidth, targetHeight, dstLinesize[0], position, timestamp);
            }

            return null;
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "FFmpeg image decoder: unexpected failure on {File}", Path.GetFileName(imagePath));
            return null;
        }
        finally
        {
            if (rgbBuffer != null) ffmpeg.av_free(rgbBuffer);
            if (pSwsContext != null) ffmpeg.sws_freeContext(pSwsContext);
            if (pPacket != null) { var p = pPacket; ffmpeg.av_packet_free(&p); }
            if (pFrame != null) { var f = pFrame; ffmpeg.av_frame_free(&f); }
            if (pCodecContext != null) { var c = pCodecContext; ffmpeg.avcodec_free_context(&c); }
            if (pFormatContext != null) { var f = pFormatContext; ffmpeg.avformat_close_input(&f); }
        }
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
