namespace LunaApp.Models;

/// <summary>Coarse phases of the end-to-end "scan → generate" workflow.</summary>
public enum ProcessingPhase
{
    Idle,
    Scanning,           // walking the folder tree
    Extracting,         // per-clip metadata + thumbnails
    Grouping,           // clustering clips into reels
    GeneratingHtml,
    GeneratingPdf,
    Finalizing,
}

/// <summary>
/// A single tick of progress published from services to the view-model.
/// Captures the current phase, progress within the phase, and (optionally) the
/// item being worked on. The view-model layers timing / ETA on top.
/// </summary>
public readonly record struct ProcessingReport(
    ProcessingPhase Phase,
    int Current,
    int Total,
    string? CurrentItem = null)
{
    /// <summary>Progress within the current phase, 0–100.</summary>
    public int Percent => Total > 0 ? Math.Clamp((int)(Current / (double)Total * 100), 0, 100) : 0;

    public string PhaseLabel => Phase switch
    {
        ProcessingPhase.Idle           => "Idle",
        ProcessingPhase.Scanning       => "Scanning folder",
        ProcessingPhase.Extracting     => "Extracting metadata",
        ProcessingPhase.Grouping       => "Grouping clips into reels",
        ProcessingPhase.GeneratingHtml => "Generating HTML report",
        ProcessingPhase.GeneratingPdf  => "Generating PDF report",
        ProcessingPhase.Finalizing     => "Finalizing",
        _ => Phase.ToString(),
    };

    public static ProcessingReport Idle => new(ProcessingPhase.Idle, 0, 0);
}
