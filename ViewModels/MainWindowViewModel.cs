using System.Collections.ObjectModel;
using System.Collections.Specialized;
using Avalonia.Platform.Storage;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using LunaApp.Services;
using Serilog;

namespace LunaApp.ViewModels;

public partial class MainWindowViewModel : ViewModelBase
{
    private readonly ReportGenerationService _reportService = new();
    private readonly AppSettings _appSettings;
    
    [ObservableProperty]
    private ObservableCollection<CameraReel> _reels = [];
    
    [ObservableProperty]
    private string _statusText = "Ready - Drop camera footage to begin";
    
    [ObservableProperty]
    private int _progress;
    
    [ObservableProperty]
    private bool _isProcessing;
    
    [ObservableProperty]
    private bool _generateHtml = true;
    
    [ObservableProperty]
    private bool _generatePdf = true;
    
    [ObservableProperty]
    private bool _openWhenDone = true;
    
    // DEV: Log viewer toggle state
    [ObservableProperty]
    private bool _isLogViewerVisible;
    
    // DEV: Combined log text for multi-line selection
    [ObservableProperty]
    private string _logText = string.Empty;
    
    // DEV: Reference to log entries for binding
    public ObservableCollection<LogEntry> LogEntries => InMemoryLogSink.Instance.LogEntries;
    
    public bool HasReels => Reels.Count > 0;
    public int ReelCount => Reels.Count;
    public int TotalClipCount => Reels.Sum(r => r.ClipCount);
    public TimeSpan TotalDuration => TimeSpan.FromTicks(Reels.Sum(r => r.TotalDuration.Ticks));
    public bool CanGenerate => HasReels && !IsProcessing;
    public string GenerateButtonText => IsProcessing ? "Generating..." : "Generate Reports";
    
    // Store provider for file dialogs
    public IStorageProvider? StorageProvider { get; set; }

    public MainWindowViewModel()
    {
        _appSettings = AppSettings.Load();
        
        // Restore settings
        GenerateHtml = _appSettings.DefaultReportSettings.GenerateHtml;
        GeneratePdf = _appSettings.DefaultReportSettings.GeneratePdf;
        OpenWhenDone = _appSettings.DefaultReportSettings.OpenReportWhenDone;
        
        // Subscribe to service events
        _reportService.StatusChanged += status => StatusText = status;
        _reportService.ProgressChanged += (current, total) => 
        {
            Progress = total > 0 ? (int)(current / (double)total * 100) : 0;
        };
        
        // DEV: Subscribe to log collection changes to update combined text
        LogEntries.CollectionChanged += OnLogEntriesChanged;
        
        // FFmpeg will auto-download on first use
        StatusText = "Ready - Drop camera footage to begin";
    }
    
