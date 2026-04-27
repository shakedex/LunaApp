using LunaApp.Models;

namespace LunaApp.Services.CameraSupport;

internal static class CameraSupportHelpers
{
    public static CameraClip CreateUnsupportedClip(string filePath, ICameraSupport support)
    {
        var fileInfo = new FileInfo(filePath);
        var reason = support.Status switch
        {
            SupportStatus.ComingLater cl => cl.RoadmapNote,
            SupportStatus.NotAvailable na => na.Reason,
            _ => "Support is not available on this install"
        };

        return new CameraClip
        {
            FilePath = filePath,
            FileName = fileInfo.Name,
            FileSizeBytes = fileInfo.Exists ? fileInfo.Length : 0,
            Container = fileInfo.Extension.TrimStart('.').ToUpperInvariant(),
            ProcessingState = ClipProcessingState.Unsupported,
            UnsupportedNotice = new UnsupportedFormatNotice(
                CameraSupportId: support.Id,
                DisplayName: support.DisplayName,
                Reason: reason),
        };
    }
}
