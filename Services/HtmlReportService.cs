using LunaApp.Models;
using LunaApp.Services.Reports;
using LunaApp.Services.Reports.Html;
using Serilog;

namespace LunaApp.Services;

/// <summary>
/// Generates HTML camera reports. Thin facade — the actual document generation
/// lives in <see cref="ReelReportBuilder"/>, <see cref="ProjectReportBuilder"/>
/// and <see cref="ClipCardRenderer"/>. CSS lives in <see cref="ReportStylesheet"/>.
/// </summary>
public sealed class HtmlReportService
{
    public Task<string> GenerateReportAsync(
        CameraReel reel,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return Task.Run(() => ReelReportBuilder.Build(reel, settings), cancellationToken);
    }

    public Task<string> GenerateProjectReportAsync(
        IReadOnlyList<CameraReel> reels,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return Task.Run(() => ProjectReportBuilder.Build(reels, settings), cancellationToken);
    }

    public async Task<string> GenerateAndSaveProjectReportAsync(
        IReadOnlyList<CameraReel> reels,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        var html = await GenerateProjectReportAsync(reels, settings, cancellationToken);

        var reportName = ReportNaming.ProjectReportName(settings);
        var reportFolder = Path.Combine(settings.OutputFolder, reportName);
        Directory.CreateDirectory(reportFolder);

        var htmlPath = Path.Combine(reportFolder, $"{reportName}.html");
        await File.WriteAllTextAsync(htmlPath, html, cancellationToken);

        Log.Information("Project HTML report saved: {Path}", htmlPath);
        return htmlPath;
    }

    public async Task<string> GenerateAndSaveReportAsync(
        CameraReel reel,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        var html = await GenerateReportAsync(reel, settings, cancellationToken);

        var reportName = settings.GenerateReportName(reel);
        var reportFolder = Path.Combine(settings.OutputFolder, reportName);
        Directory.CreateDirectory(reportFolder);

        var htmlPath = Path.Combine(reportFolder, $"{reportName}.html");
        await File.WriteAllTextAsync(htmlPath, html, cancellationToken);

        Log.Information("HTML report saved: {Path}", htmlPath);
        return htmlPath;
    }
}
