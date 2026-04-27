using System.ComponentModel.DataAnnotations;
using Avalonia.Media.Imaging;
using Avalonia.Platform.Storage;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using LunaApp.Services;
using LunaApp.Services.CameraSupport;
using LunaApp.Services.Chappie;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Row in the Settings "Camera Support" list. Tracks status + an optional
/// install action (today only the ARRI row uses it — clicking triggers
/// <see cref="ArtCliInstaller"/>; future vendor rows can plug in the same
/// way). Updates observably so the row re-renders when install completes.
/// </summary>
public partial class CameraSupportRow : ObservableObject
{
    public string SupportId { get; init; } = "";
    public string DisplayName { get; init; } = "";
    public string? InstallButtonLabel { get; init; }
    public IAsyncRelayCommand? InstallCommand { get; init; }

    [ObservableProperty] private string _state = "";
    [ObservableProperty] private string _summary = "";
    [ObservableProperty] private bool _isInstalling;
    [ObservableProperty] private double _installProgress;
    [ObservableProperty] private string? _installError;

    public bool IsReady => State == "Ready";
    public bool CanInstall => InstallCommand is not null && !IsReady && !IsInstalling;
    public bool ShowInstallButton => InstallCommand is not null && !IsReady;

    partial void OnStateChanged(string value)
    {
        OnPropertyChanged(nameof(IsReady));
        OnPropertyChanged(nameof(CanInstall));
        OnPropertyChanged(nameof(ShowInstallButton));
    }

    partial void OnIsInstallingChanged(bool value) =>
        OnPropertyChanged(nameof(CanInstall));
}

/// <summary>
/// Settings dialog VM. Uses <see cref="ObservableValidator"/> for inline form
/// validation — errors render on the relevant input via <c>INotifyDataErrorInfo</c>,
/// but they never block Save: the command always fills in sensible fallbacks for
/// empty fields so first-time users (who haven't typed anything yet) can just
/// hit Save without wrestling the form.
/// </summary>
public partial class SettingsViewModel : ObservableValidator
{
    private readonly UpdateService _updateService;
    private readonly AppSettings _appSettings;

    private readonly ArtCliInstaller _artCliInstaller;
    private readonly ArtCliLocator _artCliLocator;
    private readonly CameraSupportRegistry _cameraSupports;

    public IReadOnlyList<CameraSupportRow> CameraSupports { get; }

    // Branding — length hints only; empty is fine, we fall back to "Camera Report" at render time.
    [ObservableProperty]
    [NotifyDataErrorInfo]
    [MaxLength(120, ErrorMessage = "Keep the project name under 120 characters.")]
    private string? _projectName;

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [MaxLength(120)]
    private string? _productionCompany;

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [MaxLength(80)]
    private string? _ditName;

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [MaxLength(80)]
    private string? _director;

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [MaxLength(80)]
    private string? _dp;

    [ObservableProperty]
    private string? _logoPath;

    [ObservableProperty]
    private bool _hasLogo;

    // Decoded bitmap for the in-dialog preview. Avalonia's Image.Source won't
    // render a bare file-path string — we need an IImage. Rebuilt whenever
    // LogoPath changes.
    [ObservableProperty]
    private Bitmap? _logoBitmap;

    partial void OnLogoPathChanged(string? value)
    {
        LogoBitmap?.Dispose();
        LogoBitmap = null;

        if (string.IsNullOrEmpty(value) || !File.Exists(value)) return;

        try
        {
            LogoBitmap = new Bitmap(value);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Could not decode logo preview from {Path}", value);
        }
    }

    // Output settings
    [ObservableProperty]
    private string _outputFolder = string.Empty;

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [Range(0, 10, ErrorMessage = "Pick between 0 and 10 thumbnails per clip.")]
    private int _thumbnailsPerClip = 3;

    [ObservableProperty] private bool _generateHtmlByDefault = true;
    [ObservableProperty] private bool _generatePdfByDefault = true;
    [ObservableProperty] private bool _openReportWhenDone = true;
    [ObservableProperty] private bool _groupPdfsInSeparateFolder;

    // Report theme — two mutually-exclusive bools that mirror each other.
    // We can't use `IsChecked="{Binding !X}"` for the Light radio because that
    // binding expression is one-way only in Avalonia compiled bindings, so
    // toggling Light would never write back to the VM.
    [ObservableProperty] private bool _isLightTheme = true;
    [ObservableProperty] private bool _isDarkTheme;

