# Velopack Auto-Update System Implementation Guide

This document provides instructions for AI coding assistants on how to implement and extend the Velopack auto-update system in Luna.

## Overview

Luna uses [Velopack](https://velopack.io/) for application packaging and auto-updates. The system:
- Creates native installers (.exe for Windows, .dmg for macOS)
- Enables delta updates (only changed files downloaded)
- Checks GitHub Releases for new versions
- Handles the update lifecycle (download, apply, restart)

## Current Implementation State

### Installed Package
```xml
<!-- LunaApp.csproj -->
<PackageReference Include="Velopack" Version="0.0.1369-g1d5c984" />
```

### Bootstrap Code
```csharp
// Program.cs - Line 33
VelopackApp.Build().Run();
```

This minimal setup:
- Handles install/uninstall hooks
- Processes update application restart
- **Does NOT** check for updates automatically

## Implementing Auto-Update Checks

### Step 1: Create Update Service

Create a new service to handle update logic:

```csharp
// Services/UpdateService.cs
using Serilog;
using Velopack;
using Velopack.Sources;

namespace LunaApp.Services;

public class UpdateService
{
    private readonly UpdateManager _updateManager;
    private UpdateInfo? _pendingUpdate;
    
    public event EventHandler<UpdateInfo>? UpdateAvailable;
    public event EventHandler<int>? DownloadProgress;
    public event EventHandler? UpdateReady;
    
    public UpdateService()
    {
        // Configure GitHub Releases as the update source
        var source = new GithubSource(
            repoUrl: "https://github.com/shakedex/LunaApp",
            accessToken: null,  // Public repo, no token needed
            prerelease: false   // Only stable releases
        );
        
        _updateManager = new UpdateManager(source);
    }
    
    /// <summary>
    /// Check for updates. Call on app startup or periodically.
    /// </summary>
    public async Task<bool> CheckForUpdatesAsync()
    {
        try
        {
            if (!_updateManager.IsInstalled)
            {
                Log.Debug("App not installed via Velopack, skipping update check");
                return false;
            }
            
            Log.Information("Checking for updates...");
            _pendingUpdate = await _updateManager.CheckForUpdatesAsync();
            
            if (_pendingUpdate != null)
            {
                Log.Information("Update available: {Version}", _pendingUpdate.TargetFullRelease.Version);
                UpdateAvailable?.Invoke(this, _pendingUpdate);
                return true;
            }
            
            Log.Information("App is up to date");
            return false;
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to check for updates");
            return false;
        }
    }
    
    /// <summary>
    /// Download the pending update. Call after user confirms.
    /// </summary>
    public async Task DownloadUpdateAsync()
    {
        if (_pendingUpdate == null)
            throw new InvalidOperationException("No pending update. Call CheckForUpdatesAsync first.");
        
        Log.Information("Downloading update {Version}...", _pendingUpdate.TargetFullRelease.Version);
        
        await _updateManager.DownloadUpdatesAsync(
            _pendingUpdate,
            progress => DownloadProgress?.Invoke(this, progress)
        );
        
        Log.Information("Update downloaded successfully");
        UpdateReady?.Invoke(this, EventArgs.Empty);
    }
    
    /// <summary>
    /// Apply update and restart app. Call when user is ready.
    /// </summary>
    public void ApplyUpdateAndRestart()
    {
        if (_pendingUpdate == null)
            throw new InvalidOperationException("No pending update.");
        
        Log.Information("Applying update and restarting...");
        _updateManager.ApplyUpdatesAndRestart(_pendingUpdate);
    }
    
    /// <summary>
    /// Get current installed version.
    /// </summary>
    public string? GetCurrentVersion()
    {
        return _updateManager.IsInstalled 
            ? _updateManager.CurrentVersion?.ToString() 
            : null;
    }
}
```

### Step 2: Integrate with App Startup

Modify `App.axaml.cs` to check for updates on startup:

```csharp
// App.axaml.cs
using LunaApp.Services;

public partial class App : Application
{
    public static UpdateService? UpdateService { get; private set; }
    
    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            desktop.MainWindow = new MainWindow
            {
                DataContext = new MainWindowViewModel()
            };
            
            // Initialize update service and check for updates
            InitializeUpdates();
        }
        
        base.OnFrameworkInitializationCompleted();
    }
    
    private async void InitializeUpdates()
    {
        UpdateService = new UpdateService();
        
        // Check for updates in background (non-blocking)
        try
        {
            bool hasUpdate = await UpdateService.CheckForUpdatesAsync();
            if (hasUpdate)
            {
                // Notify the main window or show a toast
                // Implementation depends on your UI framework
            }
        }
        catch (Exception ex)
        {
            Serilog.Log.Warning(ex, "Update check failed");
        }
    }
}
```

### Step 3: Create Update UI Component

Add an update notification to your main window:

```xml
<!-- Views/MainWindow.axaml -->
<!-- Add inside your main layout -->
<Border x:Name="UpdateBanner" 
        IsVisible="{Binding HasUpdateAvailable}"
        Background="#2196F3" 
        Padding="12">
    <StackPanel Orientation="Horizontal" Spacing="12">
        <TextBlock Text="A new version is available!" 
                   Foreground="White" 
                   VerticalAlignment="Center"/>
        <Button Content="Download Now" 
                Command="{Binding DownloadUpdateCommand}"/>
        <Button Content="Remind Later" 
                Command="{Binding DismissUpdateCommand}"/>
    </StackPanel>
</Border>
```

### Step 4: ViewModel Integration

```csharp
// ViewModels/MainWindowViewModel.cs
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

public partial class MainWindowViewModel : ViewModelBase
{
    [ObservableProperty]
    private bool _hasUpdateAvailable;
    
    [ObservableProperty]
    private int _downloadProgress;
    
    [ObservableProperty]
    private string _updateVersion = "";
    
    public MainWindowViewModel()
    {
        // Subscribe to update events
        if (App.UpdateService != null)
        {
            App.UpdateService.UpdateAvailable += OnUpdateAvailable;
            App.UpdateService.DownloadProgress += OnDownloadProgress;
            App.UpdateService.UpdateReady += OnUpdateReady;
        }
    }
    
    private void OnUpdateAvailable(object? sender, UpdateInfo info)
    {
        HasUpdateAvailable = true;
        UpdateVersion = info.TargetFullRelease.Version.ToString();
    }
    
    private void OnDownloadProgress(object? sender, int progress)
    {
        DownloadProgress = progress;
    }
    
    private void OnUpdateReady(object? sender, EventArgs e)
    {
        // Show "Restart to Apply" button
    }
    
    [RelayCommand]
    private async Task DownloadUpdate()
    {
        await App.UpdateService!.DownloadUpdateAsync();
    }
    
    [RelayCommand]
    private void ApplyUpdate()
    {
        App.UpdateService!.ApplyUpdateAndRestart();
    }
    
    [RelayCommand]
    private void DismissUpdate()
    {
        HasUpdateAvailable = false;
    }
}
```

## Update Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      APP STARTUP                            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              VelopackApp.Build().Run()                      │
│   (Handles install hooks, update restart finalization)      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│           UpdateService.CheckForUpdatesAsync()              │
│   - Queries: github.com/shakedex/LunaApp/releases           │
│   - Compares versions via RELEASES file                     │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
      [No Update]                     [Update Available]
              │                               │
              ▼                               ▼
┌───────────────────────┐     ┌───────────────────────────────┐
│   Continue normally   │     │  Show update banner to user   │
└───────────────────────┘     └───────────────────────────────┘
                                              │
                                 User clicks "Download"
                                              │
                                              ▼
                              ┌───────────────────────────────┐
                              │  DownloadUpdatesAsync()       │
                              │  (Delta download if possible) │
                              └───────────────────────────────┘
                                              │
                                 User clicks "Restart"
                                              │
                                              ▼
                              ┌───────────────────────────────┐
                              │  ApplyUpdatesAndRestart()     │
                              │  - App closes                 │
                              │  - Files replaced             │
                              │  - App relaunched             │
                              └───────────────────────────────┘
```

## GitHub Releases Structure

When the release workflow runs, Velopack creates these files:

```
releases/
├── win-x64/
│   ├── RELEASES                          # Version manifest
│   ├── Luna-1.0.0-win-x64-full.nupkg    # Full package
│   ├── Luna-1.0.0-win-x64-Setup.exe     # Installer
│   └── Luna-1.0.0-1.0.1-win-x64-delta.nupkg  # Delta (if upgrading)
│
└── osx-arm64/
    ├── RELEASES
    ├── Luna-1.0.0-osx-arm64-full.nupkg
    └── Luna-1.0.0-osx-arm64.dmg
```

The `RELEASES` file is a text manifest listing all available versions. The UpdateManager reads this to determine if updates are available.

## Testing Updates Locally

### 1. Build Two Versions

```powershell
# Build v1.0.0
.\build.ps1 -Version 1.0.0 -Runtime win-x64

# Build v1.0.1
.\build.ps1 -Version 1.0.1 -Runtime win-x64
```

### 2. Create Local Update Source

```csharp
// For testing, use a local folder instead of GitHub
var source = new SimpleFileSource("C:\\Luna-Releases");
var updateManager = new UpdateManager(source);
```

### 3. Install v1.0.0 and Test

1. Run the `Luna-1.0.0-win-x64-Setup.exe` installer
2. Copy v1.0.1 release files to your local source folder
3. Launch Luna - it should detect v1.0.1 available

## Advanced Configuration

### Pre-release Updates

```csharp
var source = new GithubSource(
    repoUrl: "https://github.com/shakedex/LunaApp",
    accessToken: null,
    prerelease: true  // Include beta/alpha releases
);
```

### Private Repository

```csharp
var source = new GithubSource(
    repoUrl: "https://github.com/shakedex/LunaApp",
    accessToken: Environment.GetEnvironmentVariable("GITHUB_TOKEN"),
    prerelease: false
);
```

### Custom Update Server

```csharp
// Use any HTTP server hosting the release files
var source = new SimpleWebSource("https://updates.myapp.com/luna/");
```

### Silent Background Updates

```csharp
public async Task SilentUpdateAsync()
{
    var update = await _updateManager.CheckForUpdatesAsync();
    if (update != null)
    {
        await _updateManager.DownloadUpdatesAsync(update);
        // Apply on next app restart instead of forcing restart now
        _updateManager.ApplyUpdatesAndExit(update);
    }
}
```

## Troubleshooting

### Update Check Returns Null (No Updates Found)

1. Verify the app was installed via Velopack (not `dotnet run`)
2. Check `_updateManager.IsInstalled` returns `true`
3. Verify RELEASES file exists in GitHub Release assets
4. Check network connectivity to GitHub

### "Not Installed" When Running Locally

Velopack only works when the app is installed via the Setup installer. During development:

```csharp
if (!_updateManager.IsInstalled)
{
    Log.Debug("Running in development mode, updates disabled");
    return;
}
```

### Delta Updates Not Working

Delta updates require:
1. Previous version's full package available
2. Both versions built with same Velopack CLI version
3. `RELEASES` file listing both versions

### macOS Gatekeeper Blocks Updated App

After update, users may need to re-run:
```bash
xattr -cr /Applications/Luna.app
```

Consider showing this instruction in the app after an update completes on macOS.

## Key Files Reference

| File | Purpose |
|------|---------|
| [Program.cs](../Program.cs#L33) | Velopack bootstrap (`VelopackApp.Build().Run()`) |
| [LunaApp.csproj](../LunaApp.csproj#L62) | Velopack NuGet package reference |
| [build.ps1](../build.ps1) | `vpk pack` commands for installer creation |
| [.github/workflows/release.yml](../.github/workflows/release.yml) | CI/CD release pipeline |

## Velopack CLI Commands

```bash
# Install CLI globally
dotnet tool install -g vpk

# Create installer package
vpk pack \
  --packId "Luna" \
  --packVersion 1.0.0 \
  --packDir ./publish/win-x64 \
  --mainExe Luna.exe \
  --outputDir ./releases

# Download updates (for update server setup)
vpk download github \
  --repoUrl https://github.com/shakedex/LunaApp \
  --outputDir ./local-releases
```

## Summary

To add auto-updates to Luna:

1. **UpdateService** handles update lifecycle (check → download → apply)
2. **GithubSource** points to `shakedex/LunaApp` releases
3. **UI integration** shows update availability to users
4. **Release workflow** publishes both installer and `RELEASES` manifest
5. **VelopackApp.Build().Run()** in Program.cs handles restart hooks

The system is designed to be non-intrusive: updates are checked silently, users are notified, and they choose when to apply.
