using System.Diagnostics;
using System.IO.Compression;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Downloads + launches Sony's RAW Viewer installer. Different shape from
/// <see cref="ArtCliInstaller"/>: ART CLI is "extract zip, use binaries
/// directly" — Sony ships a Windows / macOS installer the user must click
/// through (EULA, install location, progress bar). We download the zip from
/// Sony's pro CDN, verify SHA-256, extract the inner installer, and shell
/// out to it. After it exits we re-probe via <see cref="SonyRawViewerLocator"/>
/// to confirm RAW Viewer landed in the standard install path.
///
/// We never redistribute Sony binaries — the zip pulls fresh from
/// <c>download.pro.sony</c> on demand. URLs and SHA-256s are pinned per
/// platform; bump the table when Sony ships a new version.
/// </summary>
public sealed class SonyRawViewerInstaller
{
    private static readonly Dictionary<string, RawViewerRelease> Releases = new(StringComparer.OrdinalIgnoreCase)
    {
        ["win-x64"] = new(
            Version: "5.3",
            Url: "https://download.pro.sony/26/03/swef7e9837/RAW_Viewer_5_3_win.zip",
            Sha256: "16fb7b44be55cda0335b532ccaab450c9db393f88831120d6fae07434c60b16c",
            DownloadSizeBytes: 185_263_704,
            InstallerNameInZip: "RAW_Viewer_5_3.exe"),

        ["osx-arm64"] = new(
            Version: "5.3",
            Url: "https://download.pro.sony/26/03/swef7e9837/RAW_Viewer_5_3_mac.zip",
            Sha256: "7ba23107b7e9f57d4a6420296e819e08b309c1c6117712c7e6d30ffe51c994b2",
            DownloadSizeBytes: 244_117_166,
            // macOS ships a DMG. We launch it via `open`; macOS mounts it
            // and shows the Finder window with the .app to drag into
            // /Applications. The user proceeds; we wait, re-probe.
            InstallerNameInZip: "RAW_Viewer_5_3.dmg"),
    };

    private readonly SonyRawViewerLocator _locator;
    private readonly HttpClient _http;

    public SonyRawViewerInstaller(SonyRawViewerLocator locator)
    {
        _locator = locator;
        _http = new HttpClient
        {
            Timeout = TimeSpan.FromMinutes(15), // 177 MB on a slow link
        };
        _http.DefaultRequestHeaders.UserAgent.ParseAdd("Luna/1.0");
    }

    public RawViewerRelease? CurrentRelease
    {
        get
        {
            Releases.TryGetValue(CurrentRid(), out var r);
            return r;
        }
    }

    public bool IsSupportedPlatform => CurrentRelease is not null;

    public sealed record InstallResult(bool Success, string? Path, string? Error);

