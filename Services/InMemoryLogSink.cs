using System.Collections.Concurrent;
using System.Collections.ObjectModel;
using Avalonia.Threading;
using Serilog.Core;
using Serilog.Events;

namespace LunaApp.Services;

/// <summary>
/// DEV ONLY: In-memory Serilog sink for real-time log viewing in the UI.
/// This captures all log events and stores them in an observable collection.
/// </summary>
public class InMemoryLogSink : ILogEventSink
{
    private static InMemoryLogSink? _instance;
    public static InMemoryLogSink Instance => _instance ??= new InMemoryLogSink();
    
    private const int MaxLogEntries = 1000;
    private readonly ConcurrentQueue<LogEntry> _pendingEntries = new();
    private volatile bool _isUiReady;
    
    public ObservableCollection<LogEntry> LogEntries { get; } = [];
    
    private InMemoryLogSink() { }
    
    /// <summary>
    /// Call this once the Avalonia UI thread is ready (after app initialization)
    /// </summary>
    public void MarkUiReady()
    {
        _isUiReady = true;
        FlushPendingEntries();
    }
    
    public void Emit(LogEvent logEvent)
    {
        var entry = new LogEntry
        {
            Timestamp = logEvent.Timestamp.LocalDateTime,
            Level = logEvent.Level,
            Message = logEvent.RenderMessage(),
            Exception = logEvent.Exception?.ToString()
        };
        
        if (!_isUiReady)
        {
            // Queue entries until UI is ready
            _pendingEntries.Enqueue(entry);
            return;
        }
        
        // Dispatch to UI thread for ObservableCollection updates
        Dispatcher.UIThread.Post(() => AddEntry(entry));
    }
    
    private void FlushPendingEntries()
    {
        Dispatcher.UIThread.Post(() =>
        {
            while (_pendingEntries.TryDequeue(out var entry))
            {
                AddEntry(entry);
            }
        });
    }
    
    private void AddEntry(LogEntry entry)
    {
        LogEntries.Add(entry);
        
        // Keep collection size bounded
        while (LogEntries.Count > MaxLogEntries)
        {
            LogEntries.RemoveAt(0);
        }
    }
    
    public void Clear()
    {
        if (_isUiReady)
        {
            Dispatcher.UIThread.Post(() => LogEntries.Clear());
        }
    }
}

/// <summary>
/// Represents a single log entry for display.
/// </summary>
public class LogEntry
{
    public DateTime Timestamp { get; init; }
    public LogEventLevel Level { get; init; }
    public required string Message { get; init; }
    public string? Exception { get; init; }
    
    public string FormattedMessage => Exception != null 
        ? $"[{Timestamp:HH:mm:ss}] [{Level}] {Message}\n{Exception}"
        : $"[{Timestamp:HH:mm:ss}] [{Level}] {Message}";
    
    public string LevelColor => Level switch
    {
        LogEventLevel.Verbose => "#6e7681",
        LogEventLevel.Debug => "#8b949e",
        LogEventLevel.Information => "#58a6ff",
        LogEventLevel.Warning => "#d29922",
        LogEventLevel.Error => "#f85149",
        LogEventLevel.Fatal => "#da3633",
        _ => "#e6edf3"
    };
}
