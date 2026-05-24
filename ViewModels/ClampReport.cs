using System.Collections.Generic;
using System.Linq;

namespace LunaApp.ViewModels;

/// <summary>
/// Collects entries when <see cref="SettingsViewModel.Save"/> clamps fields.
/// Hand-off shape from Settings to MainWindow so the state strip can surface
/// "what got changed" after the dialog closes.
/// </summary>
public sealed class ClampReport
{
    private readonly List<string> _labels = [];

    public void Add(string label, int originalLength, int clampedTo) => _labels.Add(label);

    public bool HasAny => _labels.Count > 0;

    public string Describe()
    {
        if (_labels.Count == 0) return string.Empty;
        if (_labels.Count == 1) return $"{_labels[0]} trimmed to fit";
        if (_labels.Count == 2) return $"{_labels[0]} and {_labels[1]} trimmed to fit";

        var head = string.Join(", ", _labels.Take(_labels.Count - 1));
        return $"{head}, and {_labels.Last()} trimmed to fit";
    }
}
