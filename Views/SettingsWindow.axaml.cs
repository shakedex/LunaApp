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
        
        // Provide storage provider to view model for file dialogs
        if (DataContext is SettingsViewModel vm)
        {
            vm.StorageProvider = StorageProvider;
        }
    }
    
    private void OnSave(object? sender, RoutedEventArgs e)
    {
        Close(true);
    }
    
    private void OnCancel(object? sender, RoutedEventArgs e)
    {
        Close(false);
    }
}
