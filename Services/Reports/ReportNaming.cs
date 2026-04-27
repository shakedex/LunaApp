using LunaApp.Models;

namespace LunaApp.Services.Reports;

/// <summary>
/// Shared report naming helpers. Both the HTML and PDF services go through
/// these so they always land in the same folder for the same run.
/// </summary>
internal static class ReportNaming
{
    /// <summary>Project name as rendered in the UI / file names. Never empty.</summary>
    public static string EffectiveProjectName(ReportSettings settings) =>
        string.IsNullOrWhiteSpace(settings.ProjectName) ? "Camera Report" : settings.ProjectName!.Trim();

    /// <summary>Folder-safe report name, e.g. "Camera Report_2026-04-23".</summary>
    public static string ProjectReportName(ReportSettings settings, DateTime? at = null) =>
        $"{Sanitize(EffectiveProjectName(settings))}_{(at ?? DateTime.Now):yyyy-MM-dd}";

    private static string Sanitize(string value)
    {
        var invalid = Path.GetInvalidFileNameChars();
        return string.Join("_", value.Split(invalid, StringSplitOptions.RemoveEmptyEntries)).Trim();
    }
}
