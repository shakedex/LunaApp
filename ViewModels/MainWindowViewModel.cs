using System.Collections.ObjectModel;
using System.Diagnostics;
using Avalonia.Platform.Storage;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using LunaApp.Services;

namespace LunaApp.ViewModels;

/// <summary>
/// The primary shell view-model. The file is split via <c>partial class</c> for
/// readability — see <c>MainWindowViewModel.Import.cs</c> (scan/generate workflow),
/// <c>MainWindowViewModel.Update.cs</c> (auto-update banner) and
/// <c>MainWindowViewModel.LogViewer.cs</c> (developer log panel).
/// </summary>
public partial class MainWindowViewModel : ViewModelBase
{
    private readonly ReportGenerationService _reportService;
    private readonly UpdateService _updateService;
    private readonly AppSettings _appSettings;

    // Core / shell state
    [ObservableProperty] private ObservableCollection<CameraReel> _reels = [];
    [ObservableProperty] private string _statusText = "Ready - Drop camera footage to begin";
    [ObservableProperty] private int _progress;
    [ObservableProperty] private bool _isProcessing;

    // Search — filters the displayed reels by matching reel label or any clip filename.
    [ObservableProperty] private string _searchText = string.Empty;

    /// <summary>Reels after applying <see cref="SearchText"/>. Bound to the list UI.</summary>
    public ObservableCollection<CameraReel> FilteredReels { get; } = [];

    public bool IsSearchActive => !string.IsNullOrWhiteSpace(SearchText);
    public bool HasFilteredReels => FilteredReels.Count > 0;
    public string FilterSummary => IsSearchActive
        ? $"{FilteredReels.Count} of {Reels.Count} reels"
        : string.Empty;

    // Phased progress (populated from ReportGenerationService.ProgressReported)
    [ObservableProperty] private string _phaseLabel = string.Empty;
    [ObservableProperty] private string _phaseDetail = string.Empty;
    [ObservableProperty] private string _etaText = string.Empty;

    // Global progress across the whole scan→generate pipeline (0–100). Drives the
    // moon hero so it waxes continuously instead of resetting at each phase
    // boundary. The per-phase <see cref="Progress"/> still feeds the progress bar.
    [ObservableProperty] private int _overallProgress;

    // Drag-drop feedback: true while a drop is hovering the window
    [ObservableProperty] private bool _isDragOver;

    // Report output toggles (surfaced on the sidebar, live in core VM state)
    [ObservableProperty] private bool _generateHtml = true;
    [ObservableProperty] private bool _generatePdf = true;
    [ObservableProperty] private bool _openWhenDone = true;

    /// <summary>
    /// Per-run report label appended to the project name in the report
    /// header and to the output folder. Empty by default; auto-filled from
    /// the scanned folder name in <c>QuickScanFolderAsync</c>, but
    /// preserved if the user typed a custom value.
    /// </summary>
    [ObservableProperty] private string _reportName = string.Empty;

    /// <summary>
    /// Tracks the last auto-fill we wrote into <see cref="ReportName"/>, so
    /// we know not to overwrite a user-edited value on the next scan. When
    /// the user types something new, ReportName diverges from this and the
    /// auto-fill backs off.
    /// </summary>
    private string _autoFilledReportName = string.Empty;

    // ETA tracking: reset at the start of each phase, extrapolates remaining
    // time from average-item-duration so far.
    private ProcessingPhase _etaPhase = ProcessingPhase.Idle;
    private Stopwatch? _etaStopwatch;
    private int _etaBaseItem;

    // Minimum time each phase label is visible before letting the next one take
    // over — prevents the overlay from strobing through phases on tiny projects.
    private static readonly TimeSpan MinPhaseDwell = TimeSpan.FromMilliseconds(450);
    private DateTime _currentPhaseShownAt = DateTime.MinValue;
    private ProcessingReport? _pendingReport;
    private bool _dwellScheduled;

