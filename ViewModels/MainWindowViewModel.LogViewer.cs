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
