namespace LunaApp.Models;

/// <summary>
/// Represents a single camera clip with metadata and thumbnails.
/// </summary>
public sealed class CameraClip
{
    public required string FilePath { get; init; }
    public required string FileName { get; init; }
    public required long FileSizeBytes { get; init; }
    
    // Technical metadata
    public string? Codec { get; set; }
    public string? Container { get; set; }
    public int Width { get; set; }
    public int Height { get; set; }
    public double FrameRate { get; set; }
    public TimeSpan Duration { get; set; }
    public string? Timecode { get; set; }
    public int BitDepth { get; set; }
    public string? ColorSpace { get; set; }
    
    // Camera metadata
    public string? CameraManufacturer { get; set; }
    public string? CameraModel { get; set; }
    public string? CameraSerial { get; set; }
    public string? ReelName { get; set; }
    public int? ClipNumber { get; set; }
    public int? Iso { get; set; }
    public int? WhiteBalance { get; set; }
    public string? Lens { get; set; }
    public string? FocalLength { get; set; }
    public string? TStop { get; set; }
    public string? ShutterAngle { get; set; }
    public string? ShutterSpeed { get; set; }
    public string? Gamma { get; set; }
    public string? LookName { get; set; }
    public DateTime? RecordedDate { get; set; }
    
    // Thumbnails (base64 encoded or file paths)
    public List<ThumbnailFrame> Thumbnails { get; set; } = [];
    
    // First thumbnail for UI preview
    public ThumbnailFrame? FirstThumbnail => Thumbnails.Count > 0 ? Thumbnails[0] : null;
    public bool HasThumbnail => Thumbnails.Count > 0 && Thumbnails[0].HasImage;
    
    // Processing state
    public ClipProcessingState ProcessingState { get; set; } = ClipProcessingState.Pending;
    public string? ProcessingError { get; set; }
    
    public string Resolution => $"{Width}x{Height}";
    public string FileSizeFormatted => FormatFileSize(FileSizeBytes);
    public string DurationFormatted => Duration.ToString(@"hh\:mm\:ss\:ff");
    
    private static string FormatFileSize(long bytes)
    {
        string[] sizes = ["B", "KB", "MB", "GB", "TB"];
        int order = 0;
        double size = bytes;
        while (size >= 1024 && order < sizes.Length - 1)
        {
            order++;
            size /= 1024;
        }
        return $"{size:0.##} {sizes[order]}";
    }
}

public enum ClipProcessingState
{
    Pending,
    Processing,
    Completed,
    Failed
}
