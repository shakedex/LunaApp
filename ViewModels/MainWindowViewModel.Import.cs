using System.Collections.ObjectModel;
using Avalonia.Platform.Storage;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Import + generation workflow concerns of <see cref="MainWindowViewModel"/>:
/// folder picking, pre-scan confirmation, processing, report generation, clear.
/// </summary>
public partial class MainWindowViewModel
{
    // Pre-scan state: folder selected but not yet processed
    [ObservableProperty] private string? _pendingFolderPath;
    [ObservableProperty] private int _pendingClipCount;
    [ObservableProperty] private string _pendingFolderName = string.Empty;

    // Cancellation: non-null only while a long-running operation is in flight.
    // CancelProcessing cancels whichever op is active (scan / process / generate).
    private CancellationTokenSource? _currentOperationCts;

    public bool HasPendingFolder => !string.IsNullOrEmpty(PendingFolderPath);
    public bool ShowDropZone => !HasReels && !HasPendingFolder && !IsProcessing;
    public bool ShowPendingConfirmation => HasPendingFolder && !IsProcessing;
    public bool CanCancel => IsProcessing && _currentOperationCts is { IsCancellationRequested: false };

    partial void OnPendingFolderPathChanged(string? value)
    {
        OnPropertyChanged(nameof(HasPendingFolder));
        OnPropertyChanged(nameof(ShowDropZone));
        OnPropertyChanged(nameof(ShowPendingConfirmation));
    }