    // Computed aggregates
    public bool HasReels => Reels.Count > 0;
    public int ReelCount => Reels.Count;
    public int TotalClipCount => Reels.Sum(r => r.ClipCount);
    public TimeSpan TotalDuration => TimeSpan.FromTicks(Reels.Sum(r => r.TotalDuration.Ticks));
    public long TotalSizeBytes => Reels.Sum(r => r.TotalSizeBytes);
    public string TotalSizeFormatted => FormatBytes(TotalSizeBytes);
    public bool CanGenerate => HasReels && !IsProcessing;
    public string GenerateButtonText => IsProcessing ? "Generating..." : "Generate Reports";

    /// <summary>
    /// Total file count across the source folders the loaded reels came
    /// from. Includes clips plus any sidecars / metadata / LUTs the camera
    /// wrote alongside (Sony XML, ARRI .ale, audio, etc.). Surfaces in the
    /// summary panel; not used in the generated report.
    /// </summary>
    [ObservableProperty] private int _totalFilesCount;

    private static string FormatBytes(long bytes)
    {
        string[] units = ["B", "KB", "MB", "GB", "TB"];
        double size = bytes;
        var order = 0;
        while (size >= 1024 && order < units.Length - 1)
        {
            order++;
            size /= 1024;
        }
        return $"{size:0.##} {units[order]}";
    }

    // Store provider for file dialogs (set by code-behind after DataContext is assigned)
    public IStorageProvider? StorageProvider { get; set; }

    /// <summary>The owning window's clipboard, set by code-behind. Used by clip context menu actions.</summary>
    public Avalonia.Input.Platform.IClipboard? Clipboard { get; set; }

    /// <summary>
    /// Raised when the settings dialog should be opened. The window code-behind
    /// handles the dialog; this event keeps the VM free of view references.
    /// </summary>
    public event Action? OpenSettingsRequested;

    /// <summary>Raised when the credits / about dialog should be opened.</summary>
    public event Action? OpenCreditsRequested;

    public MainWindowViewModel(
        ReportGenerationService reportService,
        UpdateService updateService)
    {
        _reportService = reportService;
        _updateService = updateService;
        _appSettings = AppSettings.Load();

        GenerateHtml = _appSettings.DefaultReportSettings.GenerateHtml;
        GeneratePdf = _appSettings.DefaultReportSettings.GeneratePdf;
        OpenWhenDone = _appSettings.DefaultReportSettings.OpenReportWhenDone;

        _reportService.ProgressReported += OnProgressReported;

        LogEntries.CollectionChanged += OnLogEntriesChanged;
        SubscribeToUpdateService();

        StatusText = "Ready - Drop camera footage to begin";
    }

    partial void OnReelsChanged(ObservableCollection<CameraReel> value)
    {
        OnPropertyChanged(nameof(HasReels));
        OnPropertyChanged(nameof(ReelCount));
        OnPropertyChanged(nameof(TotalClipCount));
        OnPropertyChanged(nameof(TotalDuration));
        OnPropertyChanged(nameof(TotalSizeBytes));
        OnPropertyChanged(nameof(TotalSizeFormatted));
        OnPropertyChanged(nameof(CanGenerate));
        OnPropertyChanged(nameof(ShowDropZone));
        OnPropertyChanged(nameof(ShowPendingConfirmation));
        RecomputeTotalFilesCount();
        RebuildFilteredReels();
    }

    /// <summary>
    /// Walks the unique source folders the loaded reels were detected in
    /// and counts every file beneath — clips, Sony NRT XML sidecars,
    /// ARRI .ale, audio, LUTs, anything. One enumeration per scan; the
    /// result feeds the summary panel's "Files" row.
    /// </summary>
    private void RecomputeTotalFilesCount()
    {
        if (Reels.Count == 0) { TotalFilesCount = 0; return; }

        var paths = Reels.Select(r => r.SourcePath)
                         .Where(p => !string.IsNullOrEmpty(p))
                         .Distinct(StringComparer.OrdinalIgnoreCase);

        var total = 0;
        foreach (var path in paths)
        {
            try
            {
                if (Directory.Exists(path))
                    total += Directory.EnumerateFiles(path, "*", SearchOption.AllDirectories).Count();
            }
            catch
            {
                // Permissions / IO hiccups shouldn't break the summary —
                // we just skip and count what we can.
            }
        }
        TotalFilesCount = total;
    }

