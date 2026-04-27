using LunaApp.Models;

namespace LunaApp.Services.Chappie;

/// <summary>
/// Vendor-specific enricher that augments a baseline CameraClip with deeper metadata
/// (camera model, ISO, lens, shutter, etc.) when it recognizes the format.
/// Enrichers run after the primary extractor and may overwrite fields with
/// higher-confidence values, but should not clobber non-empty fields with empty ones.
/// </summary>
public interface IMetadataEnricher
{
    /// <summary>Higher values run later and can override earlier enrichers.</summary>
    int Priority { get; }

    bool CanEnrich(string filePath);

    Task EnrichAsync(CameraClip clip, CancellationToken cancellationToken);
}
