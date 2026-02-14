using System.Text.Json;

namespace LunaApp.Models;

/// <summary>
/// Application-wide settings that persist between sessions.
/// </summary>
public sealed class AppSettings
{
    private static readonly string SettingsPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "Luna",
        "settings.json"
    );
    
    /// <summary>
    /// Default report settings (branding, output preferences)
    /// </summary>
    public ReportSettings DefaultReportSettings { get; set; } = new();
    
    /// <summary>
    /// Recently used source folders
    /// </summary>
    public List<string> RecentSources { get; set; } = [];
    
    /// <summary>
    /// Window state
    /// </summary>
    public double WindowWidth { get; set; } = 1200;
    public double WindowHeight { get; set; } = 800;
    public bool IsMaximized { get; set; } = false;
    
    /// <summary>
    /// FFmpeg library path (if custom)
    /// </summary>
    public string? FfmpegPath { get; set; }
    
    /// <summary>
    /// Load settings from disk
    /// </summary>
    public static AppSettings Load()
    {
        try
        {
            if (File.Exists(SettingsPath))
            {
                var json = File.ReadAllText(SettingsPath);
                return JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
            }
        }
        catch
        {
            // If loading fails, return default settings
        }
        
        return new AppSettings();
    }
    
    /// <summary>
    /// Save settings to disk
    /// </summary>
    public void Save()
    {
        try
        {
            var directory = Path.GetDirectoryName(SettingsPath);
            if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
            {
                Directory.CreateDirectory(directory);
            }
            
            var json = JsonSerializer.Serialize(this, new JsonSerializerOptions 
            { 
                WriteIndented = true 
            });
            File.WriteAllText(SettingsPath, json);
        }
        catch
        {
            // Silently fail if we can't save settings
        }
    }
    
    /// <summary>
    /// Add a source to recent sources list
    /// </summary>
    public void AddRecentSource(string path)
    {
        RecentSources.Remove(path);
        RecentSources.Insert(0, path);
        
        // Keep only last 10
        if (RecentSources.Count > 10)
        {
            RecentSources = RecentSources.Take(10).ToList();
        }
    }
}