    partial void OnSearchTextChanged(string value)
    {
        RebuildFilteredReels();
        OnPropertyChanged(nameof(IsSearchActive));
    }

    private void RebuildFilteredReels()
    {
        FilteredReels.Clear();

        var needle = SearchText?.Trim();
        var hasQuery = !string.IsNullOrEmpty(needle);

        foreach (var reel in Reels)
        {
            if (!hasQuery)
            {
                FilteredReels.Add(reel);
                continue;
            }

            // Match reel label OR any clip filename (case-insensitive)
            if (reel.DisplayLabel.Contains(needle!, StringComparison.OrdinalIgnoreCase) ||
                reel.Clips.Any(c => c.FileName.Contains(needle!, StringComparison.OrdinalIgnoreCase)))
            {
                FilteredReels.Add(reel);
            }
        }

        OnPropertyChanged(nameof(HasFilteredReels));
        OnPropertyChanged(nameof(FilterSummary));
    }

    [RelayCommand]
    private void ClearSearch() => SearchText = string.Empty;

    partial void OnIsProcessingChanged(bool value)
    {
        OnPropertyChanged(nameof(CanGenerate));
        OnPropertyChanged(nameof(GenerateButtonText));
        OnPropertyChanged(nameof(ShowDropZone));
        OnPropertyChanged(nameof(ShowPendingConfirmation));
        OnPropertyChanged(nameof(CanCancel));
    }

    [RelayCommand]
    private void OpenSettings()
    {
        OpenSettingsRequested?.Invoke();
    }

    [RelayCommand]
    private void OpenCredits()
    {
        OpenCreditsRequested?.Invoke();
    }

    /// <summary>
    /// Reload the default report settings from disk (called after the settings
    /// dialog saves). Copies every field exhaustively — forgetting one here
    /// would silently drop the user's preference on the next generate run.
    /// </summary>
    public void ReloadSettings()
    {
        var loaded = AppSettings.Load().DefaultReportSettings;
        var defaults = _appSettings.DefaultReportSettings;

        defaults.ProjectName               = loaded.ProjectName;
        defaults.ProductionCompany         = loaded.ProductionCompany;
        defaults.DitName                   = loaded.DitName;
        defaults.Director                  = loaded.Director;
        defaults.Dp                        = loaded.Dp;
        defaults.LogoPath                  = loaded.LogoPath;
        defaults.LogoBase64                = loaded.LogoBase64;
        defaults.OutputFolder              = loaded.OutputFolder;
        defaults.ThumbnailsPerClip         = loaded.ThumbnailsPerClip;
        defaults.ThumbnailWidth            = loaded.ThumbnailWidth;
        defaults.GroupPdfsInSeparateFolder = loaded.GroupPdfsInSeparateFolder;
        defaults.SaveReportToSource        = loaded.SaveReportToSource;
        defaults.ReportNamePattern         = loaded.ReportNamePattern;
        defaults.Theme                     = loaded.Theme;

        GenerateHtml  = loaded.GenerateHtml;
        GeneratePdf   = loaded.GeneratePdf;
        OpenWhenDone  = loaded.OpenReportWhenDone;
    }

