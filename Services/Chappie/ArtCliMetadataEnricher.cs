using LunaApp.Models;
using LunaApp.Services.CameraSupport;
using Serilog;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Enriches ARRI clips by shelling out to <c>art-cmd</c> and parsing the
/// <c>metadata.json</c> it produces. This is the only path to per-clip
/// sensor data (ISO, shutter, lens, look, etc.) for ARRI MXF — the
/// container-level enricher (<see cref="ArriQuickTimeEnricher"/>) only
/// gets manufacturer + model from MXF Identification Descriptors.
///
/// Runs <em>after</em> <see cref="ArriQuickTimeEnricher"/> (priority 130 vs
/// 120) so the QuickTime path's authoritative MOV-side data lands first;
/// ART CLI fills in the gaps and overrides MXF-only-known fields.
///
/// Skips silently when ART CLI isn't installed (managed install runs in
/// <see cref="ArtCliInstaller"/>) or when the file isn't ARRI-branded by an
/// earlier enricher — saves a process spawn + 2-3s timeout for every other
/// clip in the project.
/// </summary>
public sealed class ArtCliMetadataEnricher : IMetadataEnricher
{
    private static readonly TimeSpan ArtCliTimeout = TimeSpan.FromSeconds(45);

    private static readonly HashSet<string> Extensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mxf", ".mov", ".mp4", ".ari", ".m4v",
    };

    private readonly ArtCliLocator _locator;

    public ArtCliMetadataEnricher(ArtCliLocator locator)
    {
        _locator = locator;
    }

    public int Priority => 130;

    public bool CanEnrich(string filePath) =>
        Extensions.Contains(Path.GetExtension(filePath))
        && _locator.Resolve() is not null;

    public async Task EnrichAsync(CameraClip clip, CancellationToken cancellationToken)
    {
        // Only run when an earlier enricher has already established this is
        // ARRI footage — running art-cmd against a Sony / Blackmagic file is
        // wasted process spawn and noisy stderr.
        if (!"ARRI".Equals(clip.CameraManufacturer, StringComparison.OrdinalIgnoreCase))
            return;

        var install = _locator.Resolve();
        if (install is null) return;

        Log.Debug("Running ART CLI metadata extraction on {File}", clip.FileName);

        var metadataJson = await ArriCameraSupport.RunArtCliAsync(
            install.ExecutablePath, clip.FilePath, ArtCliTimeout, cancellationToken);

        if (metadataJson is null) return;

        try
        {
            await ArriCameraSupport.ParseArriMetadataAsync(metadataJson, clip);

            Log.Information("ART CLI enrichment for {File}: {Model} · ISO {Iso} · {Shutter} · {Lens}",
                clip.FileName, clip.CameraModel, clip.Iso, clip.ShutterAngle ?? clip.ShutterSpeed, clip.Lens);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "ART CLI metadata.json parse failed for {File}", clip.FileName);
        }
        finally
        {
            // Clean up the per-call temp dir RunArtCliAsync created.
            try
            {
                var tempDir = Path.GetDirectoryName(metadataJson);
                if (!string.IsNullOrEmpty(tempDir) && Directory.Exists(tempDir))
                    Directory.Delete(tempDir, recursive: true);
            }
            catch { /* best effort */ }
        }
    }
}
