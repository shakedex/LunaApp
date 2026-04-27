using LunaApp.Models;
using Serilog;
using System.Diagnostics;

namespace LunaApp.Services;

/// <summary>
/// Orchestrates the complete scan → generate workflow and forwards phased
/// progress to whoever subscribes (usually <c>MainWindowViewModel</c>).
/// </summary>
public sealed class ReportGenerationService
{
    private readonly ReelDetectionService _reelDetectionService;
    private readonly HtmlReportService _htmlReportService;
    private readonly PdfReportService _pdfReportService;

    public ReportGenerationService(
        ReelDetectionService reelDetectionService,
        HtmlReportService htmlReportService,
        PdfReportService pdfReportService)
    {
        _reelDetectionService = reelDetectionService;
        _htmlReportService = htmlReportService;
        _pdfReportService = pdfReportService;
    }

    public ReportProject? CurrentProject { get; private set; }

    /// <summary>
    /// Fired on every progress tick from the underlying pipeline. Carries the
    /// current phase, count, and item name — view-model layers ETA on top.
    /// </summary>
    public event Action<ProcessingReport>? ProgressReported;

    public Task<int> CountMediaFilesAsync(string folderPath)
    {
        return _reelDetectionService.CountMediaFilesAsync(folderPath);
    }

    public async Task<List<CameraReel>> ScanFolderAsync(
        string folderPath,
        CancellationToken cancellationToken = default)
    {
        var reels = await _reelDetectionService.DetectReelsAsync(
            folderPath,
            new Progress<ProcessingReport>(r => ProgressReported?.Invoke(r)),
            cancellationToken);

        CurrentProject = new ReportProject { Reels = reels };
        return reels;
    }

    public void AddReel(CameraReel reel)
    {
        CurrentProject ??= new ReportProject();
        CurrentProject.Reels.Add(reel);
    }

    public async Task<List<string>> GenerateReportsAsync(
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        if (CurrentProject == null || CurrentProject.Reels.Count == 0)
            throw new InvalidOperationException("No reels loaded. Call ScanFolderAsync first.");

        CurrentProject.Settings = settings;

        // Pin a single timestamp for this run so the HTML and PDF outputs
        // share one folder name + filename even if the wall clock crosses
        // a second between calls. ReportNaming reads this back through
        // settings.RunAt instead of calling DateTime.Now twice.
        settings.RunAt = DateTime.Now;

        var outputPaths = new List<string>();
        var stopwatch = Stopwatch.StartNew();

        Directory.CreateDirectory(settings.OutputFolder);

        try
        {
            if (settings.GenerateHtml)
            {
                ProgressReported?.Invoke(new ProcessingReport(ProcessingPhase.GeneratingHtml, 0, 1));
                var htmlPath = await _htmlReportService.GenerateAndSaveProjectReportAsync(
                    CurrentProject.Reels, settings, cancellationToken);
                outputPaths.Add(htmlPath);
                CurrentProject.HtmlReportPath = htmlPath;
                ProgressReported?.Invoke(new ProcessingReport(ProcessingPhase.GeneratingHtml, 1, 1));
            }

            if (settings.GeneratePdf)
            {
                ProgressReported?.Invoke(new ProcessingReport(ProcessingPhase.GeneratingPdf, 0, 1));
                var pdfPath = await _pdfReportService.GenerateAndSaveProjectReportAsync(
                    CurrentProject.Reels, settings, cancellationToken);
                outputPaths.Add(pdfPath);
                CurrentProject.PdfReportPath = pdfPath;
                ProgressReported?.Invoke(new ProcessingReport(ProcessingPhase.GeneratingPdf, 1, 1));
            }

            CurrentProject.CompletedAt = DateTime.Now;
            stopwatch.Stop();

            ProgressReported?.Invoke(new ProcessingReport(ProcessingPhase.Finalizing, 1, 1));
            Log.Information("Report generation completed in {ElapsedMs}ms", stopwatch.ElapsedMilliseconds);

            if (settings.OpenReportWhenDone && outputPaths.Count > 0)
            {
                var pathToOpen = outputPaths.FirstOrDefault(p => p.EndsWith(".html")) ?? outputPaths[0];
                OpenFile(pathToOpen);
            }

            return outputPaths;
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            Log.Error(ex, "Report generation failed");
            throw;
        }
    }

    public void ClearProject()
    {
        CurrentProject = null;
        ProgressReported?.Invoke(ProcessingReport.Idle);
    }

    private static void OpenFile(string path)
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = path,
                UseShellExecute = true
            });
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to open file: {Path}", path);
        }
    }
}
