using Avalonia.Platform.Storage;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using Serilog;

namespace LunaApp.ViewModels;

public partial class SettingsViewModel : ViewModelBase
{
    private readonly AppSettings _appSettings;
    
    // Branding
    [ObservableProperty]
    private string? _projectName;
    
    [ObservableProperty]
    private string? _productionCompany;
    
    [ObservableProperty]
    private string? _ditName;
    
    [ObservableProperty]
    private string? _director;
    
    [ObservableProperty]
    private string? _dp;
    
    [ObservableProperty]
    private string? _logoPath;
    
    [ObservableProperty]
    private bool _hasLogo;
    
    // Output settings
    [ObservableProperty]
    private string _outputFolder = string.Empty;
    
    [ObservableProperty]
    private int _thumbnailsPerClip = 3;
    
    [ObservableProperty]
    private bool _generateHtmlByDefault = true;
    
    [ObservableProperty]
    private bool _generatePdfByDefault = true;
    
    [ObservableProperty]
    private bool _openReportWhenDone = true;
    
    [ObservableProperty]
    private bool _groupPdfsInSeparateFolder;
    
    // Update settings
    [ObservableProperty]
    private bool _isCheckingForUpdates;
    
    [ObservableProperty]
    private string _updateStatusText = "Click to check for updates";
    
    public string CurrentVersionText => $"Current version: {App.UpdateService?.CurrentVersion ?? "Development"}";
    public string CheckForUpdatesButtonText => IsCheckingForUpdates ? "Checking..." : "Check for Updates";
    
    partial void OnIsCheckingForUpdatesChanged(bool value)
    {
        OnPropertyChanged(nameof(CheckForUpdatesButtonText));
    }
    
    // Store provider for file dialogs
    public IStorageProvider? StorageProvider { get; set; }
    
    public SettingsViewModel()
    {
        _appSettings = AppSettings.Load();
        LoadSettings();
    }
    
    private void LoadSettings()
    {
        var settings = _appSettings.DefaultReportSettings;
        
        ProjectName = settings.ProjectName;
        ProductionCompany = settings.ProductionCompany;
        DitName = settings.DitName;
        Director = settings.Director;
        Dp = settings.Dp;
        LogoPath = settings.LogoPath;
        HasLogo = settings.HasLogo;
        OutputFolder = settings.OutputFolder;
        ThumbnailsPerClip = settings.ThumbnailsPerClip;
        GenerateHtmlByDefault = settings.GenerateHtml;
        GeneratePdfByDefault = settings.GeneratePdf;
        OpenReportWhenDone = settings.OpenReportWhenDone;
        GroupPdfsInSeparateFolder = settings.GroupPdfsInSeparateFolder;
    }
    
    [RelayCommand]
    private void Save()
    {
        var settings = _appSettings.DefaultReportSettings;
        
        settings.ProjectName = ProjectName;
        settings.ProductionCompany = ProductionCompany;
        settings.DitName = DitName;
        settings.Director = Director;
        settings.Dp = Dp;
        settings.LogoPath = LogoPath;
        settings.OutputFolder = OutputFolder;
        settings.ThumbnailsPerClip = ThumbnailsPerClip;
        settings.GenerateHtml = GenerateHtmlByDefault;
        settings.GeneratePdf = GeneratePdfByDefault;
        settings.OpenReportWhenDone = OpenReportWhenDone;
        settings.GroupPdfsInSeparateFolder = GroupPdfsInSeparateFolder;
        
        // Convert logo to base64 if path is set
        if (!string.IsNullOrEmpty(LogoPath) && File.Exists(LogoPath))
        {
            try
            {
                var logoBytes = File.ReadAllBytes(LogoPath);
                settings.LogoBase64 = Convert.ToBase64String(logoBytes);
            }
            catch
            {
                // Keep path-based reference if base64 fails
            }
        }
        
        _appSettings.Save();
    }
    
    [RelayCommand]
    private async Task BrowseLogoAsync()
    {
        if (StorageProvider == null) return;
        
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Select Logo Image",
            AllowMultiple = false,
            FileTypeFilter =
            [
                new FilePickerFileType("Images") { Patterns = ["*.png", "*.jpg", "*.jpeg", "*.webp", "*.svg"] }
            ]
        });
        
        if (files.Count > 0)
        {
            LogoPath = files[0].Path.LocalPath;
            HasLogo = true;
        }
    }
    
    [RelayCommand]
    private void ClearLogo()
    {
        LogoPath = null;
        HasLogo = false;
        _appSettings.DefaultReportSettings.LogoPath = null;
        _appSettings.DefaultReportSettings.LogoBase64 = null;
    }
    
    [RelayCommand]
    private async Task BrowseOutputFolderAsync()
    {
        if (StorageProvider == null) return;
        
        var folders = await StorageProvider.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = "Select Output Folder",
            AllowMultiple = false
        });
        
        if (folders.Count > 0)
        {
            OutputFolder = folders[0].Path.LocalPath;
        }
    }
    
    [RelayCommand]
    private async Task CheckForUpdatesAsync()
    {
        if (App.UpdateService == null)
        {
            UpdateStatusText = "Updates only available in installed version";
            return;
        }
        
        try
        {
            IsCheckingForUpdates = true;
            UpdateStatusText = "Checking for updates...";
            
            var hasUpdate = await App.UpdateService.CheckForUpdatesAsync();
            
            if (hasUpdate)
            {
                var version = App.UpdateService.PendingUpdate?.TargetFullRelease.Version;
                UpdateStatusText = $"Update available: v{version}";
            }
            else
            {
                UpdateStatusText = "You're using the latest version";
            }
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to check for updates");
            UpdateStatusText = "Failed to check for updates";
        }
        finally
        {
            IsCheckingForUpdates = false;
        }
    }
}
