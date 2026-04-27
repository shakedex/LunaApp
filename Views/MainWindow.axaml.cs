using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Platform.Storage;
using LunaApp.Models;
using LunaApp.ViewModels;
using Microsoft.Extensions.DependencyInjection;
using Serilog;

namespace LunaApp.Views;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        AddHandler(DragDrop.DropEvent, OnDrop);
        AddHandler(DragDrop.DragOverEvent, OnDragOver);
        AddHandler(DragDrop.DragEnterEvent, OnDragEnter);
        AddHandler(DragDrop.DragLeaveEvent, OnDragLeave);

        RestoreWindowPlacement();
        Closing += OnClosingPersistPlacement;
    }

    /// <summary>
    /// Restores saved window size / position / maximized state from
    /// <see cref="AppSettings"/>. Falls back to the XAML defaults if nothing's
    /// saved yet or the saved position is off-screen.
    /// </summary>
    private void RestoreWindowPlacement()
    {
        try
        {
            var s = AppSettings.Load();
            if (s.WindowWidth > 400 && s.WindowHeight > 300)
            {
                Width = s.WindowWidth;
                Height = s.WindowHeight;
            }
            if (s.WindowX.HasValue && s.WindowY.HasValue)
            {
                Position = new PixelPoint(s.WindowX.Value, s.WindowY.Value);
                WindowStartupLocation = WindowStartupLocation.Manual;
            }
            if (s.IsMaximized)
            {
                WindowState = WindowState.Maximized;
            }
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to restore window placement — using defaults");
        }
    }

    private void OnClosingPersistPlacement(object? sender, WindowClosingEventArgs e)
    {
        try
        {
            var s = AppSettings.Load();
            s.IsMaximized = WindowState == WindowState.Maximized;
            if (WindowState == WindowState.Normal)
            {
                s.WindowWidth = Width;
                s.WindowHeight = Height;
                s.WindowX = Position.X;
                s.WindowY = Position.Y;
            }
            s.Save();
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to persist window placement on close");
        }
    }

    protected override void OnDataContextChanged(EventArgs e)
    {
        base.OnDataContextChanged(e);

        if (DataContext is MainWindowViewModel vm)
        {
            vm.StorageProvider = StorageProvider;
            vm.Clipboard = Clipboard;
            vm.OpenSettingsRequested += OnOpenSettingsRequested;
        }
    }

    private async void OnOpenSettingsRequested()
    {
        // Create the VM first so we can wire the close-on-save event *before* the
        // dialog runs. The earlier OnDataContextChanged-based approach didn't fire
        // reliably depending on initializer ordering, so this path is explicit.
        var settingsVm = Program.Services.GetRequiredService<SettingsViewModel>();
        var settingsWindow = new SettingsWindow { DataContext = settingsVm };
        settingsVm.SaveCompleted += () => settingsWindow.Close(true);

        var result = await settingsWindow.ShowDialog<bool?>(this);

        if (result == true && DataContext is MainWindowViewModel vm)
        {
            Log.Information("Settings dialog closed with save — reloading active report settings");
            vm.ReloadSettings();
        }
    }

    private void OnDragOver(object? sender, DragEventArgs e)
    {
#pragma warning disable CS0618 // Type or member is obsolete
        var isFiles = e.Data.Contains(DataFormats.Files);
#pragma warning restore CS0618
        e.DragEffects = isFiles ? DragDropEffects.Copy : DragDropEffects.None;
    }

    private void OnDragEnter(object? sender, DragEventArgs e)
    {
#pragma warning disable CS0618
        if (DataContext is MainWindowViewModel vm && e.Data.Contains(DataFormats.Files))
            vm.IsDragOver = true;
#pragma warning restore CS0618
    }

    private void OnDragLeave(object? sender, DragEventArgs e)
    {
        if (DataContext is MainWindowViewModel vm)
            vm.IsDragOver = false;
    }

    private async void OnDrop(object? sender, DragEventArgs e)
    {
        if (DataContext is not MainWindowViewModel vm) return;

        vm.IsDragOver = false;

        try
        {
#pragma warning disable CS0618 // Type or member is obsolete
            var files = e.Data.GetFiles();
#pragma warning restore CS0618
            if (files == null) return;

            var folders = new List<string>();
            foreach (var item in files)
            {
                var path = item.TryGetLocalPath();
                if (string.IsNullOrEmpty(path)) continue;

                if (Directory.Exists(path))
                {
                    folders.Add(path);
                }
                else
                {
                    // File dropped — use its parent directory
                    var directory = Path.GetDirectoryName(path);
                    if (!string.IsNullOrEmpty(directory) && Directory.Exists(directory))
                        folders.Add(directory);
                }
            }

            if (folders.Count == 0) return;

            if (folders.Count > 1)
            {
                Log.Information("Multi-folder drop received ({Count}), loading only the first: {Folder}",
                    folders.Count, folders[0]);
                vm.StatusText = $"Loaded '{Path.GetFileName(folders[0])}' — drop one folder at a time for now";
            }

            await vm.LoadFolderAsync(folders[0]);
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Error handling drop");
        }
    }
}
