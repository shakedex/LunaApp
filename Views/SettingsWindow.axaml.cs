using Avalonia.Controls;
using Avalonia.Interactivity;
using LunaApp.ViewModels;

namespace LunaApp.Views;

public partial class SettingsWindow : Window
{
    public SettingsWindow()
    {
        InitializeComponent();
    }

    protected override void OnDataContextChanged(EventArgs e)
    {
        base.OnDataContextChanged(e);

        if (DataContext is SettingsViewModel vm)
        {
            vm.StorageProvider = StorageProvider;
            // NB: the SaveCompleted → Close(true) hook is wired by the owner
            // (MainWindow) before ShowDialog runs, so that path is guaranteed.
        }
    }

    private void OnCancel(object? sender, RoutedEventArgs e)
    {
        Close(false);
    }
}