    /// <summary>
    /// Consumes <see cref="ProcessingReport"/> ticks from the service layer and
    /// updates the phase label, per-item detail, percent, and ETA.
    ///
    /// Phase transitions are gated by <see cref="MinPhaseDwell"/> so that on
    /// tiny projects we don't strobe through five phases in 80 ms. When a new
    /// phase arrives before the previous one has been visible long enough,
    /// it's held in <c>_pendingReport</c> and applied after a short delay;
    /// later reports for the same phase overwrite the pending one so we never
    /// queue up stale updates.
    /// </summary>
    private void OnProgressReported(ProcessingReport report)
    {
        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            // Same phase → update in place, no dwell needed
            if (report.Phase == _etaPhase)
            {
                ApplyReport(report);
                return;
            }

            var elapsed = DateTime.UtcNow - _currentPhaseShownAt;
            if (elapsed >= MinPhaseDwell)
            {
                ApplyReport(report);
                return;
            }

            // Too soon — defer so the previous phase is visible long enough
            _pendingReport = report;
            if (_dwellScheduled) return;
            _dwellScheduled = true;

            var remaining = MinPhaseDwell - elapsed;
            _ = Task.Delay(remaining).ContinueWith(_ =>
            {
                Avalonia.Threading.Dispatcher.UIThread.Post(() =>
                {
                    _dwellScheduled = false;
                    if (_pendingReport is { } queued)
                    {
                        _pendingReport = null;
                        ApplyReport(queued);
                    }
                });
            });
        });
    }

    private void ApplyReport(ProcessingReport report)
    {
        if (report.Phase != _etaPhase)
        {
            _etaPhase = report.Phase;
            _etaStopwatch = Stopwatch.StartNew();
            _etaBaseItem = report.Current;
            _currentPhaseShownAt = DateTime.UtcNow;
        }

        Progress = report.Percent;
        OverallProgress = ComputeOverallProgress(report.Phase, report.Percent);
        PhaseLabel = report.PhaseLabel;
        PhaseDetail = BuildDetail(report);
        EtaText = BuildEta(report);

        StatusText = string.IsNullOrEmpty(report.CurrentItem)
            ? report.PhaseLabel
            : $"{report.PhaseLabel}: {report.CurrentItem}";
    }

    // Maps a per-phase (Phase, Percent) tick to a 0–100 ramp within its
    // pipeline group. Two independent groups, each gets its own moon wax:
    //   • scan/parse:  Scanning → Extracting → Grouping  (ends at summary)
    //   • generation:  GeneratingHtml → GeneratingPdf → Finalizing
    private static int ComputeOverallProgress(ProcessingPhase phase, int phasePercent)
    {
        const int groupSize = 3;
        var idx = phase switch
        {
            ProcessingPhase.Scanning       => 0,
            ProcessingPhase.Extracting     => 1,
            ProcessingPhase.Grouping       => 2,
            ProcessingPhase.GeneratingHtml => 0,
            ProcessingPhase.GeneratingPdf  => 1,
            ProcessingPhase.Finalizing     => 2,
            _ => -1,
        };
        if (idx < 0) return 0;
        return (int)((idx * 100.0 + Math.Clamp(phasePercent, 0, 100)) / groupSize);
    }

    private static string BuildDetail(ProcessingReport report)
    {
        if (report.Total <= 1) return report.CurrentItem ?? string.Empty;
        var item = string.IsNullOrEmpty(report.CurrentItem) ? string.Empty : $"  •  {report.CurrentItem}";
        return $"{report.Current} of {report.Total}{item}";
    }

    private string BuildEta(ProcessingReport report)
    {
        if (_etaStopwatch == null || report.Total <= 1) return string.Empty;
        var itemsDone = report.Current - _etaBaseItem;
        if (itemsDone <= 0) return string.Empty;

        var remaining = report.Total - report.Current;
        if (remaining <= 0) return string.Empty;

        var msPerItem = _etaStopwatch.Elapsed.TotalMilliseconds / itemsDone;
        var eta = TimeSpan.FromMilliseconds(msPerItem * remaining);

        return eta.TotalSeconds switch
        {
            < 1      => string.Empty,
            < 60     => $"~{eta.Seconds}s remaining",
            < 3600   => $"~{(int)eta.TotalMinutes}m {eta.Seconds}s remaining",
            _        => $"~{(int)eta.TotalHours}h {eta.Minutes}m remaining",
        };
    }
}
