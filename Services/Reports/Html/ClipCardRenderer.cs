using System.Text;
using System.Web;
using LunaApp.Models;

namespace LunaApp.Services.Reports.Html;

/// <summary>
/// Renders one <see cref="CameraClip"/> as an HTML clip card: an interactive
/// "one big frame + picker strip" thumbnail viewer, followed by a two-column
/// metadata table. The viewer is pure HTML/CSS (radio inputs + :checked rules),
/// so it works offline with no JavaScript. In print, it collapses to three
/// side-by-side stills via the stylesheet's <c>@media print</c> overrides.
/// </summary>
internal static class ClipCardRenderer
{
    public static string Render(CameraClip clip, int index)
    {
        var sb = new StringBuilder();

        sb.AppendLine("<article class=\"clip-card\">");
        AppendHead(sb, clip);
        sb.AppendLine("    <div class=\"clip-body\">");
        AppendViewer(sb, clip, index);
        AppendMeta(sb, clip);
        sb.AppendLine("    </div>");
        sb.AppendLine("</article>");

        return sb.ToString();
    }

    private static void AppendHead(StringBuilder sb, CameraClip clip)
    {
        sb.AppendLine("    <div class=\"clip-head\">");
        sb.AppendLine($"        <span class=\"name\">{HttpUtility.HtmlEncode(clip.FileName)}</span>");
        if (!string.IsNullOrEmpty(clip.Timecode))
            sb.AppendLine($"        <span class=\"tc\">{HttpUtility.HtmlEncode(clip.Timecode)}</span>");
        sb.AppendLine("    </div>");
    }

    private static void AppendViewer(StringBuilder sb, CameraClip clip, int clipIndex)
    {
        if (clip.Thumbnails.Count == 0)
        {
            sb.AppendLine("        <div class=\"thumbs-empty\">No preview frames available for this clip.</div>");
            return;
        }

        var group = $"v{clipIndex}";

        sb.AppendLine("        <div class=\"viewer\">");

        // Radio inputs (hidden). First one is checked by default.
        for (var i = 0; i < clip.Thumbnails.Count; i++)
        {
            var id = $"{group}_{i}";
            var checkedAttr = i == 0 ? " checked" : string.Empty;
            sb.AppendLine($"            <input type=\"radio\" name=\"{group}\" id=\"{id}\"{checkedAttr}>");
        }

        // Large stage with overlayed frames.
        sb.AppendLine("            <div class=\"frames\">");
        for (var i = 0; i < clip.Thumbnails.Count; i++)
        {
            var thumb = clip.Thumbnails[i];
            var activeClass = i == 0 ? " active" : string.Empty;
            var tcFallback = thumb.Timecode ?? string.Empty;
            sb.AppendLine($"                <img class=\"frame frame-{i}{activeClass}\" " +
                          $"src=\"{thumb.ImageSource}\" alt=\"Frame at {HttpUtility.HtmlAttributeEncode(tcFallback)}\">");
        }
        // Display the current frame's timecode as an overlay caption
        for (var i = 0; i < clip.Thumbnails.Count; i++)
        {
            var thumb = clip.Thumbnails[i];
            if (string.IsNullOrEmpty(thumb.Timecode)) continue;
            var activeClass = i == 0 ? " active" : string.Empty;
            sb.AppendLine($"                <span class=\"frame-tc frame-tc-{i}{activeClass}\">{HttpUtility.HtmlEncode(thumb.Timecode!)}</span>");
        }
        sb.AppendLine("            </div>");

        // Picker strip of labels.
        sb.AppendLine("            <div class=\"picks\">");
        for (var i = 0; i < clip.Thumbnails.Count; i++)
        {
            var id = $"{group}_{i}";
            var thumb = clip.Thumbnails[i];
            var activeClass = i == 0 ? " active" : string.Empty;
            sb.AppendLine($"                <label class=\"pick pick-{i}{activeClass}\" for=\"{id}\">");
            sb.AppendLine($"                    <img src=\"{thumb.ImageSource}\" alt=\"\">");
            if (!string.IsNullOrEmpty(thumb.Timecode))
                sb.AppendLine($"                    <span class=\"pick-tc\">{HttpUtility.HtmlEncode(thumb.Timecode!)}</span>");
            sb.AppendLine("                </label>");
        }
        sb.AppendLine("            </div>");

        // Per-card :checked→active CSS rules. Local to this card so groups don't leak.
        sb.AppendLine("            <style>");
        for (var i = 0; i < clip.Thumbnails.Count; i++)
        {
            // When this radio is checked, the matching frame, tc, and pick become active.
            sb.AppendLine($"                #{group}_{i}:checked ~ .frames .frame {{ opacity: 0; }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .frames .frame-{i} {{ opacity: 1; }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .frames .frame-tc {{ opacity: 0; }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .frames .frame-tc-{i} {{ opacity: 1; }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .picks .pick {{ border-color: transparent; }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .picks .pick img {{ opacity: .75; }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .picks .pick-{i} {{ border-color: var(--accent); }}");
            sb.AppendLine($"                #{group}_{i}:checked ~ .picks .pick-{i} img {{ opacity: 1; }}");
        }
        sb.AppendLine("            </style>");

        sb.AppendLine("        </div>");
    }

