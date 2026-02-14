# FFmpeg for Luna

Luna uses **FFmpeg.AutoGen** (native C# bindings) for high-performance thumbnail extraction with accurate seeking.

## Required Libraries

Luna needs FFmpeg shared libraries (DLLs/dylibs) to be present.

### Windows (win-x64)

Place these DLLs in `tools/ffmpeg/win-x64/`:
- `avcodec-61.dll`
- `avformat-61.dll`
- `avutil-59.dll`
- `swscale-8.dll`
- `swresample-5.dll`

**Download from:** https://www.gyan.dev/ffmpeg/builds/ (use "shared" build)

### macOS (osx-arm64 / osx-x64)

Place these dylibs in `tools/ffmpeg/osx-arm64/` or `tools/ffmpeg/osx-x64/`:
- `libavcodec.61.dylib`
- `libavformat.61.dylib`
- `libavutil.59.dylib`
- `libswscale.8.dylib`
- `libswresample.5.dylib`

**Or install via Homebrew:**
```bash
brew install ffmpeg
```
Luna will auto-detect Homebrew libraries at `/opt/homebrew/lib` (ARM) or `/usr/local/lib` (Intel).

## Directory Structure

```
tools/ffmpeg/
├── win-x64/
│   ├── avcodec-61.dll
│   ├── avformat-61.dll
│   ├── avutil-59.dll
│   ├── swscale-8.dll
│   └── swresample-5.dll
├── osx-arm64/
│   ├── libavcodec.61.dylib
│   └── ...
└── README.md
```

## How It's Used

Luna uses FFmpeg.AutoGen to:
1. Open video files with `avformat_open_input`
2. Seek to specific positions with `av_seek_frame`
3. Decode frames with `avcodec_send_packet` / `avcodec_receive_frame`
4. Scale to thumbnail size with `sws_scale`
5. Convert to WebP for embedding in reports

## Fallback

If FFmpeg libraries are not available, Luna falls back to LibVLC for thumbnail extraction (may not support all codecs or have accurate seeking).

## Version Compatibility

Luna is built against FFmpeg 7.x APIs. Library version numbers in filenames may vary (e.g., avcodec-60, avcodec-61) - the AutoGen bindings handle this dynamically.
