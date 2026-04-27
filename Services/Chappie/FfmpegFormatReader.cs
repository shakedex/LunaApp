using FFmpeg.AutoGen;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Opens a media file with FFmpeg's format layer only — no decoding — and
/// returns the container-level metadata as a flat string dictionary, plus the
/// file's <c>major_brand</c> / <c>compatible_brands</c> for QuickTime-family
/// containers. Cheap: reads header and atom metadata, nothing more.
///
/// Used by vendor enrichers (e.g. <c>ArriQuickTimeEnricher</c>) to detect
/// camera-specific tags like <c>com.arri.camera.*</c> without round-tripping
/// through a dedicated decoder.
/// </summary>
public sealed unsafe class FfmpegFormatReader
{
    public FfmpegFormatReader()
    {
        // Reuses the same init + library path resolution as the thumbnail service.
        FfmpegThumbnailService.EnsureInitialized();
    }

    public sealed record FormatMetadata(
        IReadOnlyDictionary<string, string> Tags,
        string? MajorBrand,
        IReadOnlyList<string> CompatibleBrands);

    public FormatMetadata? ReadMetadata(string filePath)
    {
        AVFormatContext* ctx = null;
        try
        {
            ctx = ffmpeg.avformat_alloc_context();
            var local = ctx;
            if (ffmpeg.avformat_open_input(&local, filePath, null, null) != 0)
            {
                Log.Debug("FFmpeg format reader: failed to open {File}", Path.GetFileName(filePath));
                return null;
            }
            ctx = local;

            // Header is enough for container tags — no need for avformat_find_stream_info.
            var tags = ReadDict(ctx->metadata);

            string? majorBrand = tags.TryGetValue("major_brand", out var mb) ? mb.Trim() : null;
            IReadOnlyList<string> compatibleBrands =
                tags.TryGetValue("compatible_brands", out var cb)
                    ? SplitQuickTimeBrands(cb)
                    : Array.Empty<string>();

            return new FormatMetadata(tags, majorBrand, compatibleBrands);
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "FFmpeg format reader: unexpected failure on {File}", Path.GetFileName(filePath));
            return null;
        }
        finally
        {
            if (ctx != null)
            {
                var c = ctx;
                ffmpeg.avformat_close_input(&c);
            }
        }
    }

    private static Dictionary<string, string> ReadDict(AVDictionary* dict)
    {
        var result = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        if (dict == null) return result;

        AVDictionaryEntry* entry = null;
        while ((entry = ffmpeg.av_dict_get(dict, "", entry, ffmpeg.AV_DICT_IGNORE_SUFFIX)) != null)
        {
            var key = System.Runtime.InteropServices.Marshal.PtrToStringUTF8((IntPtr)entry->key);
            var value = System.Runtime.InteropServices.Marshal.PtrToStringUTF8((IntPtr)entry->value);
            if (!string.IsNullOrEmpty(key) && value is not null)
                result[key] = value;
        }
        return result;
    }

    private static IReadOnlyList<string> SplitQuickTimeBrands(string compatibleBrands)
    {
        // QuickTime compatible_brands is a sequence of 4-char brand codes,
        // sometimes whitespace-separated, sometimes concatenated. We emit
        // whichever normalized tokens we find (trimmed, non-empty).
        var tokens = compatibleBrands
            .Split(new[] { ' ', '\t', '\0' }, StringSplitOptions.RemoveEmptyEntries)
            .Select(t => t.Trim())
            .Where(t => t.Length > 0)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();
        return tokens;
    }
}