    // Update settings
    [ObservableProperty] private bool _isCheckingForUpdates;
    [ObservableProperty] private string _updateStatusText = "Click to check for updates";

    public string CurrentVersionText => $"Current version: {_updateService.CurrentVersion ?? "Development"}";
    public string CheckForUpdatesButtonText => IsCheckingForUpdates ? "Checking..." : "Check for Updates";

    /// <summary>True when at least one report format is enabled — gates "Open when complete".</summary>
    public bool WillGenerateAnyReport => GenerateHtmlByDefault || GeneratePdfByDefault;

    /// <summary>Raised when the Save command succeeded so the host window can close with a positive result.</summary>
    public event Action? SaveCompleted;

    partial void OnIsCheckingForUpdatesChanged(bool value) =>
        OnPropertyChanged(nameof(CheckForUpdatesButtonText));

    partial void OnGenerateHtmlByDefaultChanged(bool value) =>
        OnPropertyChanged(nameof(WillGenerateAnyReport));

    // Keep the two theme bools mutually exclusive. Guarded so we don't ping-pong.
    partial void OnIsLightThemeChanged(bool value)
    {
        if (value && IsDarkTheme) IsDarkTheme = false;
    }
    partial void OnIsDarkThemeChanged(bool value)
    {
        if (value && IsLightTheme) IsLightTheme = false;
    }

    partial void OnGeneratePdfByDefaultChanged(bool value)
    {
        OnPropertyChanged(nameof(WillGenerateAnyReport));
        if (!value)
            GroupPdfsInSeparateFolder = false;
    }

    public IStorageProvider? StorageProvider { get; set; }

    public SettingsViewModel(
        UpdateService updateService,
        CameraSupportRegistry cameraSupports,
        ArtCliInstaller artCliInstaller,
        ArtCliLocator artCliLocator)
    {
        _updateService = updateService;
        _artCliInstaller = artCliInstaller;
        _artCliLocator = artCliLocator;
        _cameraSupports = cameraSupports;
        _appSettings = AppSettings.Load();

        CameraSupports = cameraSupports.All.Select(BuildRow).ToArray();

        LoadSettings();
        // Run validation once so inline field errors (not Save gating) are accurate.
        ValidateAllProperties();
    }

    private CameraSupportRow BuildRow(ICameraSupport support)
    {
        var row = new CameraSupportRow
        {
            SupportId = support.Id,
            DisplayName = support.DisplayName,
            State = SupportStateLabel(support.Status),
            Summary = SupportSummary(support.Status),
            // ARRI is the only support with a managed installer today.
            // BRAW / Sony plug in here when their installer story lands.
            InstallButtonLabel = support.Id == "arri" ? BuildArtCliInstallLabel() : null,
            InstallCommand = support.Id == "arri" ? new AsyncRelayCommand(InstallArtCliAsync) : null,
        };
        return row;
    }

    private string BuildArtCliInstallLabel()
    {
        var release = _artCliInstaller.CurrentRelease;
        if (release is null) return "Install ARRI Reference Tool";
        var sizeMb = release.DownloadSizeBytes / (1024 * 1024);
        return $"Install ARRI Reference Tool ({sizeMb} MB)";
    }

    private async Task InstallArtCliAsync()
    {
        var row = CameraSupports.FirstOrDefault(r => r.SupportId == "arri");
        if (row is null) return;

        row.IsInstalling = true;
        row.InstallError = null;
        row.InstallProgress = 0;

        var progress = new Progress<double>(p => row.InstallProgress = p);
        try
        {
            var result = await _artCliInstaller.InstallAsync(progress);
            if (result.Success)
            {
                // Re-resolve via the locator and refresh the row's state.
                _artCliLocator.Invalidate();
                var arri = _cameraSupports.All.FirstOrDefault(s => s.Id == "arri");
                if (arri is not null)
                {
                    row.State = SupportStateLabel(arri.Status);
                    row.Summary = SupportSummary(arri.Status);
                }
            }
            else
            {
                row.InstallError = result.Error;
            }
        }
        finally
        {
            row.IsInstalling = false;
        }
    }

    private static string SupportStateLabel(SupportStatus status) => status switch
    {
        SupportStatus.Ready => "Ready",
        SupportStatus.ComingLater => "Coming later",
        _ => "Not available",
    };

