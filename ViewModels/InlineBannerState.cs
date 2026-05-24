using System.Windows.Input;
using LunaApp.Services;

namespace LunaApp.ViewModels;

/// <summary>
/// State for the InlineBanner control inside SettingsWindow. Carries an
/// optional retry command for the Error case.
/// </summary>
public sealed record InlineBannerState(
    Level Level,
    string Text,
    ICommand? RetryCommand = null)
{
    public static InlineBannerState Info(string text) => new(Level.Info, text);
    public static InlineBannerState Error(string text, ICommand? retryCommand = null) =>
        new(Level.Error, text, retryCommand);
}
