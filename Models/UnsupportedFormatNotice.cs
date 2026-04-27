namespace LunaApp.Models;

/// <summary>
/// Attached to a <see cref="CameraClip"/> when its format belongs to a camera
/// support that isn't Ready yet. The UI renders the message inline on the clip
/// row instead of showing an empty thumbnail or cryptic FFmpeg errors.
/// </summary>
public sealed record UnsupportedFormatNotice(
    string CameraSupportId,
    string DisplayName,
    string Reason);