    private static string SupportSummary(SupportStatus status) => status switch
    {
        SupportStatus.Ready r => $"{r.Version} ({r.Provenance})",
        SupportStatus.ComingLater cl => cl.RoadmapNote,
        SupportStatus.NotAvailable na => na.Reason,
        _ => "",
    };

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
        // If we loaded a previously-saved base64 logo but the original path is
        // gone, still show a preview from the embedded bytes.
        if (LogoBitmap is null && !string.IsNullOrEmpty(settings.LogoBase64))
        {
            try
            {
                var bytes = Convert.FromBase64String(settings.LogoBase64);
                using var stream = new MemoryStream(bytes);
                LogoBitmap = new Bitmap(stream);
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Could not decode saved logo bytes for preview");
            }
        }
        OutputFolder = settings.OutputFolder;
        ThumbnailsPerClip = settings.ThumbnailsPerClip;
        GenerateHtmlByDefault = settings.GenerateHtml;
        GeneratePdfByDefault = settings.GeneratePdf;
        OpenReportWhenDone = settings.OpenReportWhenDone;
        GroupPdfsInSeparateFolder = settings.GroupPdfsInSeparateFolder;
        IsDarkTheme = settings.Theme == ReportTheme.Dark;
        IsLightTheme = !IsDarkTheme;
    }

    /// <summary>
    /// Save is always enabled — if the form has minor issues (too-long field,
    /// out-of-range thumbnail count) we clamp/fallback instead of blocking. The
    /// user sees the inline error and can fix it, but typing nothing shouldn't
    /// soft-lock the dialog.
    /// </summary>
    [RelayCommand]
    private void Save()
    {
        var settings = _appSettings.DefaultReportSettings;

        settings.ProjectName = string.IsNullOrWhiteSpace(ProjectName) ? null : ProjectName.Trim();
        settings.ProductionCompany = string.IsNullOrWhiteSpace(ProductionCompany) ? null : ProductionCompany.Trim();
        settings.DitName = string.IsNullOrWhiteSpace(DitName) ? null : DitName.Trim();
        settings.Director = string.IsNullOrWhiteSpace(Director) ? null : Director.Trim();
        settings.Dp = string.IsNullOrWhiteSpace(Dp) ? null : Dp.Trim();
        settings.LogoPath = LogoPath;

        settings.OutputFolder = string.IsNullOrWhiteSpace(OutputFolder)
            ? _appSettings.DefaultReportSettings.OutputFolder
            : OutputFolder.Trim();

        settings.ThumbnailsPerClip = Math.Clamp(ThumbnailsPerClip, 0, 10);
        settings.GenerateHtml = GenerateHtmlByDefault;
        settings.GeneratePdf = GeneratePdfByDefault;
        settings.OpenReportWhenDone = OpenReportWhenDone;
        settings.GroupPdfsInSeparateFolder = GroupPdfsInSeparateFolder;
        settings.Theme = IsDarkTheme ? ReportTheme.Dark : ReportTheme.Light;

        // Embed logo as base64 so reports are self-contained.
        if (!string.IsNullOrEmpty(LogoPath) && File.Exists(LogoPath))
        {
            try
            {
                var logoBytes = File.ReadAllBytes(LogoPath);
                settings.LogoBase64 = Convert.ToBase64String(logoBytes);
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Could not embed logo from {Path} — report will reference path instead", LogoPath);
            }
        }
        else if (string.IsNullOrEmpty(LogoPath))
        {
            settings.LogoBase64 = null;
        }

        if (_appSettings.Save())
        {
            Log.Information("Settings saved (project={Project}, output={Output}, theme={Theme})",
                settings.ProjectName, settings.OutputFolder, settings.Theme);
            SaveCompleted?.Invoke();
        }
        else
        {
            // Save() already logged the underlying exception. Keep the dialog
            // open so the user knows something went wrong instead of silently
            // losing their input.
            Log.Warning("Settings write failed — leaving the dialog open");
        }
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
        try
        {
            IsCheckingForUpdates = true;
            UpdateStatusText = "Checking for updates...";

            var hasUpdate = await _updateService.CheckForUpdatesAsync();

            if (hasUpdate)
            {
                var version = _updateService.PendingUpdate?.TargetFullRelease.Version;
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
