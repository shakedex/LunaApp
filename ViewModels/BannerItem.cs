using System.Collections.ObjectModel;
using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using LunaApp.Services;

namespace LunaApp.ViewModels;

/// <summary>Primary or secondary action attached to a banner.</summary>
public sealed record BannerAction(string Label, ICommand Command);

/// <summary>
/// One row in the BannerStack. Observable so a VM partial can mutate
/// Body/PrimaryAction in place (e.g. update download progress) without
/// removing and re-adding the item.
/// </summary>
public sealed partial class BannerItem : ObservableObject
{
    public required string Key { get; init; }
    public required Level Level { get; init; }
    public required string Title { get; init; }

    [ObservableProperty] private string? _body;
    [ObservableProperty] private BannerAction? _primaryAction;
    [ObservableProperty] private BannerAction? _secondaryAction;

    public bool IsDismissible { get; init; }
    public ICommand? OnDismiss { get; init; }
}

public static class BannerCollectionExtensions
{
    /// <summary>
    /// Add the item, or replace the existing item with the same Key. Used by
    /// VM partials whose state may change repeatedly (update progress, etc.).
    /// </summary>
    public static void AddOrReplace(this ObservableCollection<BannerItem> items, BannerItem item)
    {
        for (var i = 0; i < items.Count; i++)
        {
            if (items[i].Key == item.Key)
            {
                items[i] = item;
                return;
            }
        }
        items.Add(item);
    }

    public static void RemoveByKey(this ObservableCollection<BannerItem> items, string key)
    {
        for (var i = items.Count - 1; i >= 0; i--)
        {
            if (items[i].Key == key) items.RemoveAt(i);
        }
    }
}
