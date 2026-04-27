using LunaApp.Models;
using LunaApp.Services.CameraSupport;

namespace LunaApp.Tests.Fakes;

internal sealed class FakeCameraSupport : ICameraSupport
{
    public required string Id { get; init; }
    public required string DisplayName { get; init; }
    public IReadOnlySet<string> HandledExtensions { get; init; } = new HashSet<string>();
    public required SupportStatus Status { get; init; }
    public bool CanHandle(string filePath) => false;
    public Task<CameraClip> ProcessAsync(string filePath, bool extractThumbnails, int thumbnailCount, int thumbnailWidth, CancellationToken cancellationToken)
        => throw new NotImplementedException();
}
