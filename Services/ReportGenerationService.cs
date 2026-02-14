using LunaApp.Models;
using Serilog;
using System.Diagnostics;

namespace LunaApp.Services;

/// <summary>
/// Orchestrates the complete report generation workflow.
/// </summary>
public sealed class ReportGenerationService : IDisposable
{
    private readonly ReelDetectionService _reelDetectionService = new();
    private readonly HtmlReportService _htmlReportService = new();
    private readonly PdfReportService _pdfReportService = new();
    
    /// <summary>
    /// Current report project being processed
    /// </summary>
    public ReportProject? CurrentProject { get; private set; }
    
    /// <summary>
    /// Event raised when processing status changes
    /// </summary>
    public event Action<string>? StatusChanged;
    
    /// <summary>
    /// Event raised when processing progress changes
    /// </summary>
    public event Action<int, int>? ProgressChanged;
    
    /// <summary>
    /// Scan a folder and detect reels
    /// </summary>
    public async Task<List<CameraReel>> ScanFolderAsync(
        string folderPath, 
        CancellationToken cancellationToken = default)
    {
        ReportStatus("Scanning folder...");
        
        var reels = await _reelDetectionService.DetectReelsAsync(
            folderPath,
            new Progress<(int current, int total, string status)>(p =>
            {
                ReportProgress(p.current, p.total);
                ReportStatus(p.status);
            }),
            cancellationToken);
        
        // Create new project
        CurrentProject = new ReportProject
        {
            Reels = reels
        };
        
        ReportStatus($"Found {reels.Count} reel(s) with {CurrentProject.TotalClipCount} clips");
        
        return reels;
    }
    
    /// <summary>
    /// Add a pre-configured reel to the current project
    /// </summary>
    public void AddReel(CameraReel reel)
    {
        CurrentProject ??= new ReportProject();
        CurrentProject.Reels.Add(reel);
    }
    
    /// <summary>
    /// Generate reports for all reels in the current project
    /// </summary>
    public async Task<List<string>> GenerateReportsAsync(
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        if (CurrentProject == null || CurrentProject.Reels.Count == 0)
        {
            throw new InvalidOperationException("No reels loaded. Call ScanFolderAsync first.");
        }
        
        CurrentProject.Settings = settings;
        var outputPaths = new List<string>();
        var stopwatch = Stopwatch.StartNew();
        
        // Ensure output folder exists
        Directory.CreateDirectory(settings.OutputFolder);
        
        try
        {
            // Generate ONE unified project report containing all reels
            ReportStatus($"Generating project report...");
            ReportProgress(1, 2);
            
            // Generate HTML
            if (settings.GenerateHtml)
            {
                var htmlPath = await _htmlReportService.GenerateAndSaveProjectReportAsync(
                    CurrentProject.Reels, settings, cancellationToken);
                outputPaths.Add(htmlPath);
                CurrentProject.HtmlReportPath = htmlPath;
            }
            
            ReportProgress(2, 2);
            
            // Generate PDF
            if (settings.GeneratePdf)
            {
                var pdfPath = await _pdfReportService.GenerateAndSaveProjectReportAsync(
                    CurrentProject.Reels, settings, cancellationToken);
                outputPaths.Add(pdfPath);
                CurrentProject.PdfReportPath = pdfPath;
            }
            
            CurrentProject.CompletedAt = DateTime.Now;
            stopwatch.Stop();
            
            ReportStatus($"Report generated in {stopwatch.Elapsed.TotalSeconds:0.0}s");
            Log.Information("Report generation completed in {ElapsedMs}ms", stopwatch.ElapsedMilliseconds);
            
            // Open report if requested
            if (settings.OpenReportWhenDone && outputPaths.Count > 0)
            {
                var pathToOpen = outputPaths.FirstOrDefault(p => p.EndsWith(".html")) ?? outputPaths[0];
                OpenFile(pathToOpen);
            }
            
            return outputPaths;
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Report generation failed");
            ReportStatus($"Error: {ex.Message}");
            throw;
        }
    }
    
    /// <summary>
    /// Clear the current project
    /// </summary>
    public void ClearProject()
    {
        CurrentProject = null;
        ReportStatus("Ready");
        ReportProgress(0, 100);
    }
    
    private void ReportStatus(string status)
    {
        StatusChanged?.Invoke(status);
    }
    
    private void ReportProgress(int current, int total)
    {
        ProgressChanged?.Invoke(current, total);
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
    
    public void Dispose()
    {
        _reelDetectionService.Dispose();
    }
}
