using System.Collections.ObjectModel;
using Avalonia;
using Avalonia.Controls;
using LunaApp.ViewModels;

namespace LunaApp.Views.Controls;

public partial class BannerStack : UserControl
{
    public static readonly StyledProperty<ObservableCollection<BannerItem>?> ItemsProperty =
        AvaloniaProperty.Register<BannerStack, ObservableCollection<BannerItem>?>(nameof(Items));

    public ObservableCollection<BannerItem>? Items
    {
        get => GetValue(ItemsProperty);
        set => SetValue(ItemsProperty, value);
    }

    public BannerStack()
    {
        InitializeComponent();
    }
}