    /// <summary>
    /// Downloads, verifies, extracts, and launches Sony's RAW Viewer
    /// installer. Returns once the installer process exits — success
    /// depends on whether the locator finds <c>rawexporter.exe</c> at a
    /// standard install path afterward (the user could have cancelled the
    /// installer, in which case nothing landed and we return a clear "user
    /// cancelled" message).
    /// </summary>
    public async Task<InstallResult> InstallAsync(IProgress<double>? progress = null, CancellationToken ct = default)
    {
        var rid = CurrentRid();
        if (!Releases.TryGetValue(rid, out var release))
            return new InstallResult(false, null, $"No Sony RAW Viewer release pinned for platform {rid}");

        // Already installed? Skip the whole flow.
        var existing = _locator.Resolve();
        if (existing is not null && existing.Version.Contains(release.Version, StringComparison.Ordinal))
        {
            Log.Information("Sony RAW Viewer {Version} already installed at {Path}", release.Version, existing.ExecutablePath);
            progress?.Report(1.0);
            return new InstallResult(true, existing.ExecutablePath, null);
        }

        var tempDir = Path.Combine(Path.GetTempPath(), $"luna_sony_rawviewer_{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);
        var tempZip = Path.Combine(tempDir, "rawviewer.zip");
        var extractedInstaller = Path.Combine(tempDir, release.InstallerNameInZip);

        try
        {
            // 1. Download. 80% of the progress bar.
            Log.Information("Downloading Sony RAW Viewer {Version} from {Url}", release.Version, release.Url);
            await DownloadAsync(release.Url, tempZip, release.DownloadSizeBytes, progress, ct);

            // 2. Verify SHA-256 if pinned (macOS sha left empty until first pass).
            if (!string.IsNullOrEmpty(release.Sha256))
            {
                var actual = await ComputeSha256Async(tempZip, ct);
                if (!actual.Equals(release.Sha256, StringComparison.OrdinalIgnoreCase))
                {
                    return new InstallResult(false, null,
                        $"SHA-256 mismatch on Sony RAW Viewer download. Expected {release.Sha256}, got {actual}.");
                }
            }
            progress?.Report(0.85);

            // 3. Extract the installer.
            ZipFile.ExtractToDirectory(tempZip, tempDir, overwriteFiles: true);
            if (!File.Exists(extractedInstaller))
            {
                return new InstallResult(false, null,
                    $"Zip extracted but {release.InstallerNameInZip} wasn't found inside.");
            }
            progress?.Report(0.9);

            // 4. Launch the installer and return as soon as the process is
            //    started. We deliberately do NOT WaitForExit: Sony's
            //    installer is an InstallShield-style stub that spawns an
            //    elevated child, then the parent exits immediately —
            //    waiting on the parent reports "done" while the real
            //    installer is still running. The Settings UI hands off to
            //    a "Detect" button that re-probes the locator on demand;
            //    this is the only honest contract we can offer.
            Log.Information("Launching Sony RAW Viewer installer: {Installer}", extractedInstaller);
            LaunchInstaller(extractedInstaller);
            progress?.Report(1.0);
            return new InstallResult(true, null, null);
        }
        catch (OperationCanceledException)
        {
            return new InstallResult(false, null, "Install was cancelled");
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Sony RAW Viewer install failed");
            return new InstallResult(false, null, ex.Message);
        }
        finally
        {
            // Best-effort cleanup. The installer copies what it needs into
            // Program Files; we don't need the temp zip / extracted .exe.
            try { Directory.Delete(tempDir, recursive: true); } catch { /* ignore */ }
        }
    }

    private async Task DownloadAsync(string url, string destPath, long expectedSize, IProgress<double>? progress, CancellationToken ct)
    {
        using var response = await _http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
        response.EnsureSuccessStatusCode();
        var totalBytes = response.Content.Headers.ContentLength ?? expectedSize;

        await using var contentStream = await response.Content.ReadAsStreamAsync(ct);
        await using var fileStream = File.Create(destPath);

        var buffer = new byte[81920];
        long downloaded = 0;
        int read;
        while ((read = await contentStream.ReadAsync(buffer, ct)) > 0)
        {
            await fileStream.WriteAsync(buffer.AsMemory(0, read), ct);
            downloaded += read;
            // Reserve last 20% of the bar for verify + extract + installer launch.
            if (totalBytes > 0)
                progress?.Report(Math.Min(0.80, (double)downloaded / totalBytes * 0.80));
        }
    }

    private static async Task<string> ComputeSha256Async(string path, CancellationToken ct)
    {
        await using var stream = File.OpenRead(path);
        var hash = await SHA256.HashDataAsync(stream, ct);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    /// <summary>
    /// Launches the installer and returns. We deliberately do not wait —
    /// Sony's installer (and most Windows InstallShield-style installers)
    /// fork an elevated child and exit the parent immediately, so a
    /// <c>WaitForExit</c> reports "done" while the real installer is still
    /// running. Settings re-probes on demand via a Detect button.
    /// </summary>
    private static void LaunchInstaller(string installerPath)
    {
        if (OperatingSystem.IsMacOS())
        {
            // 'open' mounts the DMG and exits; the user finishes the drag
            // manually. No way to know when they're done short of probing.
            var openPsi = new ProcessStartInfo
            {
                FileName = "open",
                ArgumentList = { installerPath },
                UseShellExecute = false,
            };
            using var _ = Process.Start(openPsi)
                ?? throw new InvalidOperationException("Failed to invoke 'open' on the DMG.");
            return;
        }

        // Windows: shell-execute so UAC prompts and the installer GUI shows.
        var psi = new ProcessStartInfo
        {
            FileName = installerPath,
            UseShellExecute = true,
            Verb = "runas",
        };

        using var proc = Process.Start(psi)
            ?? throw new InvalidOperationException("Failed to start the Sony RAW Viewer installer.");
    }

    private static string CurrentRid()
    {
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows)) return "win-x64";
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX)) return "osx-arm64";
        return "linux-x64";
    }
}

public sealed record RawViewerRelease(
    string Version,
    string Url,
    string Sha256,
    long DownloadSizeBytes,
    string InstallerNameInZip);
