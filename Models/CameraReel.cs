using CommunityToolkit.Mvvm.ComponentModel;

namespace LunaApp.Models;

/// <summary>
/// Represents a camera reel/roll containing multiple clips.
/// Typically corresponds to a single camera card offload.
/// </summary>
public partial class CameraReel : ObservableObject
{
    [ObservableProperty]
    private string _label = string.Empty;
    
    [ObservableProperty]
    private string _sourcePath = string.Empty;
    
    /// <summary>
    /// Auto-detected reel identifier (e.g., A001, B002)
    /// </summary>
    public string? DetectedReelId { get; set; }
    
    /// <summary>
    /// Camera that recorded this reel
    /// </summary>
    public string? CameraName { get; set; }
    
    /// <summary>
    /// Date the reel was recorded
    /// </summary>
    public DateTime? RecordedDate { get; set; }
    
    /// <summary>
    /// All clips in this reel
    /// </summary>
    public List<CameraClip> Clips { get; set; } = [];
    
    /// <summary>
    /// Total duration of all clips
    /// </summary>
    public TimeSpan TotalDuration => TimeSpan.FromTicks(Clips.Sum(c => c.Duration.Ticks));
    
    /// <summary>
    /// Total file size of all clips
    /// </summary>
    public long TotalSizeBytes => Clips.Sum(c => c.FileSizeBytes);
    
    /// <summary>
    /// Number of clips in this reel
    /// </summary>
    public int ClipCount => Clips.Count;
    
    /// <summary>
    /// Display label (uses custom label if set, otherwise detected reel ID)
    /// </summary>
    public string DisplayLabel => !string.IsNullOrWhiteSpace(Label) ? Label : DetectedReelId ?? "Unknown Reel";
}
