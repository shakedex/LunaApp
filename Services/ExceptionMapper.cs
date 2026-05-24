using System;
using System.IO;
using CommunityToolkit.Mvvm.Input;

namespace LunaApp.Services;

/// <summary>Severity level shared by StateStrip, BannerStack, and InlineBanner.</summary>
public enum Level
{
    Info,
    Success,
    Warning,
    Error,
}

/// <summary>Optional inline action button on a state strip or banner.</summary>
public sealed record StateAction(string Label, IRelayCommand Command);

/// <summary>
/// Single payload bound by the StateStrip control. Replaces the old
/// <c>StatusText</c> string. The factory helpers cover every shape the
/// app needs without exposing the constructor.
/// </summary>
public sealed record StateMessage(
    Level Level,
    string Text,
    StateAction? Action = null,
    bool IsDismissible = false)
{
    public static StateMessage Info(string text, StateAction? action = null) =>
        new(Level.Info, text, action, IsDismissible: false);

    public static StateMessage Success(string text, StateAction? action = null) =>
        new(Level.Success, text, action, IsDismissible: false);

    public static StateMessage Warning(string text, StateAction? action = null) =>
        new(Level.Warning, text, action, IsDismissible: true);

    public static StateMessage Error(string text, StateAction? action = null) =>
        new(Level.Error, text, action, IsDismissible: true);

    /// <summary>Neutral idle state — used as the initial value on app start.</summary>
    public static StateMessage Idle(string text) => new(Level.Info, text);
}

/// <summary>
/// Maps caught exceptions to a friendly <see cref="StateMessage"/>. The raw
/// exception should always be logged at the call site before invoking this —
/// the user gets a clean message, the dev gets the full stack.
/// </summary>
public static class ExceptionMapper
{
    public static StateMessage ToUserMessage(Exception ex, string contextVerb) => ex switch
    {
        OperationCanceledException        => StateMessage.Info($"{contextVerb} cancelled"),
        UnauthorizedAccessException       => StateMessage.Error("Permission denied — check folder access"),
        DirectoryNotFoundException        => StateMessage.Error("Folder not found"),
        FileNotFoundException             => StateMessage.Error("File not found"),
        PathTooLongException              => StateMessage.Error("File path is too long"),
        IOException io when IsDiskFull(io)=> StateMessage.Error("Not enough disk space"),
        IOException                       => StateMessage.Error("File system error — the file may be open in another app"),
        _                                 => StateMessage.Error("Something went wrong"),
    };

    // ERROR_DISK_FULL = 0x70 on Windows; ENOSPC = 0x1C on Linux/macOS. Both
    // surface as the low 16 bits of HResult.
    private static bool IsDiskFull(IOException io) =>
        (io.HResult & 0xFFFF) is 0x70 or 0x1C;
}
