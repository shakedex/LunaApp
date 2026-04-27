using LunaApp.Models;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Primary metadata extractor. Reads the container to produce a baseline CameraClip
/// with technical fields (codec, resolution, duration, frame rate, timecode) populated.
/// </summary>
public interface IMetadataExtractor
{
    Task<CameraClip> ExtractAsync(string filePath, CancellationToken cancellationToken);
}
