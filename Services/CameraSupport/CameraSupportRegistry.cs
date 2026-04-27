using Serilog;

namespace LunaApp.Services.CameraSupport;

/// <summary>
/// Routes files to the first registered <see cref="ICameraSupport"/> that
/// claims them. Proprietary supports are checked before the generic fallback,
/// so a scaffolded (ComingLater) support for .ari still wins over Generic —
/// which is what lets the engine emit a typed "coming later" notice instead
/// of dropping the file into FFmpeg and producing garbage.
/// </summary>
public sealed class CameraSupportRegistry
{
    private readonly IReadOnlyList<ICameraSupport> _all;

    public CameraSupportRegistry(IEnumerable<ICameraSupport> supports)
    {
        // Generic sorts last so specific matches win. Everything else keeps
        // registration order.
        _all = supports
            .OrderBy(s => s.Id == "generic" ? 1 : 0)
            .ToArray();

        Log.Information("Camera support registry: {Count} supports registered ({Ready} ready)",
            _all.Count, _all.Count(s => s.Status is SupportStatus.Ready));

        foreach (var s in _all)
            Log.Debug("  {Id} ({Display}) — {Status}", s.Id, s.DisplayName, s.Status.Summary);
    }

    public IReadOnlyList<ICameraSupport> All => _all;

    public ICameraSupport? ResolveFor(string filePath) =>
        _all.FirstOrDefault(s => s.CanHandle(filePath));
}
