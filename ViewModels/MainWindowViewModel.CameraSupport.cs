using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using LunaApp.Services;
using LunaApp.Services.CameraSupport;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Camera-support "missing tool" toast concerns of <see cref="MainWindowViewModel"/>.
/// Mirrors the shape of <c>MainWindowViewModel.Update.cs</c>: a small bit of
/// observable state, snooze persistence on AppSettings, and three commands
/// (Open Settings / Later / Dismiss). The actual install UX lives in
/// Settings → Camera Support; we don't duplicate it here.
/// </summary>
public partial class MainWindowViewModel
{
    private CameraSupportToastState? _cameraSupportToast;

    [ObservableProperty] private bool _hasMissingCameraSupport;
    [ObservableProperty] private string _missingCameraSupportSummary = string.Empty;

    partial void OnHasMissingCameraSupportChanged(bool value)
    {
        if (value) PushCameraSupportBanner();
        else Banners.RemoveByKey("camera-support");
    }

    partial void OnMissingCameraSupportSummaryChanged(string value)
    {
        if (HasMissingCameraSupport) PushCameraSupportBanner();
    }

    private void PushCameraSupportBanner()
    {
        Banners.AddOrReplace(new BannerItem
        {
            Key = "camera-support",
            Level = Level.Warning,
            Title = "Camera support missing",
            Body = $"{MissingCameraSupportSummary} not installed. To decode these formats, install the tools from Settings.",
            PrimaryAction = new BannerAction("Open Settings", OpenSettingsForCameraSupportCommand),
            SecondaryAction = new BannerAction("Later", RemindCameraSupportLaterCommand),
            IsDismissible = true,
            OnDismiss = DismissCameraSupportCommand,
        });
    }

    private void SubscribeToCameraSupport(CameraSupportInstallationStatus status)
    {
        _cameraSupportToast = new CameraSupportToastState(status, _appSettings, s => s.Save());
        _cameraSupportToast.PropertyChanged += (_, e) =>
        {
            Avalonia.Threading.Dispatcher.UIThread.Post(() =>
            {
                if (e.PropertyName == nameof(CameraSupportToastState.HasMissingCameraSupport))
                    HasMissingCameraSupport = _cameraSupportToast.HasMissingCameraSupport;
                else if (e.PropertyName == nameof(CameraSupportToastState.MissingCameraSupportSummary))
                    MissingCameraSupportSummary = _cameraSupportToast.MissingCameraSupportSummary;
            });
        };
        status.StatusChanged += (_, _) => _cameraSupportToast.OnStatusChanged();
        // Initial probe — off the UI thread because the production probes
        // hit disk and may shell out to --version. Swallow + log any failure
        // so a flaky locator never crashes app startup; the toast just stays
        // hidden in that case.
        Task.Run(() =>
        {
            try { _cameraSupportToast.Refresh(); }
            catch (Exception ex) { Log.Warning(ex, "Camera-support initial probe failed"); }
        });
    }

    [RelayCommand]
    private void OpenSettingsForCameraSupport()
    {
        // Reuse the existing settings open path — same code-behind handles it.
        OpenSettingsRequested?.Invoke();
    }

    [RelayCommand]
    private void RemindCameraSupportLater()
    {
        _cameraSupportToast?.RemindLater();
        Log.Information("Camera-support toast snoozed until {Until}", _appSettings.CameraSupportSnoozeUntil);
    }

    [RelayCommand]
    private void DismissCameraSupport() =>
        _cameraSupportToast?.Dismiss();
}

/// <summary>
/// Observable state for the camera-support toast. Lives outside the
/// <see cref="MainWindowViewModel"/> so it can be unit-tested without
/// needing to construct the full shell view-model.
/// </summary>
public sealed class CameraSupportToastState : ObservableObject
{
    private readonly CameraSupportInstallationStatus _status;
    private readonly AppSettings _settings;
    private readonly Action<AppSettings> _save;
    private bool _dismissedThisSession;

    private bool _hasMissingCameraSupport;
    public bool HasMissingCameraSupport
    {
        get => _hasMissingCameraSupport;
        private set => SetProperty(ref _hasMissingCameraSupport, value);
    }

    private string _missingCameraSupportSummary = string.Empty;
    public string MissingCameraSupportSummary
    {
        get => _missingCameraSupportSummary;
        private set => SetProperty(ref _missingCameraSupportSummary, value);
    }

    public CameraSupportToastState(
        CameraSupportInstallationStatus status,
        AppSettings settings,
        Action<AppSettings> save)
    {
        _status = status;
        _settings = settings;
        _save = save;
    }

    private bool IsSnoozed =>
        (_settings.CameraSupportSnoozeUntil is DateTime snooze && snooze > DateTime.Now) ||
        (_settings.CameraSupportDismissedUntil is DateTime dismiss && dismiss > DateTime.Now);

    public void Refresh()
    {
        if (_dismissedThisSession || IsSnoozed)
        {
            HasMissingCameraSupport = false;
            MissingCameraSupportSummary = string.Empty;
            return;
        }

        var missing = _status.ResolveMissing();
        if (missing.Count == 0)
        {
            HasMissingCameraSupport = false;
            MissingCameraSupportSummary = string.Empty;
            return;
        }

        MissingCameraSupportSummary = string.Join(", ", missing.Select(m => m.DisplayName));
        HasMissingCameraSupport = true;
    }

    /// <summary>Called when <see cref="CameraSupportInstallationStatus.StatusChanged"/> fires.</summary>
    public void OnStatusChanged()
    {
        // If the user dismissed because of a specific missing set, and that set
        // is now installed, clear the dismissal so they're not permanently
        // hidden from future changes (e.g. a different vendor goes missing
        // because the user uninstalled it).
        if (_settings.CameraSupportDismissedUntil is not null)
        {
            var missing = _status.ResolveMissing();
            if (missing.Count == 0)
            {
                _settings.CameraSupportDismissedUntil = null;
                _save(_settings);
                _dismissedThisSession = false;
            }
        }

        Refresh();
    }

    public void RemindLater()
    {
        _settings.CameraSupportSnoozeUntil = DateTime.Now.AddDays(3);
        _save(_settings);
        HasMissingCameraSupport = false;
        MissingCameraSupportSummary = string.Empty;
    }

    public void Dismiss()
    {
        _settings.CameraSupportDismissedUntil = DateTime.MaxValue;
        _save(_settings);
        HasMissingCameraSupport = false;
        MissingCameraSupportSummary = string.Empty;
    }
}