    // DEV: Update combined log text when collection changes
    private void OnLogEntriesChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        LogText = string.Join(Environment.NewLine, LogEntries.Select(l => l.FormattedMessage));
    }
    
    partial void OnReelsChanged(ObservableCollection<CameraReel> value)
    {
        OnPropertyChanged(nameof(HasReels));
        OnPropertyChanged(nameof(ReelCount));
        OnPropertyChanged(nameof(TotalClipCount));
        OnPropertyChanged(nameof(TotalDuration));
        OnPropertyChanged(nameof(CanGenerate));
    }
    
    partial void OnIsProcessingChanged(bool value)
    {
        OnPropertyChanged(nameof(CanGenerate));
        OnPropertyChanged(nameof(GenerateButtonText));
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
            var folder = folders[0];
            await LoadFolderAsync(folder.Path.LocalPath);
        }
    }
    
    public async Task LoadFolderAsync(string folderPath)
    {
        if (IsProcessing) return;
        
        try
        {
            IsProcessing = true;
            StatusText = $"Scanning {Path.GetFileName(folderPath)}...";
            Progress = 0;
            
            var reels = await _reportService.ScanFolderAsync(folderPath);
            
            Reels = new ObservableCollection<CameraReel>(reels);
            
            // Update app settings with recent source
            _appSettings.AddRecentSource(folderPath);
            _appSettings.Save();
            
            StatusText = $"Found {ReelCount} reel(s) with {TotalClipCount} clips";
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to scan folder: {Path}", folderPath);
            StatusText = $"Error: {ex.Message}";
        }
        finally
        {
            IsProcessing = false;
            Progress = 0;
        }
    }
    
    [RelayCommand]
    private async Task GenerateReportsAsync()
    {
        if (!HasReels || IsProcessing) return;
        
        try
        {
            IsProcessing = true;
            Progress = 0;
            
            // Build settings from current state
            var settings = new ReportSettings
            {
                ProjectName = _appSettings.DefaultReportSettings.ProjectName,
                ProductionCompany = _appSettings.DefaultReportSettings.ProductionCompany,
                DitName = _appSettings.DefaultReportSettings.DitName,
                LogoPath = _appSettings.DefaultReportSettings.LogoPath,
                LogoBase64 = _appSettings.DefaultReportSettings.LogoBase64,
                OutputFolder = _appSettings.DefaultReportSettings.OutputFolder,
                ThumbnailsPerClip = _appSettings.DefaultReportSettings.ThumbnailsPerClip,
                ThumbnailWidth = _appSettings.DefaultReportSettings.ThumbnailWidth,
                GenerateHtml = GenerateHtml,
                GeneratePdf = GeneratePdf,
                OpenReportWhenDone = OpenWhenDone
            };
            
            var outputPaths = await _reportService.GenerateReportsAsync(settings);
            
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
            IsProcessing = false;
            Progress = 0;
        }
    }
    
    [RelayCommand]
    private void Clear()
    {
        Reels.Clear();
        _reportService.ClearProject();
        OnPropertyChanged(nameof(HasReels));
        OnPropertyChanged(nameof(ReelCount));
        OnPropertyChanged(nameof(TotalClipCount));
        OnPropertyChanged(nameof(TotalDuration));
        OnPropertyChanged(nameof(CanGenerate));
        StatusText = "Ready - Drop camera footage to begin";
        Progress = 0;
    }
    
    /// <summary>
    /// Event raised when settings window should be opened.
    /// Subscribe in MainWindow to show the dialog.
    /// </summary>
    public event Action? OpenSettingsRequested;
    
    [RelayCommand]
    private void OpenSettings()
    {
        OpenSettingsRequested?.Invoke();
    }
    
    // DEV: Toggle log viewer visibility
    [RelayCommand]
    private void ToggleLogViewer()
    {
        IsLogViewerVisible = !IsLogViewerVisible;
        Log.Debug("Log viewer toggled: {IsVisible}", IsLogViewerVisible);
    }
    
    // DEV: Clear all log entries
    [RelayCommand]
    private void ClearLogs()
    {
        InMemoryLogSink.Instance.Clear();
    }
    
    /// <summary>
    /// Reload settings after they've been changed
    /// </summary>
    public void ReloadSettings()
    {
        var settings = AppSettings.Load();
        _appSettings.DefaultReportSettings.ProjectName = settings.DefaultReportSettings.ProjectName;
        _appSettings.DefaultReportSettings.ProductionCompany = settings.DefaultReportSettings.ProductionCompany;
        _appSettings.DefaultReportSettings.DitName = settings.DefaultReportSettings.DitName;
        _appSettings.DefaultReportSettings.LogoPath = settings.DefaultReportSettings.LogoPath;
        _appSettings.DefaultReportSettings.LogoBase64 = settings.DefaultReportSettings.LogoBase64;
        _appSettings.DefaultReportSettings.OutputFolder = settings.DefaultReportSettings.OutputFolder;
        _appSettings.DefaultReportSettings.ThumbnailsPerClip = settings.DefaultReportSettings.ThumbnailsPerClip;
        
        GenerateHtml = settings.DefaultReportSettings.GenerateHtml;
        GeneratePdf = settings.DefaultReportSettings.GeneratePdf;
        OpenWhenDone = settings.DefaultReportSettings.OpenReportWhenDone;
    }
}

