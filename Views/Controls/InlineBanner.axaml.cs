using Avalonia;
using Avalonia.Controls;
using LunaApp.ViewModels;

namespace LunaApp.Views.Controls;

public partial class InlineBanner : UserControl
{
    public static readonly StyledProperty<InlineBannerState?> StateProperty =
        AvaloniaProperty.Register<InlineBanner, InlineBannerState?>(nameof(State));

    public InlineBannerState? State
    {
        get => GetValue(StateProperty);
        set => SetValue(StateProperty, value);
    }

    public InlineBanner()
    {
        InitializeComponent();
    }
}
