using System.Text.Json.Serialization;

namespace LunaApp.Models;

/// <summary>Visual theme applied to generated reports.</summary>
public enum ReportTheme
{
    Light,
    Dark,
}

/// <summary>
/// Settings for report generation and branding.
/// </summary>
public sealed class ReportSettings
{
    // Branding
    public string? LogoPath { get; set; }
    public string? LogoBase64 { get; set; }
    public string? ProjectName { get; set; }
    public string? ProductionCompany { get; set; }
    public string? DitName { get; set; }
    public string? Director { get; set; }
    public string? Dp { get; set; }

    // Output settings
    public string OutputFolder { get; set; } = GetDefaultOutputFolder();
    public int ThumbnailsPerClip { get; set; } = 3;
    public int ThumbnailWidth { get; set; } = 480;
    public bool GenerateHtml { get; set; } = true;
    public bool GeneratePdf { get; set; } = true;
    public bool OpenReportWhenDone { get; set; } = true;
    public bool SaveReportToSource { get; set; } = false;
    public bool GroupPdfsInSeparateFolder { get; set; } = false;

    /// <summary>Theme used for the generated HTML / PDF reports.</summary>
    public ReportTheme Theme { get; set; } = ReportTheme.Light;

    // Report naming
    public string ReportNamePattern { get; set; } = "{project}_{reel}_{date}";

    /// <summary>
    /// Per-run "Report Name" entered in the main window before clicking
    /// Generate. Appended to the project name in the report header
    /// ("Project Name — Report Name") and to the output folder. Runtime
    /// only — never persisted, since each generation can have a different
    /// label (Day 02, Reshoots, Studio A, etc.).
    /// </summary>
    [JsonIgnore]
    public string? ReportName { get; set; }

    /// <summary>
    /// Single timestamp shared between the HTML and PDF outputs of one
    /// generation. <see cref="ReportGenerationService"/> sets this once at
    /// the start of a run so both files land in the same folder even when
    /// the wall clock crosses a second between calls. Runtime only.
    /// </summary>
    [JsonIgnore]
    public DateTime RunAt { get; set; } = DateTime.Now;

    [JsonIgnore]
    public bool HasLogo => !string.IsNullOrEmpty(LogoPath) || !string.IsNullOrEmpty(LogoBase64);
    
    private static string GetDefaultOutputFolder()
    {
        var documentsPath = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
        return Path.Combine(documentsPath, "Luna Reports");
    }
    
    public string GenerateReportName(CameraReel reel)
    {
        var name = ReportNamePattern
            .Replace("{project}", SanitizeFileName(ProjectName ?? "Project"))
            .Replace("{reel}", SanitizeFileName(reel.DisplayLabel))
            .Replace("{date}", DateTime.Now.ToString("yyyy-MM-dd"))
            .Replace("{time}", DateTime.Now.ToString("HH-mm"));
        
        return name;
    }
    
    private static string SanitizeFileName(string name)
    {
        var invalid = Path.GetInvalidFileNameChars();
        return string.Join("_", name.Split(invalid, StringSplitOptions.RemoveEmptyEntries));
    }
}
