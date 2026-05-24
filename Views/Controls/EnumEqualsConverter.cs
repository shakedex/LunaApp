using System;
using System.Globalization;
using Avalonia.Data.Converters;

namespace LunaApp.Views.Controls;

/// <summary>
/// Returns <c>true</c> when the bound enum value equals the
/// <see cref="IValueConverter.Convert"/> <paramref name="parameter"/>.
/// Used by MainWindow.axaml to toggle overlay layer visibility based on
/// <see cref="LunaApp.ViewModels.OverlayState"/>.
/// </summary>
public sealed class EnumEqualsConverter : IValueConverter
{
    public static readonly EnumEqualsConverter Instance = new();

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is null) return false;
        // ConverterParameter arrives as a string from XAML — compare via ToString so
        // both "SuccessHold" and the actual enum value OverlayState.SuccessHold match.
        return parameter is string s
            ? string.Equals(value.ToString(), s, StringComparison.Ordinal)
            : value.Equals(parameter);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
