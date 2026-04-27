using LunaApp.Models;
using MediaInfo;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Primary metadata extractor backed by libmediainfo. Reads container-level technical
/// metadata for essentially every professional camera/container format (MOV, MXF, MP4,
/// R3D, BRAW, ARRI, etc.) and populates the baseline <see cref="CameraClip"/>.
/// Vendor-specific enrichers run afterwards to add sensor/lens/exposure detail.
/// </summary>
public sealed class MediaInfoMetadataExtractor : IMetadataExtractor
{
    public Task<CameraClip> ExtractAsync(string filePath, CancellationToken cancellationToken)
    {
        return Task.Run(() => ExtractCore(filePath), cancellationToken);
    }

    private static CameraClip ExtractCore(string filePath)
    {
        var fileInfo = new FileInfo(filePath);
        var clip = new CameraClip
        {
            FilePath = filePath,
            FileName = fileInfo.Name,
            FileSizeBytes = fileInfo.Exists ? fileInfo.Length : 0,
            Container = fileInfo.Extension.TrimStart('.').ToUpperInvariant(),
        };

        try
        {
            var media = new MediaInfoWrapper(filePath);

            if (!media.HasVideo)
            {
                Log.Debug("MediaInfo reports no video stream in {File}", fileInfo.Name);
                return clip;
            }

            var video = media.BestVideoStream;
            if (video == null)
            {
                return clip;
            }

            clip.Width = video.Width;
            clip.Height = video.Height;
            clip.FrameRate = video.FrameRate;
            clip.Duration = video.Duration;
            clip.Codec = NormalizeCodec(video.CodecName, video.Format);
            clip.BitDepth = video.BitDepth;
            clip.ColorSpace = video.ColorSpace.ToString();
            clip.Timecode = string.IsNullOrWhiteSpace(video.TimeCodeFirstFrame) ? null : video.TimeCodeFirstFrame;
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "MediaInfo extraction failed for {File}", fileInfo.Name);
        }

        return clip;
    }

    private static string? NormalizeCodec(string? codecName, string? format)
    {
        if (!string.IsNullOrWhiteSpace(codecName)) return codecName;
        if (!string.IsNullOrWhiteSpace(format)) return format;
        return null;
    }
}
