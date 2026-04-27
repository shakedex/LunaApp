using System.Diagnostics;
using System.Reflection;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Services;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Credits / About dialog VM. Shows authorship and links, hosts the
/// "Check for updates" workflow (moved here from SettingsViewModel so the
/// settings dialog stays scoped to per-report preferences), and surfaces
/// the running app version.
/// </summary>
public partial class CreditsViewModel : ViewModelBase
{
    private readonly UpdateService _updateService;

    [ObservableProperty] private bool _isCheckingForUpdates;
    [ObservableProperty] private string _updateStatusText = "Click to check for updates";

    public string VersionText
    {
        get
        {
            var installed = _updateService.CurrentVersion;
            if (!string.IsNullOrEmpty(installed)) return $"Version {installed}";

            var asm = Assembly.GetEntryAssembly()?.GetName().Version;
            return asm is null ? "Development build" : $"Version {asm.ToString(3)} (dev)";
        }
    }

    public string CheckForUpdatesButtonText => IsCheckingForUpdates ? "Checking..." : "Check for Updates";

    public CreditsViewModel(UpdateService updateService)
    {
        _updateService = updateService;
    }

    partial void OnIsCheckingForUpdatesChanged(bool value) =>
        OnPropertyChanged(nameof(CheckForUpdatesButtonText));

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

    [RelayCommand]
    private void OpenLink(string? url)
    {
        if (string.IsNullOrWhiteSpace(url)) return;

        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = url,
                UseShellExecute = true,
            });
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to open link {Url}", url);
        }
    }
}
