using Serilog;
using Velopack;
using Velopack.Sources;

namespace LunaApp.Services;

/// <summary>
/// Handles application auto-updates via Velopack and GitHub Releases.
/// </summary>
public sealed class UpdateService
{
    private readonly UpdateManager _updateManager;
    private UpdateInfo? _pendingUpdate;
    
    /// <summary>
    /// Raised when an update is available.
    /// </summary>
    public event EventHandler<UpdateInfo>? UpdateAvailable;
    
    /// <summary>
    /// Raised during download with progress percentage (0-100).
    /// </summary>
    public event EventHandler<int>? DownloadProgress;
    
    /// <summary>
    /// Raised when update is downloaded and ready to install.
    /// </summary>
    public event EventHandler? UpdateReady;
    
    /// <summary>
    /// Whether the app was installed via Velopack (not running from dotnet run).
    /// </summary>
    public bool IsInstalled => _updateManager.IsInstalled;
    
    /// <summary>
    /// Current installed version, or null if running in dev mode.
    /// </summary>
    public string? CurrentVersion => _updateManager.IsInstalled 
        ? _updateManager.CurrentVersion?.ToString() 
        : null;
    
    /// <summary>
    /// The pending update info, if any.
    /// </summary>
    public UpdateInfo? PendingUpdate => _pendingUpdate;
    
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
    /// Check for updates. Call on app startup or when user requests.
    /// </summary>
    /// <returns>True if an update is available.</returns>
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
    /// Download the pending update.
    /// </summary>
    public async Task DownloadUpdateAsync()
    {
        if (_pendingUpdate == null)
        {
            throw new InvalidOperationException("No pending update. Call CheckForUpdatesAsync first.");
        }
        
        Log.Information("Downloading update {Version}...", _pendingUpdate.TargetFullRelease.Version);
        
        await _updateManager.DownloadUpdatesAsync(
            _pendingUpdate,
            progress => DownloadProgress?.Invoke(this, progress)
        );
        
        Log.Information("Update downloaded successfully");
        UpdateReady?.Invoke(this, EventArgs.Empty);
    }
    
    /// <summary>
    /// Apply update and restart the application.
    /// </summary>
    public void ApplyUpdateAndRestart()
    {
        if (_pendingUpdate == null)
        {
            throw new InvalidOperationException("No pending update.");
        }
        
        Log.Information("Applying update and restarting...");
        _updateManager.ApplyUpdatesAndRestart(_pendingUpdate);
    }
    
    // ============ DEV TESTING METHODS ============
    // These simulate update states for UI testing during development
    
    /// <summary>
    /// DEV: Simulate an available update for UI testing.
    /// </summary>
    public void DevSimulateUpdateAvailable(string version = "99.0.0")
    {
        Log.Debug("DEV: Simulating update available: {Version}", version);
        // We can't create a real UpdateInfo, so we raise a custom event
        // The ViewModel will handle this specially
        DevUpdateAvailable?.Invoke(this, version);
    }
    
    /// <summary>
    /// DEV: Simulate download progress for UI testing.
    /// </summary>
    public void DevSimulateDownloadProgress(int progress)
    {
        Log.Debug("DEV: Simulating download progress: {Progress}%", progress);
        DownloadProgress?.Invoke(this, progress);
    }
    
    /// <summary>
    /// DEV: Simulate update ready for UI testing.
    /// </summary>
    public void DevSimulateUpdateReady()
    {
        Log.Debug("DEV: Simulating update ready");
        UpdateReady?.Invoke(this, EventArgs.Empty);
    }
    
    /// <summary>
    /// DEV: Event for simulated update available (since we can't create real UpdateInfo).
    /// </summary>
    public event EventHandler<string>? DevUpdateAvailable;
}
