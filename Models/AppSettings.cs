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
    /// Window state (persisted across sessions by the main window code-behind).
    /// </summary>
    public double WindowWidth { get; set; } = 1200;
    public double WindowHeight { get; set; } = 800;
    public int? WindowX { get; set; }
    public int? WindowY { get; set; }
    public bool IsMaximized { get; set; } = false;

    /// <summary>
    /// When non-null, the update banner is suppressed until this date. Set by
    /// the user choosing "Remind me later" on the update toast.
    /// </summary>
    public DateTime? UpdateSnoozeUntil { get; set; }

    /// <summary>
    /// When non-null, the camera-support toast is suppressed until this date.
    /// Set by the user choosing "Remind me later" on the toast. Mirrors the
    /// shape of <see cref="UpdateSnoozeUntil"/>.
    /// </summary>
    public DateTime? CameraSupportSnoozeUntil { get; set; }

    /// <summary>
    /// FFmpeg library path (if custom)
    /// </summary>
    public string? FfmpegPath { get; set; }
    
    /// <summary>
    /// Load settings from disk. Uses the source-generated JSON context because
    /// the app is trimmed and reflection-based <c>JsonSerializer</c> throws at
    /// runtime under trimming.
    /// </summary>
    public static AppSettings Load()
    {
        try
        {
            if (File.Exists(SettingsPath))
            {
                var json = File.ReadAllText(SettingsPath);
                return JsonSerializer.Deserialize(json, AppSettingsJsonContext.Default.AppSettings)
                       ?? new AppSettings();
            }
        }
        catch (Exception ex)
        {
            Serilog.Log.Warning(ex, "Failed to load settings from {Path} — falling back to defaults", SettingsPath);
        }

        return new AppSettings();
    }

    /// <summary>
    /// Save settings to disk. Returns true on success, false on failure (so
    /// callers can tell whether persistence actually happened — the UI logs
    /// the positive path, so we mustn't pretend success on throw).
    /// </summary>
    public bool Save()
    {
        try
        {
            var directory = Path.GetDirectoryName(SettingsPath);
            if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
            {
                Directory.CreateDirectory(directory);
            }

            var json = JsonSerializer.Serialize(this, AppSettingsJsonContext.Default.AppSettings);
            File.WriteAllText(SettingsPath, json);
            return true;
        }
        catch (Exception ex)
        {
            Serilog.Log.Error(ex, "Failed to save settings to {Path}", SettingsPath);
            return false;
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
