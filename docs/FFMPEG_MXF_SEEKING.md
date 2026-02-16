# FFmpeg MXF Seeking Solution

## Problem

ARRI ProRes 4444 MXF files have incomplete index tables that cause FFmpeg to:
1. Report incorrect duration (e.g., 7.33 seconds instead of actual 2:56)
2. Fail timestamp-based seeking (seeks to start of file regardless of target)
3. Return EOF immediately after seeking

### Symptoms in Logs
```
FFmpeg: Duration format=0:00:07.333333, stream=0:00:07.333333, provided=0:02:56
FFmpeg: Seeking to 50% (88.00s, pts=2112) 
FFmpeg: Got frame at pts=175 (target=2112) after 1 attempts  # Wrong frame!
```

## Solution: Byte-Based Seeking

MXF files store video data linearly. Since FFmpeg can't parse the index properly, we bypass it by seeking to a **byte position** instead of a timestamp.

### Key Insight
```
position_in_file = (desired_percentage * file_size * 0.95)
```
The 0.95 factor prevents seeking past the actual end of video data.

### Implementation

```csharp
// Get file size for byte-based seeking
var fileSize = pFormatContext->pb != null ? ffmpeg.avio_size(pFormatContext->pb) : 0L;

// Try byte-based seek first (more reliable for MXF with incomplete index)
if (fileSize > 0)
{
    var bytePosition = (long)(position * fileSize * 0.95);
    var seekResult = ffmpeg.av_seek_frame(pFormatContext, -1, bytePosition, ffmpeg.AVSEEK_FLAG_BYTE);
    if (seekResult >= 0)
    {
        // Success! Flush decoder and read frames
        ffmpeg.avcodec_flush_buffers(pCodecContext);
    }
}
```

### Additional Optimizations

1. **Increase probe size** for MXF files:
```csharp
ffmpeg.av_dict_set(&options, "probesize", "100000000", 0); // 100MB
ffmpeg.av_dict_set(&options, "analyzeduration", "100000000", 0); // 100 seconds
```

2. **Use provided duration** instead of FFmpeg's reported duration:
```csharp
// Prefer: calculated from nb_frames → provided by metadata extractor → FFmpeg format
var effectiveDuration = calculatedDuration > TimeSpan.Zero ? calculatedDuration 
    : duration > TimeSpan.Zero ? duration 
    : ffmpegDuration;
```

3. **Fallback chain** for seeking:
   - Byte-based seek (AVSEEK_FLAG_BYTE)
   - Stream-specific timestamp seek (AVSEEK_FLAG_BACKWARD)
   - AV_TIME_BASE timestamp seek

## Working Log Output
```
FFmpeg: Duration format=0:00:07.333333, nb_frames=0, provided=0:02:56
FFmpeg: Seeking to 10% (17.60s, pts=422)
FFmpeg: Byte seek to 86810345 succeeded
FFmpeg: Got frame at pts=15 (target=422) after 4 attempts  # Correct position!

FFmpeg: Seeking to 50% (88.00s, pts=2112)
FFmpeg: Byte seek to 434051726 succeeded
FFmpeg: Got frame at pts=86 (target=2112) after 4 attempts  # Correct position!

FFmpeg: Seeking to 90% (158.40s, pts=3801)  
FFmpeg: Byte seek to 781293107 succeeded
FFmpeg: Got frame at pts=156 (target=3801) after 4 attempts  # Correct position!
```

## Why This Works

1. **ARRI MXF structure**: Video essence is stored linearly in the file
2. **Byte seeking**: Jumps to a file offset, then the decoder finds the nearest keyframe
3. **Index bypass**: We don't rely on the (broken) MXF index tables
4. **ProRes is all-intra**: Every frame is a keyframe, so seeking is accurate

## Files Affected

- `Services/Chappie/FfmpegThumbnailService.cs` - Main implementation
- `tools/ffmpeg/win-x64/` - FFmpeg 7.1 DLLs (avcodec-61, etc.)

## Dependencies

- `FFmpeg.AutoGen` NuGet package version 7.1.1
- FFmpeg 7.x native libraries (avcodec-61.dll, avformat-61.dll, etc.)

## Tested Formats

- ✅ ARRI ProRes 4444 MXF (ALEXA 35)
- ✅ Standard MOV/MP4 files
- ✅ Sony XAVC MXF (with proper index)

## Alternative Approaches Considered

1. **MPV CLI**: Works but requires shelling out to external process
2. **LibVLC**: Cannot decode ARRIRAW, struggles with seeking in some MXF
3. **Timestamp seeking only**: Fails due to incomplete MXF index tables
