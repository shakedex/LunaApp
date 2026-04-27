using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Models;
using Serilog;

namespace LunaApp.ViewModels;

/// <summary>
/// Clip-level actions exposed to the UI context menu (right-click on a clip row):
/// reveal in explorer / finder, copy path, copy filename, copy metadata as text.
/// </summary>
public partial class MainWindowViewModel
{
    [RelayCommand]
    private void RevealClipInExplorer(CameraClip? clip)
    {
        if (clip is null || string.IsNullOrEmpty(clip.FilePath)) return;

        try
        {
            if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = "explorer.exe",
                    Arguments = $"/select,\"{clip.FilePath}\"",
                    UseShellExecute = true
                });
            }
            else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = "open",
                    Arguments = $"-R \"{clip.FilePath}\"",
                    UseShellExecute = false
                });
            }
            else
            {
                // Linux — open the containing directory; no cross-distro "reveal this file" primitive.
                var dir = Path.GetDirectoryName(clip.FilePath);
                if (!string.IsNullOrEmpty(dir))
                {
                    Process.Start(new ProcessStartInfo
                    {
                        FileName = "xdg-open",
                        Arguments = $"\"{dir}\"",
                        UseShellExecute = false
                    });
                }
            }
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to reveal {File} in file manager", clip.FilePath);
        }
    }

    [RelayCommand]
    private async Task CopyClipPathAsync(CameraClip? clip)
    {
        if (clip is null || Clipboard is null) return;
        await Clipboard.SetTextAsync(clip.FilePath);
    }

    [RelayCommand]
    private async Task CopyClipFilenameAsync(CameraClip? clip)
    {
        if (clip is null || Clipboard is null) return;
        await Clipboard.SetTextAsync(clip.FileName);
    }

    [RelayCommand]
    private async Task CopyClipMetadataAsync(CameraClip? clip)
    {
        if (clip is null || Clipboard is null) return;

        var sb = new StringBuilder();
        sb.AppendLine($"File:        {clip.FileName}");
        sb.AppendLine($"Path:        {clip.FilePath}");
        if (!string.IsNullOrEmpty(clip.Timecode)) sb.AppendLine($"Timecode:    {clip.Timecode}");
        sb.AppendLine($"Duration:    {clip.DurationFormatted}");
        sb.AppendLine($"Resolution:  {clip.Resolution}");
        if (!string.IsNullOrEmpty(clip.Codec))       sb.AppendLine($"Codec:       {clip.Codec}");
        if (clip.FrameRate > 0)                      sb.AppendLine($"Frame rate:  {clip.FrameRate:0.##} fps");
        if (!string.IsNullOrEmpty(clip.CameraModel)) sb.AppendLine($"Camera:      {clip.CameraModel}");
        if (clip.Iso.HasValue)                       sb.AppendLine($"ISO:         {clip.Iso}");
        if (clip.WhiteBalance.HasValue)              sb.AppendLine($"White bal.:  {clip.WhiteBalance}K");
        if (!string.IsNullOrEmpty(clip.Lens))        sb.AppendLine($"Lens:        {clip.Lens}");
        if (!string.IsNullOrEmpty(clip.FocalLength)) sb.AppendLine($"Focal len:   {clip.FocalLength}");
        if (!string.IsNullOrEmpty(clip.TStop))       sb.AppendLine($"Aperture:    {clip.TStop}");
        if (!string.IsNullOrEmpty(clip.ShutterAngle))sb.AppendLine($"Shutter:     {clip.ShutterAngle}");
        sb.AppendLine($"Size:        {clip.FileSizeFormatted}");

        await Clipboard.SetTextAsync(sb.ToString());
    }
}
