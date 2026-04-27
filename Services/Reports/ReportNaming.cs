using LunaApp.Models;

namespace LunaApp.Services.Reports;

/// <summary>
/// Shared report naming helpers. Both the HTML and PDF services go through
/// these so they always land in the same folder for the same run, and
/// render the same title in the document header.
/// </summary>
internal static class ReportNaming
{
    /// <summary>Project name as rendered in the UI / file names. Never empty.</summary>
    public static string EffectiveProjectName(ReportSettings settings) =>
        string.IsNullOrWhiteSpace(settings.ProjectName) ? "Camera Report" : settings.ProjectName!.Trim();

    /// <summary>
    /// Display title shown in HTML <c>&lt;title&gt;</c> and the PDF cover.
    /// "Project — Report Name" when the user supplied a per-run name,
    /// otherwise just the project name. Em-dash so it reads as a subtitle,
    /// not a hyphenation.
    /// </summary>
    public static string DisplayTitle(ReportSettings settings)
    {
        var project = EffectiveProjectName(settings);
        var report = settings.ReportName?.Trim();
        return string.IsNullOrEmpty(report) ? project : $"{project} — {report}";
    }

    /// <summary>
    /// Folder-safe report bundle name. Always carries the project, optionally
    /// the per-run report name, and a date+time suffix so back-to-back
    /// generations on the same day land in distinct folders. Format:
    /// <c>{Project}[_{ReportName}]_{yyyy-MM-dd at HH.mm.ss}</c>.
    /// </summary>
    public static string ProjectReportName(ReportSettings settings)
    {
        var project = Sanitize(EffectiveProjectName(settings));
        var report = string.IsNullOrWhiteSpace(settings.ReportName)
            ? null
            : Sanitize(settings.ReportName!);
        var stamp = settings.RunAt.ToString("yyyy-MM-dd 'at' HH.mm.ss");

        return report is null
            ? $"{project}_{stamp}"
            : $"{project}_{report}_{stamp}";
    }

    private static string Sanitize(string value)
    {
        var invalid = Path.GetInvalidFileNameChars();
        return string.Join("_", value.Split(invalid, StringSplitOptions.RemoveEmptyEntries)).Trim();
    }
}
