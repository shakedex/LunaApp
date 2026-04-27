using System.Diagnostics;
using System.Runtime.InteropServices;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Finds <c>art-cmd</c> (ARRI Reference Tool CLI) on disk. Probe order:
/// <list type="number">
///   <item>Luna's auto-installed copy under <c>%LOCALAPPDATA%\Luna\tools\arri\{rid}\</c>
///         (or the macOS equivalent), populated by <see cref="ArtCliInstaller"/>.</item>
///   <item>A dev-bundled copy next to the running app (<c>tools/arri/{rid}/</c>) — only relevant when running from source.</item>
///   <item>System <c>PATH</c>, in case the user installed it themselves.</item>
/// </list>
/// First hit wins. Returns the resolved binary path + version (parsed from
/// <c>art-cmd --version</c>) or null when nothing's installed yet.
/// </summary>
public sealed class ArtCliLocator
{
    private RawInstall? _cached;

    public sealed record RawInstall(string ExecutablePath, string Version);

    /// <summary>The user-scope dir Luna installs into. Public so the installer + locator share one truth.</summary>
    public static string ManagedInstallRoot
    {
        get
        {
            var basePath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            return Path.Combine(basePath, "Luna", "tools", "arri", CurrentRid());
        }
    }

    public RawInstall? Resolve(bool forceRefresh = false)
    {
        if (!forceRefresh && _cached is not null) return _cached;
        _cached = Probe();
        return _cached;
    }

    /// <summary>Invalidates the cached result — call after the installer completes.</summary>
    public void Invalidate() => _cached = null;

    private static RawInstall? Probe()
    {
        foreach (var candidate in CandidatePaths())
        {
            if (!File.Exists(candidate)) continue;
            var version = ReadVersion(candidate);
            if (version is null)
            {
                Log.Warning("ART CLI found at {Path} but --version probe failed", candidate);
                continue;
            }
            Log.Information("ART CLI detected: art-cmd {Version} at {Path}", version, candidate);
            return new RawInstall(candidate, version);
        }
        Log.Information("ART CLI not detected — ARRIRAW thumbnails and full ARRI MXF metadata unavailable");
        return null;
    }

    private static IEnumerable<string> CandidatePaths()
    {
        var rid = CurrentRid();
        var exeName = RuntimeInformation.IsOSPlatform(OSPlatform.Windows) ? "art-cmd.exe" : "art-cmd";

        // 1. Managed install. We don't know the exact subfolder layout (Sony,
        // ARRI Win, ARRI macOS all differ), so search for art-cmd recursively
        // — bounded because the install dir is small.
        if (Directory.Exists(ManagedInstallRoot))
        {
            foreach (var hit in Directory.EnumerateFiles(ManagedInstallRoot, exeName, SearchOption.AllDirectories))
                yield return hit;
        }

        // 2. Dev-bundled copy next to the running app.
        var devBundled = Path.Combine(AppContext.BaseDirectory, "tools", "arri", rid, exeName);
        if (File.Exists(devBundled)) yield return devBundled;

        // 3. PATH lookup.
        var pathEnv = Environment.GetEnvironmentVariable("PATH");
        if (!string.IsNullOrEmpty(pathEnv))
        {
            foreach (var dir in pathEnv.Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries))
            {
                var candidate = Path.Combine(dir.Trim(), exeName);
                if (File.Exists(candidate)) yield return candidate;
            }
        }
    }

    private static string? ReadVersion(string exePath)
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = exePath,
                Arguments = "--version",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };
            using var proc = Process.Start(psi);
            if (proc is null) return null;
            if (!proc.WaitForExit(5000))
            {
                try { proc.Kill(); } catch { /* best effort */ }
                return null;
            }
            // art-cmd --version prints something like "art-cmd 1.0.0" or similar.
            var stdout = proc.StandardOutput.ReadToEnd().Trim();
            var stderr = proc.StandardError.ReadToEnd().Trim();
            return string.IsNullOrEmpty(stdout) ? (string.IsNullOrEmpty(stderr) ? null : stderr) : stdout;
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "ART CLI --version probe threw");
            return null;
        }
    }

    /// <summary>Maps the running platform to the RID we use under the managed install root.</summary>
    public static string CurrentRid()
    {
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows)) return "win-x64";
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX)) return "osx-arm64";
        return "linux-x64";
    }
}
