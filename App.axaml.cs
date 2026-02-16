using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Data.Core;
using Avalonia.Data.Core.Plugins;
using System.Linq;
using Avalonia.Markup.Xaml;
using LunaApp.Services;
using LunaApp.ViewModels;
using LunaApp.Views;
using Serilog;

namespace LunaApp;

public partial class App : Application
{
    /// <summary>
    /// Global update service instance for checking and applying updates.
    /// </summary>
    public static UpdateService? UpdateService { get; private set; }
    
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // Avoid duplicate validations from both Avalonia and the CommunityToolkit. 
            // More info: https://docs.avaloniaui.net/docs/guides/development-guides/data-validation#manage-validationplugins
            DisableAvaloniaDataAnnotationValidation();
            desktop.MainWindow = new MainWindow
            {
                DataContext = new MainWindowViewModel(),
            };
        }

        base.OnFrameworkInitializationCompleted();
        
        // DEV: Mark log sink ready now that UI thread is available
        InMemoryLogSink.Instance.MarkUiReady();
        
        // Initialize update service and check for updates in background
        InitializeUpdatesAsync();
    }
    
    private async void InitializeUpdatesAsync()
    {
        UpdateService = new UpdateService();
        
        try
        {
            // Check for updates silently on startup
            await UpdateService.CheckForUpdatesAsync();
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Update check failed on startup");
        }
    }

    private void DisableAvaloniaDataAnnotationValidation()
    {
        // Get an array of plugins to remove
        var dataValidationPluginsToRemove =
            BindingPlugins.DataValidators.OfType<DataAnnotationsValidationPlugin>().ToArray();

        // remove each entry found
        foreach (var plugin in dataValidationPluginsToRemove)
        {
            BindingPlugins.DataValidators.Remove(plugin);
        }
    }
}