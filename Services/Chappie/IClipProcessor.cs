using LunaApp.Models;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Interface for clip metadata and thumbnail extraction.
/// Each manufacturer/format gets its own implementation.
/// </summary>
public interface IClipProcessor
{
    /// <summary>
    /// Priority for processor selection (higher = preferred)
    /// </summary>
    int Priority { get; }
    
    /// <summary>
    /// Check if this processor can handle the given file
    /// </summary>
    bool CanProcess(string filePath);
    
    /// <summary>
    /// Extract metadata from clip
    /// </summary>
    Task<CameraClip> ExtractMetadataAsync(string filePath, CancellationToken cancellationToken = default);
    
    /// <summary>
    /// Extract thumbnail frames from clip
    /// </summary>
    Task<List<ThumbnailFrame>> ExtractThumbnailsAsync(
        string filePath, 
        TimeSpan duration,
        int count = 3,
        int width = 480,
        CancellationToken cancellationToken = default);
}

/// <summary>
/// Detected camera manufacturer
/// </summary>
public enum CameraManufacturer
{
    Unknown,
    Arri,
    Sony,
    Red,
    Blackmagic,
    Canon,
    Panasonic,
    DJI
}
