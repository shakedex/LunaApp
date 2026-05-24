using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Interactivity;
using LunaApp.Services;

namespace LunaApp.Views.Controls;

public partial class StateStrip : UserControl
{
    public static readonly StyledProperty<StateMessage?> MessageProperty =
        AvaloniaProperty.Register<StateStrip, StateMessage?>(nameof(Message));

    public StateMessage? Message
    {
        get => GetValue(MessageProperty);
        set => SetValue(MessageProperty, value);
    }

    public event EventHandler? DismissRequested;

    public StateStrip()
    {
        InitializeComponent();
    }

    private void OnDismissClick(object? sender, RoutedEventArgs e) =>
        DismissRequested?.Invoke(this, EventArgs.Empty);
}
