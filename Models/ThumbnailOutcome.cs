namespace LunaApp.Models;

/// <summary>
/// The result of trying to extract thumbnail frames from a clip. Today the
/// pipeline collapses five distinct outcomes into "Thumbnails is an empty
/// list" — which is why Sony Venice X-OCN looks identical in the UI to a
/// corrupted file. This enum plus the accompanying
/// <see cref="CameraClip.ThumbnailOutcomeDetail"/> string splits them back
/// apart so the UI can render the right placeholder.
/// </summary>
public enum ThumbnailOutcome
{
    /// <summary>Thumbnail extraction wasn't requested (count=0, duration=0, or caller skipped it).</summary>
    NotAttempted,

    /// <summary>Frames produced successfully.</summary>
    Success,

    /// <summary>FFmpeg has no decoder for this clip's codec (ARRIRAW, X-OCN, BRAW, REDCODE, …). Vendor SDK required.</summary>
    NoDecoder,

    /// <summary>Decoder exists but seeking into the stream didn't land on a decodable frame.</summary>
    SeekFailed,

    /// <summary>Decoder exists and seek succeeded, but decoding the packets failed.</summary>
    DecodeFailed,

    /// <summary>Opening the container with FFmpeg failed outright (corrupt / unknown format).</summary>
    ContainerOpenFailed,
}
