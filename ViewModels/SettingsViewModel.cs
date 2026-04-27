using System.ComponentModel.DataAnnotations;
using Avalonia.Media.Imaging;
using Avalonia.Platform.Storage;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
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
    public IAsyncRelayCommand? DetectCommand { get; init; }

    [ObservableProperty] private string _state = "";
    [ObservableProperty] private string _summary = "";
    [ObservableProperty] private bool _isInstalling;
    [ObservableProperty] private double _installProgress;
    [ObservableProperty] private string? _installError;

    /// <summary>
    /// Set after we launched a vendor's interactive installer (Sony) but
    /// haven't yet been able to detect the binaries on disk. Hides the
    /// Install button and surfaces a Detect button + a helpful message —
    /// the user finishes Sony's installer at their own pace and clicks
    /// Detect when done.
    /// </summary>
    [ObservableProperty] private bool _isAwaitingDetect;

    public bool IsReady => State == "Ready";
    public bool CanInstall => InstallCommand is not null && !IsReady && !IsInstalling && !IsAwaitingDetect;
    public bool ShowInstallButton => InstallCommand is not null && !IsReady && !IsAwaitingDetect;
    public bool ShowDetectButton => DetectCommand is not null && IsAwaitingDetect && !IsInstalling;

    partial void OnStateChanged(string value)
    {
        OnPropertyChanged(nameof(IsReady));
        OnPropertyChanged(nameof(CanInstall));
        OnPropertyChanged(nameof(ShowInstallButton));
    }

    partial void OnIsInstallingChanged(bool value)
    {
        OnPropertyChanged(nameof(CanInstall));
        OnPropertyChanged(nameof(ShowDetectButton));
    }

    partial void OnIsAwaitingDetectChanged(bool value)
    {
        OnPropertyChanged(nameof(CanInstall));
        OnPropertyChanged(nameof(ShowInstallButton));
        OnPropertyChanged(nameof(ShowDetectButton));
    }
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
    private readonly AppSettings _appSettings;

    private readonly ArtCliInstaller _artCliInstaller;
    private readonly ArtCliLocator _artCliLocator;
    private readonly SonyRawViewerInstaller _sonyInstaller;
    private readonly SonyRawViewerLocator _sonyLocator;
    private readonly CameraSupportRegistry _cameraSupports;
    private readonly CameraSupportInstallationStatus _installStatus;

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

    /// <summary>True when at least one report format is enabled — gates "Open when complete".</summary>
    public bool WillGenerateAnyReport => GenerateHtmlByDefault || GeneratePdfByDefault;

    /// <summary>Raised when the Save command succeeded so the host window can close with a positive result.</summary>
    public event Action? SaveCompleted;

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
        CameraSupportRegistry cameraSupports,
        ArtCliInstaller artCliInstaller,
        ArtCliLocator artCliLocator,
        SonyRawViewerInstaller sonyInstaller,
        SonyRawViewerLocator sonyLocator,
        CameraSupportInstallationStatus installStatus)
    {
        _artCliInstaller = artCliInstaller;
        _artCliLocator = artCliLocator;
        _sonyInstaller = sonyInstaller;
        _sonyLocator = sonyLocator;
        _cameraSupports = cameraSupports;
        _installStatus = installStatus;
        _appSettings = AppSettings.Load();

        CameraSupports = cameraSupports.All.Select(BuildRow).ToArray();

        LoadSettings();
        // Run validation once so inline field errors (not Save gating) are accurate.
        ValidateAllProperties();
    }

    private CameraSupportRow BuildRow(ICameraSupport support)
    {
        // Each vendor has its own installer + locator pair. The row stays
        // generic; we just look up the right installer for this support id.
        Action? invalidate = support.Id switch
        {
            "arri"        => () => _artCliLocator.Invalidate(),
            "sony-venice" => () => _sonyLocator.Invalidate(),
            _             => null,
        };

        var (label, installCommand) = support.Id switch
        {
            "arri" => (
                BuildSizeLabel("Install ARRI Reference Tool", _artCliInstaller.CurrentRelease?.DownloadSizeBytes),
                (IAsyncRelayCommand?)new AsyncRelayCommand(() =>
                    RunInstallAsync(support.Id, p => _artCliInstaller.InstallAsync(p), invalidate!))),

            "sony-venice" => (
                BuildSizeLabel("Install Sony RAW Viewer", _sonyInstaller.CurrentRelease?.DownloadSizeBytes),
                (IAsyncRelayCommand?)new AsyncRelayCommand(() =>
                    RunInstallAsync(support.Id, p => RunSonyInstall(p), invalidate!))),

            _ => (null, null),
        };

        // Detect command is only set on rows that have a managed installer —
        // surfaces only after the install kicks off in "awaiting" mode.
        var detectCommand = invalidate is null
            ? null
            : new AsyncRelayCommand(() => DetectAsync(support.Id, invalidate));

        return new CameraSupportRow
        {
            SupportId = support.Id,
            DisplayName = support.DisplayName,
            State = SupportStateLabel(support.Status),
            Summary = SupportSummary(support.Status),
            InstallButtonLabel = label,
            InstallCommand = installCommand,
            DetectCommand = detectCommand,
        };
    }

    /// <summary>
    /// Manual re-probe after the user finishes an out-of-process installer
    /// (Sony's exe, macOS DMG drag, etc.). Either flips the row to Ready or
    /// leaves it in "awaiting" mode with a friendly message.
    /// </summary>
    private Task DetectAsync(string supportId, Action invalidateLocator)
    {
        var row = CameraSupports.FirstOrDefault(r => r.SupportId == supportId);
        if (row is null) return Task.CompletedTask;

        invalidateLocator();
        var support = _cameraSupports.All.FirstOrDefault(s => s.Id == supportId);
        if (support is null) return Task.CompletedTask;

        if (support.Status is SupportStatus.Ready)
        {
            row.State = SupportStateLabel(support.Status);
            row.Summary = SupportSummary(support.Status);
            row.IsAwaitingDetect = false;
            row.InstallError = null;
            _installStatus.Invalidate();
        }
        else
        {
            row.InstallError = "Still not detected. Make sure the installer finished — if it's still running, wait for it. If it's done and Luna can't find the install, restart Luna.";
        }

        return Task.CompletedTask;
    }

    private static string BuildSizeLabel(string baseLabel, long? bytes)
    {
        if (bytes is null or <= 0) return baseLabel;
        var mb = bytes.Value / (1024 * 1024);
        return $"{baseLabel} ({mb} MB)";
    }

    /// <summary>
    /// Bridges the Sony installer's signature into the shared
    /// <see cref="RunInstallAsync"/> flow. The Sony installer's
    /// <c>InstallResult</c> is a parallel record to ART CLI's, so we wrap
    /// it in the shared shape here.
    /// </summary>
    private async Task<(bool Success, string? Path, string? Error)> RunSonyInstall(IProgress<double> progress)
    {
        var r = await _sonyInstaller.InstallAsync(progress);
        return (r.Success, r.Path, r.Error);
    }

    /// <summary>
    /// Shared install-button handler — ART CLI overload.
    /// </summary>
    private Task RunInstallAsync(
        string supportId,
        Func<IProgress<double>, Task<ArtCliInstaller.InstallResult>> installFunc,
        Action invalidateLocator) =>
        RunInstallCoreAsync(supportId, async p =>
        {
            var r = await installFunc(p);
            return (r.Success, r.Error);
        }, invalidateLocator);

    /// <summary>Sony's overload — same flow, different installer's result type.</summary>
    private Task RunInstallAsync(
        string supportId,
        Func<IProgress<double>, Task<(bool Success, string? Path, string? Error)>> installFunc,
        Action invalidateLocator) =>
        RunInstallCoreAsync(supportId, async p =>
        {
            var r = await installFunc(p);
            return (r.Success, r.Error);
        }, invalidateLocator);

    /// <summary>
    /// Real install loop. Drives the row's IsInstalling / InstallProgress /
    /// InstallError observables, then re-resolves the vendor locator. If
    /// the locator finds the binaries, the row flips to Ready. If it
    /// doesn't (e.g. Sony's installer is still running asynchronously),
    /// the row enters "awaiting detect" mode — the user finishes the
    /// installer at their own pace and clicks Detect to confirm.
    /// </summary>
    private async Task RunInstallCoreAsync(
        string supportId,
        Func<IProgress<double>, Task<(bool Success, string? Error)>> installFunc,
        Action invalidateLocator)
    {
        var row = CameraSupports.FirstOrDefault(r => r.SupportId == supportId);
        if (row is null) return;

        row.IsInstalling = true;
        row.InstallError = null;
        row.IsAwaitingDetect = false;
        row.InstallProgress = 0;

        var progress = new Progress<double>(p => row.InstallProgress = p);
        try
        {
            var result = await installFunc(progress);
            if (!result.Success)
            {
                row.InstallError = result.Error;
                return;
            }

            invalidateLocator();
            var support = _cameraSupports.All.FirstOrDefault(s => s.Id == supportId);
            if (support is null) return;

            if (support.Status is SupportStatus.Ready)
            {
                // ART CLI path: zip extract is fully done, locator finds it.
                row.State = SupportStateLabel(support.Status);
                row.Summary = SupportSummary(support.Status);
                _installStatus.Invalidate();
            }
            else
            {
                // Sony path: installer launched but binaries aren't on disk
                // yet. Hand off to the Detect button + helpful message.
                row.IsAwaitingDetect = true;
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
}
