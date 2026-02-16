using LunaApp.Models;
using LunaApp.Services.Chappie;
using Serilog;
using System.Text.RegularExpressions;

namespace LunaApp.Services;

/// <summary>
/// Service for auto-detecting camera reels from folder structure and file patterns.
/// </summary>
public sealed partial class ReelDetectionService : IDisposable
{
    private readonly ChappieEngine _chappie = new();
    
    // Common camera card folder patterns to skip when finding meaningful folder names
    private static readonly string[] CameraFolderPatterns =
    [
        // Common structure folders
        "DCIM", "PRIVATE", "CLIPS", "CONTENTS", "STREAMS", "STREAM",
        "AVCHD", "BDMV", "BPAV", "CLPR", "CUEUP", "GENERAL", "PROAV",
        
        // Manufacturer folders
        "ARRI", "RED", "BRAW", "SONY", "BLACKMAGIC", "CANON", "PANASONIC", 
        "DJI", "GOPRO", "NIKON", "FUJI", "FUJIFILM",
        
        // Common project structure folders
        "CAMERA", "CAMERAS", "CAM", "FOOTAGE", "RAW", "ORIGINALS", "MASTERS",
        "A_CAM", "B_CAM", "C_CAM", "D_CAM", "A-CAM", "B-CAM", "C-CAM", "D-CAM",
        "ACAM", "BCAM", "CCAM", "DCAM",
        
        // Media type folders
        "VIDEO", "AUDIO", "PROXY", "PROXIES", "THUMBNAILS", "SUBS"
    ];
    
    // Reel naming patterns (A001, B002, REEL_01, etc.)
    [GeneratedRegex(@"^([A-Z])(\d{3})$", RegexOptions.IgnoreCase)]
    private static partial Regex ReelIdPattern();
    
    [GeneratedRegex(@"REEL[_\-]?(\d+)", RegexOptions.IgnoreCase)]
    private static partial Regex ReelNumberPattern();
    
    [GeneratedRegex(@"([A-Z]+\d+)[_\-]", RegexOptions.IgnoreCase)]
    private static partial Regex ClipPrefixPattern();
    
    /// <summary>
    /// Quick count of media files without processing (for pre-scan confirmation UI)
    /// </summary>
    public Task<int> CountMediaFilesAsync(string folderPath)
    {
        return Task.Run(() => FindMediaFiles(folderPath).Count());
    }
    
    /// <summary>
    /// Scan a folder and detect camera reels
    /// </summary>
    public async Task<List<CameraReel>> DetectReelsAsync(
        string folderPath,
        IProgress<(int current, int total, string status)>? progress = null,
        CancellationToken cancellationToken = default)
    {
        Log.Information("Scanning folder for reels: {FolderPath}", folderPath);
        
        progress?.Report((0, 100, "Scanning for media files..."));
        
        // Find all supported media files
        var mediaFiles = await Task.Run(() => 
            FindMediaFiles(folderPath).ToList(), 
            cancellationToken);
        
        if (mediaFiles.Count == 0)
        {
            Log.Warning("No media files found in {FolderPath}", folderPath);
            return [];
        }
        
        Log.Information("Found {Count} media files", mediaFiles.Count);
        foreach (var f in mediaFiles)
        {
            Log.Debug("Found media file: {File}", f);
        }
        progress?.Report((10, 100, $"Found {mediaFiles.Count} media files"));
        
        // Extract metadata and thumbnails using Chappie engine
        var clips = await _chappie.ProcessClipsAsync(
            mediaFiles,
            extractThumbnails: true,
            thumbnailCount: 3,
            thumbnailWidth: 480,
            new Progress<(int current, int total, string file)>(p =>
            {
                var pct = 10 + (int)(p.current / (double)p.total * 60);
                progress?.Report((pct, 100, $"Processing: {p.file}"));
            }),
            cancellationToken);
        
        Log.Information("Chappie processed {Count} clips", clips.Count);
        foreach (var c in clips)
        {
            Log.Debug("Processed clip: {File}, ReelName={Reel}, Camera={Camera}, State={State}", 
                c.FileName, c.ReelName ?? "null", c.CameraModel ?? "null", c.ProcessingState);
        }
        
        progress?.Report((70, 100, "Grouping clips into reels..."));
        
        // Group clips into reels
        var reels = GroupClipsIntoReels(clips, folderPath);
        
        progress?.Report((100, 100, $"Found {reels.Count} reels"));
        
        return reels;
    }
    