    [RelayCommand]
    private async Task BrowseFolderAsync()
    {
        if (StorageProvider == null) return;

        var folders = await StorageProvider.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = "Select Camera Footage Folder",
            AllowMultiple = false
        });

        if (folders.Count > 0)
        {
            await QuickScanFolderAsync(folders[0].Path.LocalPath);
        }
    }

    /// <summary>Quick scan to count files without processing (called on folder drop/selection).</summary>
    public async Task QuickScanFolderAsync(string folderPath)
    {
        if (IsProcessing) return;

        try
        {
            StatusText = $"Counting files in {Path.GetFileName(folderPath)}...";
            var count = await _reportService.CountMediaFilesAsync(folderPath);

            if (count == 0)
            {
                StatusText = "No video files found in this folder";
                PendingFolderPath = null;
                PendingClipCount = 0;
                PendingFolderName = string.Empty;
                return;
            }

            PendingFolderPath = folderPath;
            PendingClipCount = count;

            // DirectoryInfo.Name handles trailing separators ("D:/foo/bar/" → "bar"),
            // which Path.GetFileName returns as empty for. Drag-drop on Windows can
            // hand us either form, so use the resilient one.
            var folderName = SafeFolderName(folderPath);
            PendingFolderName = folderName;

            // Auto-fill the per-run Report Name with the folder we're about
            // to process — but only when the user hasn't typed something
            // custom. We track the last value we auto-filled so we can tell
            // "still untouched" from "user edited it".
            if (!string.IsNullOrEmpty(folderName) &&
                (string.IsNullOrWhiteSpace(ReportName) || ReportName == _autoFilledReportName))
            {
                ReportName = folderName;
                _autoFilledReportName = folderName;
            }

            StatusText = $"Found {count} video clip(s) ready to process";
        }
        catch (OperationCanceledException)
        {
            StatusText = "Scan cancelled";
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to scan folder: {Path}", folderPath);
            StatusText = $"Error: {ex.Message}";
        }
    }

    /// <summary>Called by drag/drop — delegates to <see cref="QuickScanFolderAsync"/>.</summary>
    public Task LoadFolderAsync(string folderPath) => QuickScanFolderAsync(folderPath);

    /// <summary>
    /// Robust folder-name extraction. <c>Path.GetFileName</c> returns empty
    /// for paths with a trailing separator (which drag-drop frequently
    /// hands us). <c>DirectoryInfo.Name</c> handles both shapes.
    /// </summary>
    private static string SafeFolderName(string folderPath)
    {
        try { return new DirectoryInfo(folderPath).Name; }
        catch { return string.Empty; }
    }

    /// <summary>Start full processing after user confirmation.</summary>
    [RelayCommand]
    private async Task StartProcessingAsync()
    {
        if (string.IsNullOrEmpty(PendingFolderPath) || IsProcessing) return;

        var folderPath = PendingFolderPath;
        using var cts = BeginOperation();

        try
        {
            IsProcessing = true;
            StatusText = $"Processing {PendingFolderName}...";
            Progress = 0;

            var reels = await _reportService.ScanFolderAsync(folderPath, cts.Token);
            Reels = new ObservableCollection<CameraReel>(reels);

            PendingFolderPath = null;
            PendingClipCount = 0;
            PendingFolderName = string.Empty;

            _appSettings.AddRecentSource(folderPath);
            _appSettings.Save();

            StatusText = $"Found {ReelCount} reel(s) with {TotalClipCount} clips";
        }
        catch (OperationCanceledException)
        {
            StatusText = "Processing cancelled";
            PendingFolderPath = null;
            PendingClipCount = 0;
            PendingFolderName = string.Empty;
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to process folder: {Path}", folderPath);
            StatusText = $"Error: {ex.Message}";
        }
        finally
        {
            EndOperation();
            Progress = 0;
        }
    }

    [RelayCommand]
    private void CancelPending()
    {
        PendingFolderPath = null;
        PendingClipCount = 0;
        PendingFolderName = string.Empty;
        StatusText = "Ready - Drop camera footage to begin";
    }

    /// <summary>
    /// Cancels the active scan / processing / report generation. Safe to call
    /// from the UI thread mid-operation.
    /// </summary>
    [RelayCommand]
    private void CancelProcessing()
    {
        if (_currentOperationCts is { IsCancellationRequested: false } cts)
        {
            Log.Information("User requested cancellation of current operation");
            StatusText = "Cancelling...";
            cts.Cancel();
            OnPropertyChanged(nameof(CanCancel));
        }
    }

    private CancellationTokenSource BeginOperation()
    {
        _currentOperationCts = new CancellationTokenSource();
        OnPropertyChanged(nameof(CanCancel));
        return _currentOperationCts;
    }

    private void EndOperation()
    {
        _currentOperationCts = null;
        IsProcessing = false;
        OnPropertyChanged(nameof(CanCancel));
    }

    [RelayCommand]
    private async Task GenerateReportsAsync()
    {
        if (!HasReels || IsProcessing) return;

        using var cts = BeginOperation();

        try
        {
            IsProcessing = true;
            Progress = 0;

            // Use the stored defaults as the source of truth — reconstructing
            // a fresh ReportSettings here silently drops any field we forget
            // to copy (previously: Theme, Director, Dp, GroupPdfsInSeparateFolder).
            // The three sidebar toggles override the saved defaults for this run only.
            var defaults = _appSettings.DefaultReportSettings;
            var settings = new ReportSettings
            {
                ProjectName               = defaults.ProjectName,
                ProductionCompany         = defaults.ProductionCompany,
                DitName                   = defaults.DitName,
                Director                  = defaults.Director,
                Dp                        = defaults.Dp,
                LogoPath                  = defaults.LogoPath,
                LogoBase64                = defaults.LogoBase64,
                OutputFolder              = defaults.OutputFolder,
                ThumbnailsPerClip         = defaults.ThumbnailsPerClip,
                ThumbnailWidth            = defaults.ThumbnailWidth,
                GroupPdfsInSeparateFolder = defaults.GroupPdfsInSeparateFolder,
                SaveReportToSource        = defaults.SaveReportToSource,
                ReportNamePattern         = defaults.ReportNamePattern,
                Theme                     = defaults.Theme,
                GenerateHtml              = GenerateHtml,
                GeneratePdf               = GeneratePdf,
                OpenReportWhenDone        = OpenWhenDone,

                // Per-run Report Name from the sidebar TextBox. Empty string
                // means "no report name" — the title falls back to just the
                // project name, and the folder skips the {ReportName} segment.
                ReportName                = string.IsNullOrWhiteSpace(ReportName) ? null : ReportName.Trim(),
            };

            await _reportService.GenerateReportsAsync(settings, cts.Token);
            StatusText = $"Reports saved to {settings.OutputFolder}";
        }
        catch (OperationCanceledException)
        {
            StatusText = "Generation cancelled";
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Report generation failed");
            StatusText = $"Error: {ex.Message}";
        }
        finally
        {
            EndOperation();
            Progress = 0;
        }
    }

    [RelayCommand]
    private void Clear()
    {
        Reels.Clear();
        _reportService.ClearProject();

        PendingFolderPath = null;
        PendingClipCount = 0;
        PendingFolderName = string.Empty;

        OnPropertyChanged(nameof(HasReels));
        OnPropertyChanged(nameof(ReelCount));
        OnPropertyChanged(nameof(TotalClipCount));
        OnPropertyChanged(nameof(TotalDuration));
        OnPropertyChanged(nameof(CanGenerate));
        OnPropertyChanged(nameof(ShowDropZone));
        OnPropertyChanged(nameof(ShowPendingConfirmation));

        StatusText = "Ready - Drop camera footage to begin";
        Progress = 0;
    }
}
