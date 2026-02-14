using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Platform.Storage;
using LunaApp.ViewModels;
using Serilog;

namespace LunaApp.Views;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        
        // Set up drag-drop handlers
        AddHandler(DragDrop.DropEvent, OnDrop);
        AddHandler(DragDrop.DragOverEvent, OnDragOver);
    }
    
    protected override void OnDataContextChanged(EventArgs e)
    {
        base.OnDataContextChanged(e);
        
        // Provide storage provider to view model for file dialogs
        if (DataContext is MainWindowViewModel vm)
        {
            vm.StorageProvider = StorageProvider;
            vm.OpenSettingsRequested += OnOpenSettingsRequested;
        }
    }
    
    private async void OnOpenSettingsRequested()
    {
        var settingsWindow = new SettingsWindow
        {
            DataContext = new SettingsViewModel()
        };
        
        var result = await settingsWindow.ShowDialog<bool?>(this);
        
        if (result == true && DataContext is MainWindowViewModel vm)
        {
            vm.ReloadSettings();
        }
    }
    
    private void OnDragOver(object? sender, DragEventArgs e)
    {
        // Only accept folders/files
#pragma warning disable CS0618 // Type or member is obsolete
        e.DragEffects = e.Data.Contains(DataFormats.Files) 
            ? DragDropEffects.Copy 
            : DragDropEffects.None;
#pragma warning restore CS0618
    }
    
    private async void OnDrop(object? sender, DragEventArgs e)
    {
        if (DataContext is not MainWindowViewModel vm) return;
        
        try
        {
#pragma warning disable CS0618 // Type or member is obsolete
            var files = e.Data.GetFiles();
#pragma warning restore CS0618
            if (files == null) return;
            
            foreach (var item in files)
            {
                var path = item.TryGetLocalPath();
                if (string.IsNullOrEmpty(path)) continue;
                
                // Check if it's a directory
                if (Directory.Exists(path))
                {
                    await vm.LoadFolderAsync(path);
                    return; // Only load first folder
                }
                
                // If it's a file, use its parent directory
                var directory = Path.GetDirectoryName(path);
                if (!string.IsNullOrEmpty(directory) && Directory.Exists(directory))
                {
                    await vm.LoadFolderAsync(directory);
                    return;
                }
            }
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Error handling drop");
        }
    }
}