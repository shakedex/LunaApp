using System.Diagnostics;
using System.Runtime.InteropServices;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Finds Sony's <c>rawexporter.exe</c> from a RAW Viewer install. The CLI
/// decodes Sony X-OCN / F55-raw / F65-raw clips that FFmpeg refuses — we
/// shell out to it from <see cref="SonyRawExporterThumbnailService"/>.
///
/// We never redistribute Sony binaries. Detection only.
/// </summary>
public sealed class SonyRawViewerLocator
{
    private readonly Lazy<RawViewerInstall?> _resolved;

    public SonyRawViewerLocator()
    {
        _resolved = new Lazy<RawViewerInstall?>(Probe, isThreadSafe: true);
    }

    public RawViewerInstall? Resolve() => _resolved.Value;

    public sealed record RawViewerInstall(string ExecutablePath, string Version);

    private static RawViewerInstall? Probe()
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            // macOS RAW Viewer ships at /Applications/RAW Viewer.app/Contents/MacOS/
            // — out of scope until we do a Mac test pass.
            Log.Debug("Sony RAW Viewer locator: non-Windows platform, skipping");
            return null;
        }

        foreach (var candidate in CandidatePaths())
        {
            if (!File.Exists(candidate)) continue;

            var version = ReadVersion(candidate);
            if (version is null)
            {
                Log.Warning("Sony RAW Viewer found at {Path} but --version probe failed", candidate);
                continue;
            }

            Log.Information("Sony RAW Viewer detected: rawexporter {Version} at {Path}", version, candidate);
            return new RawViewerInstall(candidate, version);
        }

        Log.Information("Sony RAW Viewer not detected — X-OCN frame extraction unavailable");
        return null;
    }

    private static IEnumerable<string> CandidatePaths()
    {
        // Default install location.
        yield return @"C:\Program Files\Sony\RAW Viewer\rawexporter.exe";

        // 32-bit Program Files in case Sony ever ships a 32-bit build.
        var pf86 = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86);
        if (!string.IsNullOrEmpty(pf86))
            yield return Path.Combine(pf86, "Sony", "RAW Viewer", "rawexporter.exe");
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
            var stdout = proc.StandardOutput.ReadToEnd().Trim();
            // Output shape: "version : 5.0.0.12180"
            var colon = stdout.IndexOf(':');
            return colon >= 0 ? stdout[(colon + 1)..].Trim() : stdout;
        }
        catch (Exception ex)
        {
            Log.Debug(ex, "Sony RAW Viewer locator: --version probe threw");
            return null;
        }
    }
}
