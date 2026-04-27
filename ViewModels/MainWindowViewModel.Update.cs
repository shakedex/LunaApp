using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Update banner + auto-update workflow concerns of <see cref="MainWindowViewModel"/>:
/// detect update, download progress, apply/restart, dismiss, plus dev helpers.
/// </summary>
public partial class MainWindowViewModel
{
    [ObservableProperty] private bool _hasUpdateAvailable;
    [ObservableProperty] private string _updateVersion = string.Empty;
    [ObservableProperty] private int _updateDownloadProgress;
    [ObservableProperty] private bool _isDownloadingUpdate;
    [ObservableProperty] private bool _isUpdateReady;

    private bool IsSnoozed =>
        _appSettings.UpdateSnoozeUntil is DateTime until && until > DateTime.Now;

    private void SubscribeToUpdateService()
    {
        _updateService.UpdateAvailable += (_, info) =>
        {
            if (IsSnoozed)
            {
                Log.Debug("Update available but snoozed until {Until}", _appSettings.UpdateSnoozeUntil);
                return;
            }
            HasUpdateAvailable = true;
            UpdateVersion = info.TargetFullRelease.Version.ToString();
            IsUpdateReady = false;
            IsDownloadingUpdate = false;
        };

        _updateService.DevUpdateAvailable += (_, version) =>
        {
            // Dev helper bypasses snooze so we can test the banner UI on demand.
            HasUpdateAvailable = true;
            UpdateVersion = version;
            IsUpdateReady = false;
            IsDownloadingUpdate = false;
        };

        _updateService.DownloadProgress += (_, progress) =>
        {
            UpdateDownloadProgress = progress;
        };

        _updateService.UpdateReady += (_, _) =>
        {
            IsDownloadingUpdate = false;
            IsUpdateReady = true;
        };
    }

    [RelayCommand]
    private async Task DownloadUpdateAsync()
    {
        try
        {
            IsDownloadingUpdate = true;
            UpdateDownloadProgress = 0;
            await _updateService.DownloadUpdateAsync();
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
        _updateService.ApplyUpdateAndRestart();
    }

    [RelayCommand]
    private void DismissUpdate()
    {
        HasUpdateAvailable = false;
        IsUpdateReady = false;
        IsDownloadingUpdate = false;
    }

    /// <summary>Snooze update reminders for 3 days and hide the banner.</summary>
    [RelayCommand]
    private void RemindUpdateLater()
    {
        _appSettings.UpdateSnoozeUntil = DateTime.Now.AddDays(3);
        _appSettings.Save();
        Log.Information("Update snoozed until {Until}", _appSettings.UpdateSnoozeUntil);
        HasUpdateAvailable = false;
        IsUpdateReady = false;
        IsDownloadingUpdate = false;
    }

    [RelayCommand]
    private async Task CheckForUpdatesAsync()
    {
        try
        {
            StatusText = "Checking for updates...";
            var hasUpdate = await _updateService.CheckForUpdatesAsync();
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
        _updateService.DevSimulateUpdateAvailable("99.0.0");
    }

    [RelayCommand]
    private void DevSimulateDownload()
    {
        IsDownloadingUpdate = true;
        UpdateDownloadProgress = 0;

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
}