    /// <summary>
    /// Find all supported media files in a folder
    /// </summary>
    private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mov", ".mp4", ".mxf", ".avi", ".mkv", ".m4v",
        ".mts", ".m2ts", ".3gp", ".webm", ".wmv", ".flv",
        ".r3d", ".braw", ".ari"
    };
    
    private static IEnumerable<string> FindMediaFiles(string folderPath)
    {
        var searchOption = SearchOption.AllDirectories;
        
        foreach (var file in Directory.EnumerateFiles(folderPath, "*.*", searchOption))
        {
            var ext = Path.GetExtension(file);
            if (!SupportedExtensions.Contains(ext))
                continue;
            
            // Skip hidden/system files
            var attr = File.GetAttributes(file);
            if ((attr & FileAttributes.Hidden) == 0 && (attr & FileAttributes.System) == 0)
            {
                yield return file;
            }
        }
    }
    
    /// <summary>
    /// Group clips into reels based on folder structure.
    /// Scenario 1: Single card - all clips in one reel
    /// Scenario 2: Project folder - each subfolder with clips becomes a reel
    /// </summary>
    private List<CameraReel> GroupClipsIntoReels(List<CameraClip> clips, string sourcePath)
    {
        var normalizedRoot = Path.GetFullPath(sourcePath).TrimEnd(Path.DirectorySeparatorChar);
        var rootName = Path.GetFileName(normalizedRoot);
        
        // First, determine reel ID for each clip using metadata or folder structure
        var clipsByReel = clips
            .GroupBy(c => GetReelIdentifier(c, normalizedRoot))
            .ToList();
        
        // If all clips resolve to same reel, or only 1 group exists - single card mode
        if (clipsByReel.Count <= 1)
        {
            var reelName = clipsByReel.FirstOrDefault()?.Key ?? rootName;
            Log.Information("Single card mode: all {Count} clips in one reel '{Name}'", clips.Count, reelName);
            return
            [
                new CameraReel
                {
                    SourcePath = sourcePath,
                    DetectedReelId = reelName,
                    Label = reelName,
                    CameraName = clips.FirstOrDefault()?.CameraModel ?? clips.FirstOrDefault()?.CameraManufacturer,
                    RecordedDate = clips.Where(c => c.RecordedDate.HasValue).MinBy(c => c.RecordedDate)?.RecordedDate,
                    Clips = clips.OrderBy(c => c.Timecode ?? c.FileName).ToList()
                }
            ];
        }
        
        // Project mode: multiple reels detected
        Log.Information("Project mode: {Count} cards detected in '{Project}'", clipsByReel.Count, rootName);
        
        var reels = clipsByReel
            .Select(g => new CameraReel
            {
                SourcePath = sourcePath,
                DetectedReelId = g.Key,
                Label = g.Key,
                CameraName = g.First().CameraModel ?? g.First().CameraManufacturer,
                RecordedDate = g.Where(c => c.RecordedDate.HasValue).MinBy(c => c.RecordedDate)?.RecordedDate,
                Clips = g.OrderBy(c => c.Timecode ?? c.FileName).ToList()
            })
            .OrderBy(r => r.Label)
            .ToList();
        
        return reels;
    }
    
    /// <summary>
    /// Get reel identifier for a clip. Priority: metadata ReelName > folder structure
    /// </summary>
    private string GetReelIdentifier(CameraClip clip, string rootPath)
    {
        // Use embedded reel name if available
        if (!string.IsNullOrEmpty(clip.ReelName))
        {
            // Clean up reel name (remove suffixes like _1I7H)
            var reelName = clip.ReelName;
            var underscoreIdx = reelName.IndexOf('_');
            if (underscoreIdx > 3)
            {
                // Check if it looks like A001_xxxx format, keep just A001
                var prefix = reelName[..underscoreIdx];
                if (ReelIdPattern().IsMatch(prefix) || ClipPrefixPattern().IsMatch(prefix))
                {
                    reelName = prefix;
                }
            }
            return reelName;
        }
        
        // Fall back to folder structure analysis
        return GetCardFolderName(clip.FilePath, rootPath);
    }
    
    /// <summary>
    /// Get the card/reel identifier for a clip.
    /// Priority: 1) ReelName from metadata, 2) Reel-like folder name in path, 3) Immediate parent folder
    /// </summary>
    private string GetCardFolderName(string filePath, string rootPath)
    {
        var fileDir = Path.GetDirectoryName(filePath);
        if (string.IsNullOrEmpty(fileDir))
            return Path.GetFileName(rootPath);
        
        var normalizedFileDir = Path.GetFullPath(fileDir).TrimEnd(Path.DirectorySeparatorChar);
        var normalizedRoot = Path.GetFullPath(rootPath).TrimEnd(Path.DirectorySeparatorChar);
        
        // If file is directly in root, return root name
        if (normalizedFileDir.Equals(normalizedRoot, StringComparison.OrdinalIgnoreCase))
            return Path.GetFileName(normalizedRoot);
        
        // Get relative path from root to file's directory
        if (!normalizedFileDir.StartsWith(normalizedRoot, StringComparison.OrdinalIgnoreCase))
            return Path.GetFileName(normalizedRoot);
        
        var relativePath = normalizedFileDir[(normalizedRoot.Length + 1)..];
        var parts = relativePath.Split(Path.DirectorySeparatorChar);
        
        // Strategy 1: Find the deepest folder that looks like a reel ID (A001, B005, etc.)
        foreach (var part in parts.Reverse())
        {
            // Skip camera structure folders
            if (CameraFolderPatterns.Any(p => part.Equals(p, StringComparison.OrdinalIgnoreCase)))
                continue;
            
            // Check if this folder looks like a reel ID
            var reelId = ExtractReelId(part);
            if (reelId != null)
            {
                return reelId;
            }
        }
        
        // Strategy 2: Find deepest meaningful folder (skip camera structure folders)
        foreach (var part in parts.Reverse())
        {
            if (!CameraFolderPatterns.Any(p => part.Equals(p, StringComparison.OrdinalIgnoreCase)))
            {
                return part;
            }
        }
        
        // Fallback: use the first part of the path
        return parts[0];
    }
    
    /// <summary>
    /// Extract reel ID from a string using common patterns
    /// </summary>
    private static string? ExtractReelId(string name)
    {
        // Try A001 pattern
        var match = ReelIdPattern().Match(name);
        if (match.Success)
            return match.Value.ToUpperInvariant();
        
        // Try REEL_01 pattern
        match = ReelNumberPattern().Match(name);
        if (match.Success)
            return match.Value.ToUpperInvariant();
        
        // Try clip prefix pattern (e.g., A001_C001)
        match = ClipPrefixPattern().Match(name);
        if (match.Success)
            return match.Groups[1].Value.ToUpperInvariant();
        
        return null;
    }
    
    public void Dispose()
    {
        _chappie.Dispose();
    }
}
