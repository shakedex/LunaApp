using System.IO.Compression;
using System.Net.Http;
using System.Security.Cryptography;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Downloads + extracts the ARRI Reference Tool CLI (<c>art-cmd</c>) into
/// Luna's managed install folder under <c>%LOCALAPPDATA%\Luna\tools\arri\{rid}\</c>.
/// The user never has to touch arri.com manually.
///
/// We don't redistribute ARRI binaries — the installer pulls them from
/// arri.com on demand, verifies SHA-256 against pinned values, and lays them
/// out so <see cref="ArtCliLocator"/> finds <c>art-cmd[.exe]</c>.
///
/// URLs and SHA-256s are pinned per platform / version. When ARRI ships a
/// new version of the CLI, update the <see cref="Releases"/> table.
/// </summary>
public sealed class ArtCliInstaller
{
    /// <summary>Pinned release per RID. Update when ARRI publishes a new ART CMD version.</summary>
    private static readonly Dictionary<string, ArtCliRelease> Releases = new(StringComparer.OrdinalIgnoreCase)
    {
        ["win-x64"] = new(
            Version: "1.0.0",
            Url: "https://www.arri.com/resource/blob/402596/86c25131d88f205cec5d23e1e2339bb1/arrireferencetool-cmd-1-0-0-win-msvc192-x64-data.zip",
            Sha256: "36cd2269877a73ecd2247a4bc50201b4c93f7d0ed80659540b58ba010be5178d",
            DownloadSizeBytes: 33_469_545),

        ["osx-arm64"] = new(
            Version: "1.0.0",
            Url: "https://www.arri.com/resource/blob/402592/d7dbcc61cfe76fcf5522e55e03bca2a4/arrireferencetool-cmd-1-0-0-macos-universal-data.zip",
            Sha256: "546f87f662a51e4ff95e1d8afb23ac5cfb39b9d9fe82f51625bc5376e22a59c6",
            DownloadSizeBytes: 60_000_000),
    };

    private readonly ArtCliLocator _locator;
    private readonly HttpClient _http;

    public ArtCliInstaller(ArtCliLocator locator)
    {
        _locator = locator;
        _http = new HttpClient
        {
            Timeout = TimeSpan.FromMinutes(5),
        };
        _http.DefaultRequestHeaders.UserAgent.ParseAdd("Luna/1.0 (+https://github.com/shakedex/LunaApp)");
    }

    /// <summary>Returns the pinned release info for the current platform, or null if unsupported.</summary>
    public ArtCliRelease? CurrentRelease
    {
        get
        {
            Releases.TryGetValue(ArtCliLocator.CurrentRid(), out var rel);
            return rel;
        }
    }

    /// <summary>True when the current platform has a pinned release we can install.</summary>
    public bool IsSupportedPlatform => CurrentRelease is not null;

    public sealed record InstallResult(bool Success, string? Path, string? Error);

