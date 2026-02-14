namespace LunaApp.Models;

/// <summary>
/// Represents a complete report project containing reels and settings.
/// </summary>
public sealed class ReportProject
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public DateTime CreatedAt { get; init; } = DateTime.Now;
    public DateTime? CompletedAt { get; set; }
    
    /// <summary>
    /// All reels in this project
    /// </summary>
    public List<CameraReel> Reels { get; set; } = [];
    
    /// <summary>
    /// Branding and report settings
    /// </summary>
    public ReportSettings Settings { get; set; } = new();
    
    /// <summary>
    /// Output paths for generated reports
    /// </summary>
    public string? HtmlReportPath { get; set; }
    public string? PdfReportPath { get; set; }
    
    /// <summary>
    /// All clips across all reels
    /// </summary>
    public IEnumerable<CameraClip> AllClips => Reels.SelectMany(r => r.Clips);
    
    /// <summary>
    /// Total clip count
    /// </summary>
    public int TotalClipCount => Reels.Sum(r => r.ClipCount);
    
    /// <summary>
    /// Total duration across all clips
    /// </summary>
    public TimeSpan TotalDuration => TimeSpan.FromTicks(Reels.Sum(r => r.TotalDuration.Ticks));
}
