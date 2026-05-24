using System;
using System.Globalization;
using Avalonia;
using Avalonia.Data.Converters;
using Avalonia.Media;
using Avalonia.Styling;
using LunaApp.Services;

namespace LunaApp.Views.Controls;

/// <summary>
/// Resolves a <see cref="Level"/> to one of the Luna theme brushes. Used by
/// StateStrip, BannerStack item template, and InlineBanner so a single point
/// owns the level→color mapping. Returns <c>LunaBgTertiary</c> for Info,
/// <c>LunaSuccess</c> for Success, <c>LunaWarning</c> for Warning,
/// <c>LunaDanger</c> for Error.
/// </summary>
public sealed class LevelToBrushConverter : IValueConverter
{
    public static readonly LevelToBrushConverter Instance = new();

    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not Level level) return null;
        var key = level switch
        {
            Level.Info    => "LunaBgTertiary",
            Level.Success => "LunaSuccess",
            Level.Warning => "LunaWarning",
            Level.Error   => "LunaDanger",
            _             => "LunaBgTertiary",
        };
        if (Application.Current?.TryGetResource(key, ThemeVariant.Default, out var brush) == true)
        {
            return brush;
        }
        return Brushes.Transparent;
    }

    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
