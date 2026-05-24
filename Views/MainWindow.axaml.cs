using System;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using Avalonia.Styling;
using LunaApp.Models;
using LunaApp.Services;
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
            vm.OpenCreditsRequested += OnOpenCreditsRequested;
            vm.ClearConfirmRequested = ShowClearConfirmAsync;
        }
    }

    private async void OnOpenCreditsRequested()
    {
        var creditsVm = Program.Services.GetRequiredService<CreditsViewModel>();
        var creditsWindow = new CreditsWindow { DataContext = creditsVm };
        await creditsWindow.ShowDialog(this);
    }

    private async Task<bool> ShowClearConfirmAsync()
    {
        var tcs = new TaskCompletionSource<bool>();

        object? bgRes = null;
        object? textRes = null;
        Application.Current?.TryGetResource("LunaBgPrimary", ThemeVariant.Default, out bgRes);
        Application.Current?.TryGetResource("LunaTextPrimary", ThemeVariant.Default, out textRes);
        IBrush? bgBrush = bgRes as IBrush ?? Brushes.Transparent;
        IBrush? textBrush = textRes as IBrush;

        var dialog = new Window
        {
            Title = "Clear loaded reels?",
            Width = 420,
            Height = 170,
            SystemDecorations = SystemDecorations.BorderOnly,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = bgBrush,
        };

        var cancel = new Button
        {
            Content = "Cancel",
            Classes = { "secondary" },
        };
        var clear = new Button
        {
            Content = "Clear",
            Classes = { "primary" },
        };
        cancel.Click += (_, _) => { tcs.TrySetResult(false); dialog.Close(); };
        clear.Click  += (_, _) => { tcs.TrySetResult(true);  dialog.Close(); };

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            Children = { cancel, clear },
        };
        var content = new StackPanel
        {
            Margin = new Thickness(20),
            Spacing = 16,
            Children =
            {
                new TextBlock
                {
                    Text = "Clear all loaded reels and start over?",
                    FontSize = 14,
                    Foreground = textBrush,
                },
                buttons,
            },
        };
        dialog.Content = content;

        await dialog.ShowDialog(this);
        return await tcs.Task;
    }

    private async void OnOpenSettingsRequested()
    {
        // Create the VM first so we can wire the close-on-save event *before* the
        // dialog runs. The earlier OnDataContextChanged-based approach didn't fire
        // reliably depending on initializer ordering, so this path is explicit.
        var settingsVm = Program.Services.GetRequiredService<SettingsViewModel>();
        var settingsWindow = new SettingsWindow { DataContext = settingsVm };
        settingsVm.SaveCompleted += (clampReport) =>
        {
            if (DataContext is MainWindowViewModel mwvm)
            {
                mwvm.OnSettingsSaved(clampReport);
            }
            settingsWindow.Close(true);
        };

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
                vm.State = StateMessage.Warning($"Loaded '{Path.GetFileName(folders[0])}' — drop one folder at a time for now");
            }

            await vm.LoadFolderAsync(folders[0]);
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Error handling drop");
        }
    }
}