    /// <summary>
    /// Downloads + extracts ART CLI for the current platform. Idempotent: if
    /// it's already installed at the expected version, returns immediately.
    /// Reports progress in 0..1 fractions.
    /// </summary>
    public async Task<InstallResult> InstallAsync(IProgress<double>? progress = null, CancellationToken ct = default)
    {
        var rid = ArtCliLocator.CurrentRid();
        if (!Releases.TryGetValue(rid, out var release))
        {
            return new InstallResult(false, null, $"No ART CLI release pinned for platform {rid}");
        }

        var existing = _locator.Resolve(forceRefresh: true);
        if (existing is not null && existing.Version.Contains(release.Version, StringComparison.Ordinal))
        {
            Log.Information("ART CLI {Version} already installed at {Path}", release.Version, existing.ExecutablePath);
            progress?.Report(1.0);
            return new InstallResult(true, existing.ExecutablePath, null);
        }

        var installRoot = ArtCliLocator.ManagedInstallRoot;
        Directory.CreateDirectory(installRoot);

        var tempZip = Path.Combine(Path.GetTempPath(), $"luna_art_cli_{Guid.NewGuid():N}.zip");

        try
        {
            // 1. Download with progress.
            Log.Information("Downloading ART CLI {Version} from {Url}", release.Version, release.Url);
            await DownloadWithProgressAsync(release.Url, tempZip, release.DownloadSizeBytes, progress, ct);

            // 2. Verify SHA-256 against the pinned hash.
            var actualHash = await ComputeSha256Async(tempZip, ct);
            if (!actualHash.Equals(release.Sha256, StringComparison.OrdinalIgnoreCase))
            {
                return new InstallResult(false, null,
                    $"SHA-256 mismatch. Expected {release.Sha256}, got {actualHash}. Aborting install.");
            }
            Log.Information("ART CLI archive verified ({Sha} bytes={Bytes})", actualHash[..16], new FileInfo(tempZip).Length);

            // 3. Extract — clean the install root first so updates don't leave stale files.
            CleanDirectoryContents(installRoot);
            ZipFile.ExtractToDirectory(tempZip, installRoot, overwriteFiles: true);

            // 4. Set executable permissions on macOS / Linux. Zip extraction
            // on those platforms loses the +x bit.
            if (!OperatingSystem.IsWindows())
                MakeExecutables(installRoot);

            // 5. Re-probe via locator.
            _locator.Invalidate();
            var resolved = _locator.Resolve(forceRefresh: true);
            if (resolved is null)
            {
                return new InstallResult(false, null,
                    $"Extraction succeeded but art-cmd wasn't found under {installRoot}");
            }

            progress?.Report(1.0);
            return new InstallResult(true, resolved.ExecutablePath, null);
        }
        catch (OperationCanceledException)
        {
            return new InstallResult(false, null, "Installation was cancelled");
        }
        catch (Exception ex)
        {
            Log.Error(ex, "ART CLI installation failed");
            return new InstallResult(false, null, ex.Message);
        }
        finally
        {
            try { if (File.Exists(tempZip)) File.Delete(tempZip); } catch { /* best effort */ }
        }
    }

    private async Task DownloadWithProgressAsync(
        string url, string destPath, long expectedSize, IProgress<double>? progress, CancellationToken ct)
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
            // Reserve final 5% for verification + extraction so the progress
            // bar doesn't sit at 100% while we're still working.
            if (totalBytes > 0)
                progress?.Report(Math.Min(0.95, (double)downloaded / totalBytes * 0.95));
        }
    }

    private static async Task<string> ComputeSha256Async(string path, CancellationToken ct)
    {
        await using var stream = File.OpenRead(path);
        var hash = await SHA256.HashDataAsync(stream, ct);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    private static void CleanDirectoryContents(string dir)
    {
        if (!Directory.Exists(dir)) return;
        foreach (var entry in Directory.EnumerateFileSystemEntries(dir))
        {
            try
            {
                if (Directory.Exists(entry)) Directory.Delete(entry, recursive: true);
                else File.Delete(entry);
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Could not remove stale ART CLI artifact at {Path}", entry);
            }
        }
    }

    private static void MakeExecutables(string root)
    {
        // art-cmd, look-builder, and possibly other binaries lose +x on extract
        // on Unix-y platforms. Mark anything in bin/ as executable.
        try
        {
            foreach (var binDir in Directory.EnumerateDirectories(root, "bin", SearchOption.AllDirectories))
                foreach (var f in Directory.EnumerateFiles(binDir))
                    File.SetUnixFileMode(f,
                        UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute |
                        UnixFileMode.GroupRead | UnixFileMode.GroupExecute |
                        UnixFileMode.OtherRead | UnixFileMode.OtherExecute);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Could not set executable bit on extracted ART CLI binaries");
        }
    }
}

public sealed record ArtCliRelease(
    string Version,
    string Url,
    string Sha256,
    long DownloadSizeBytes);