    private static void AppendMeta(StringBuilder sb, CameraClip clip)
    {
        sb.AppendLine("        <div class=\"meta-grid\">");

        var left = new StringBuilder();
        Row(left, "aspect_ratio",  "Resolution",   clip.Resolution);
        Row(left, "videocam",      "Codec",        clip.Codec);
        Row(left, "speed",         "Frame rate",   clip.FrameRate > 0 ? $"{clip.FrameRate:0.##} fps" : null);
        Row(left, "schedule",      "Duration",     clip.DurationFormatted);
        Row(left, "storage",       "Size",         clip.FileSizeFormatted);
        if (!string.IsNullOrEmpty(clip.ColorSpace)) Row(left, "palette", "Color space", clip.ColorSpace);

        var right = new StringBuilder();
        if (!string.IsNullOrEmpty(clip.CameraModel)) Row(right, "videocam",      "Camera",        clip.CameraModel);
        if (clip.Iso.HasValue)                       Row(right, "iso",           "ISO",           clip.Iso!.Value.ToString());
        if (clip.WhiteBalance.HasValue)              Row(right, "wb_sunny",      "White balance", $"{clip.WhiteBalance}K");
        if (!string.IsNullOrEmpty(clip.Lens))        Row(right, "camera",        "Lens",          clip.Lens);
        if (!string.IsNullOrEmpty(clip.FocalLength)) Row(right, "straighten",    "Focal length",  clip.FocalLength);
        if (!string.IsNullOrEmpty(clip.TStop))       Row(right, "camera_roll",   "Aperture",      clip.TStop);
        if (!string.IsNullOrEmpty(clip.ShutterAngle))Row(right, "shutter_speed", "Shutter",       clip.ShutterAngle);
        if (!string.IsNullOrEmpty(clip.ShutterSpeed))Row(right, "shutter_speed", "Shutter speed", clip.ShutterSpeed);
        if (!string.IsNullOrEmpty(clip.Gamma))       Row(right, "tune",          "Gamma",         clip.Gamma);
        if (!string.IsNullOrEmpty(clip.LookName))    Row(right, "style",         "Look",          clip.LookName);
        if (clip.RecordedDate.HasValue)              Row(right, "calendar_today","Recorded",      clip.RecordedDate.Value.ToString("yyyy-MM-dd HH:mm"));

        sb.Append("            <div>").Append(left).AppendLine("</div>");
        sb.Append("            <div>").Append(right).AppendLine("</div>");
        sb.AppendLine("        </div>");
    }

    private static void Row(StringBuilder sb, string icon, string key, string? value)
    {
        if (string.IsNullOrEmpty(value)) return;
        sb.Append("<div class=\"meta-row\"><span class=\"k\">")
          .Append(MaterialIconLibrary.Render(icon))
          .Append(HttpUtility.HtmlEncode(key))
          .Append("</span><span class=\"v\">")
          .Append(HttpUtility.HtmlEncode(value))
          .Append("</span></div>");
    }
}
