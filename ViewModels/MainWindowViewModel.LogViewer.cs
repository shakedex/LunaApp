using System.Collections.ObjectModel;
using System.Collections.Specialized;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Services;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Developer log viewer concerns of <see cref="MainWindowViewModel"/>:
/// toggle visibility, track log entries, clear.
/// </summary>
public partial class MainWindowViewModel
{
    [ObservableProperty] private bool _isLogViewerVisible;
    [ObservableProperty] private string _logText = string.Empty;

    /// <summary>
    /// True only on local Debug builds. Gates the "Dev" header button and
    /// the developer tools panel so Release builds shipped through GitHub
    /// have no dev playground exposed to end users.
    /// </summary>
    public bool IsDevBuild
    {
        get
        {
#if DEBUG
            return true;
#else
            return false;
#endif
        }
    }

    /// <summary>
    /// Belt-and-suspenders: the panel only renders when both the build is
    /// dev *and* the user has toggled the panel open. Prevents the panel
    /// from ever appearing in Release even if IsLogViewerVisible were
    /// flipped via some unforeseen path.
    /// </summary>
    public bool IsDevPanelVisible => IsDevBuild && IsLogViewerVisible;

    partial void OnIsLogViewerVisibleChanged(bool value) =>
        OnPropertyChanged(nameof(IsDevPanelVisible));

    public ObservableCollection<LogEntry> LogEntries => InMemoryLogSink.Instance.LogEntries;

    private void OnLogEntriesChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        LogText = string.Join(Environment.NewLine, LogEntries.Select(l => l.FormattedMessage));
    }

    [RelayCommand]
    private void ToggleLogViewer()
    {
        IsLogViewerVisible = !IsLogViewerVisible;
        Log.Debug("Log viewer toggled: {IsVisible}", IsLogViewerVisible);
    }

    [RelayCommand]
    private void ClearLogs()
    {
        InMemoryLogSink.Instance.Clear();
    }
}
