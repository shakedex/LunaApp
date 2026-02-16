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
    
    // Pre-scan state: folder selected but not yet processed
    [ObservableProperty]
    private string? _pendingFolderPath;
    
    [ObservableProperty]
    private int _pendingClipCount;
    
    [ObservableProperty]
    private string _pendingFolderName = string.Empty;
    
    // Update system properties
    [ObservableProperty]
    private bool _hasUpdateAvailable;
    
    [ObservableProperty]
    private string _updateVersion = string.Empty;
    
    [ObservableProperty]
    private int _updateDownloadProgress;
    
    [ObservableProperty]
    private bool _isDownloadingUpdate;
    
    [ObservableProperty]
    private bool _isUpdateReady;
    
    // DEV: Reference to log entries for binding
    public ObservableCollection<LogEntry> LogEntries => InMemoryLogSink.Instance.LogEntries;
    
    public bool HasReels => Reels.Count > 0;
    public bool HasPendingFolder => !string.IsNullOrEmpty(PendingFolderPath);
    public bool ShowDropZone => !HasReels && !HasPendingFolder && !IsProcessing;
    public bool ShowPendingConfirmation => HasPendingFolder && !IsProcessing;
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
        
        // Subscribe to update service events (will be available after App initialization)
        SubscribeToUpdateService();
        
        // FFmpeg will auto-download on first use
        StatusText = "Ready - Drop camera footage to begin";
    }
    
    private void SubscribeToUpdateService()
    {
        // Delay subscription since UpdateService is initialized after ViewModel
        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            var updateService = App.UpdateService;
            if (updateService == null) return;
            
            updateService.UpdateAvailable += (_, info) =>
            {
                HasUpdateAvailable = true;
                UpdateVersion = info.TargetFullRelease.Version.ToString();
                IsUpdateReady = false;
                IsDownloadingUpdate = false;
            };
            
            updateService.DevUpdateAvailable += (_, version) =>
            {
                HasUpdateAvailable = true;
                UpdateVersion = version;
                IsUpdateReady = false;
                IsDownloadingUpdate = false;
            };
            
            updateService.DownloadProgress += (_, progress) =>
            {
                UpdateDownloadProgress = progress;
            };
            
            updateService.UpdateReady += (_, _) =>
            {
                IsDownloadingUpdate = false;
                IsUpdateReady = true;
            };
        });
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
        OnPropertyChanged(nameof(ShowDropZone));
        OnPropertyChanged(nameof(ShowPendingConfirmation));
    }
    
    partial void OnIsProcessingChanged(bool value)
    {
        OnPropertyChanged(nameof(CanGenerate));
        OnPropertyChanged(nameof(GenerateButtonText));
        OnPropertyChanged(nameof(ShowDropZone));
        OnPropertyChanged(nameof(ShowPendingConfirmation));
    }
    
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
            var folder = folders[0];
            await QuickScanFolderAsync(folder.Path.LocalPath);
        }
    }
    
    /// <summary>
    /// Quick scan to count files without processing (called on folder drop/selection)
    /// </summary>
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
            
            // Store for confirmation
            PendingFolderPath = folderPath;
            PendingClipCount = count;
            PendingFolderName = Path.GetFileName(folderPath);
            
            StatusText = $"Found {count} video clip(s) ready to process";
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to scan folder: {Path}", folderPath);
            StatusText = $"Error: {ex.Message}";
        }
    }
    
    /// <summary>
    /// Called by drag/drop - delegates to QuickScanFolderAsync
    /// </summary>
    public Task LoadFolderAsync(string folderPath) => QuickScanFolderAsync(folderPath);
    
    /// <summary>
    /// Start full processing after user confirmation
    /// </summary>
    [RelayCommand]
    private async Task StartProcessingAsync()
    {
        if (string.IsNullOrEmpty(PendingFolderPath) || IsProcessing) return;
        
        var folderPath = PendingFolderPath;
        
        try
        {
            IsProcessing = true;
            StatusText = $"Processing {PendingFolderName}...";
            Progress = 0;
            
            var reels = await _reportService.ScanFolderAsync(folderPath);
            
            Reels = new ObservableCollection<CameraReel>(reels);
            
            // Clear pending state
            PendingFolderPath = null;
            PendingClipCount = 0;
            PendingFolderName = string.Empty;
            
            // Update app settings with recent source
            _appSettings.AddRecentSource(folderPath);
            _appSettings.Save();
            
            StatusText = $"Found {ReelCount} reel(s) with {TotalClipCount} clips";
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to process folder: {Path}", folderPath);
            StatusText = $"Error: {ex.Message}";
        }
        finally
        {
            IsProcessing = false;
            Progress = 0;
        }
    }
    
    /// <summary>
    /// Cancel pending folder selection
    /// </summary>
    [RelayCommand]
    private void CancelPending()
    {
        PendingFolderPath = null;
        PendingClipCount = 0;
        PendingFolderName = string.Empty;
        StatusText = "Ready - Drop camera footage to begin";
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
        
        // Clear pending state too
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
    
    // ============ UPDATE COMMANDS ============
    
    [RelayCommand]
    private async Task DownloadUpdateAsync()
    {
        if (App.UpdateService == null) return;
        
        try
        {
            IsDownloadingUpdate = true;
            UpdateDownloadProgress = 0;
            await App.UpdateService.DownloadUpdateAsync();
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to download update");
            IsDownloadingUpdate = false;
        }
    }
    
    [RelayCommand]
    private void ApplyUpdate()
    {
        App.UpdateService?.ApplyUpdateAndRestart();
    }
    
    [RelayCommand]
    private void DismissUpdate()
    {
        HasUpdateAvailable = false;
        IsUpdateReady = false;
        IsDownloadingUpdate = false;
    }
    
    [RelayCommand]
    private async Task CheckForUpdatesAsync()
    {
        if (App.UpdateService == null) return;
        
        try
        {
            StatusText = "Checking for updates...";
            var hasUpdate = await App.UpdateService.CheckForUpdatesAsync();
            StatusText = hasUpdate ? $"Update available: v{UpdateVersion}" : "You're up to date!";
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to check for updates");
            StatusText = "Failed to check for updates";
        }
    }
    
    // ============ DEV TESTING COMMANDS ============
    
    [RelayCommand]
    private void DevShowUpdateBanner()
    {
        App.UpdateService?.DevSimulateUpdateAvailable("99.0.0");
    }
    
    [RelayCommand]
    private void DevSimulateDownload()
    {
        IsDownloadingUpdate = true;
        UpdateDownloadProgress = 0;
        
        // Simulate progress over time
        _ = Task.Run(async () =>
        {
            for (int i = 0; i <= 100; i += 10)
            {
                await Task.Delay(200);
                Avalonia.Threading.Dispatcher.UIThread.Post(() => UpdateDownloadProgress = i);
            }
            Avalonia.Threading.Dispatcher.UIThread.Post(() =>
            {
                IsDownloadingUpdate = false;
                IsUpdateReady = true;
            });
        });
    }
    
    [RelayCommand]
    private void DevResetUpdateState()
    {
        HasUpdateAvailable = false;
        IsDownloadingUpdate = false;
        IsUpdateReady = false;
        UpdateDownloadProgress = 0;
        UpdateVersion = string.Empty;
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

