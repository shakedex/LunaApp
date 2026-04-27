using System.Text;
using System.Web;
using LunaApp.Models;

namespace LunaApp.Services.Reports.Html;

/// <summary>Builds a single-reel HTML report document.</summary>
internal static class ReelReportBuilder
{
    public static string Build(CameraReel reel, ReportSettings settings)
    {
        var displayTitle = ReportNaming.DisplayTitle(settings);
        var totalBytes = reel.Clips.Sum(c => c.FileSizeBytes);

        var sb = new StringBuilder();
        sb.AppendLine("<!DOCTYPE html>");
        sb.AppendLine("<html lang=\"en\">");
        sb.AppendLine("<head>");
        sb.AppendLine("    <meta charset=\"UTF-8\">");
        sb.AppendLine("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">");
        sb.AppendLine($"    <title>{HttpUtility.HtmlEncode($"{displayTitle} · {reel.DisplayLabel}")}</title>");
        sb.AppendLine(ReportStylesheet.Base(settings.Theme));
        sb.AppendLine("</head>");
        sb.AppendLine("<body>");

        AppendCover(sb, reel, displayTitle, settings);
        AppendStats(sb, reel.ClipCount, reel.TotalDuration, totalBytes, reel);

        sb.AppendLine("<main>");
        for (var i = 0; i < reel.Clips.Count; i++)
            sb.AppendLine(ClipCardRenderer.Render(reel.Clips[i], i));
        sb.AppendLine("</main>");

        sb.AppendLine(HtmlDocumentParts.Footer());
        sb.AppendLine("</body>");
        sb.AppendLine("</html>");

        return sb.ToString();
    }

    private static void AppendCover(StringBuilder sb, CameraReel reel, string displayTitle, ReportSettings settings)
    {
        sb.AppendLine("<header class=\"cover\">");
        sb.AppendLine("    <div class=\"cover-inner\">");
        sb.AppendLine("        <div>");
        sb.AppendLine($"            <div class=\"eyebrow\">Camera Report · {HttpUtility.HtmlEncode(reel.DisplayLabel)}</div>");
        sb.AppendLine($"            <h1>{HttpUtility.HtmlEncode(displayTitle)}</h1>");

        var meta = new List<string>();
        if (!string.IsNullOrEmpty(settings.ProductionCompany)) meta.Add(HttpUtility.HtmlEncode(settings.ProductionCompany));
        if (!string.IsNullOrEmpty(settings.DitName))           meta.Add("DIT · " + HttpUtility.HtmlEncode(settings.DitName));
        if (!string.IsNullOrEmpty(settings.Director))          meta.Add("Director · " + HttpUtility.HtmlEncode(settings.Director));
        if (!string.IsNullOrEmpty(settings.Dp))                meta.Add("DP · " + HttpUtility.HtmlEncode(settings.Dp));
        meta.Add(DateTime.Now.ToString("MMMM d, yyyy"));

        sb.Append("            <div class=\"meta\">");
        for (var i = 0; i < meta.Count; i++)
        {
            if (i > 0) sb.Append("<span class=\"dot\">·</span>");
            sb.Append("<span>").Append(meta[i]).Append("</span>");
        }
        sb.AppendLine("</div>");

        sb.AppendLine("        </div>");
        AppendLogoIfAny(sb, settings);
        sb.AppendLine("    </div>");
        sb.AppendLine("</header>");
    }

    private static void AppendLogoIfAny(StringBuilder sb, ReportSettings settings)
    {
        if (!settings.HasLogo) return;
        var src = !string.IsNullOrEmpty(settings.LogoBase64)
            ? $"data:image/png;base64,{settings.LogoBase64}"
            : settings.LogoPath ?? string.Empty;
        if (string.IsNullOrEmpty(src)) return;
        sb.AppendLine($"        <img class=\"logo\" src=\"{HttpUtility.HtmlAttributeEncode(src)}\" alt=\"Production logo\">");
    }

    private static void AppendStats(StringBuilder sb, int clipCount, TimeSpan duration, long totalBytes, CameraReel reel)
    {
        sb.AppendLine("<section class=\"stats\">");
        sb.AppendLine("    <div class=\"stats-inner\">");
        Stat(sb, clipCount.ToString(), clipCount == 1 ? "Clip" : "Clips");
        Stat(sb, duration.ToString(@"hh\:mm\:ss"), "Total duration");
        Stat(sb, FormatBytes(totalBytes), "Total size");
        if (!string.IsNullOrEmpty(reel.CameraName)) Stat(sb, reel.CameraName, "Camera");
        sb.AppendLine("    </div>");
        sb.AppendLine("</section>");
    }

    private static void Stat(StringBuilder sb, string number, string label)
    {
        sb.AppendLine("        <div class=\"stat\">");
        sb.AppendLine($"            <span class=\"num\">{HttpUtility.HtmlEncode(number)}</span>");
        sb.AppendLine($"            <span class=\"lbl\">{HttpUtility.HtmlEncode(label)}</span>");
        sb.AppendLine("        </div>");
    }

    private static string FormatBytes(long bytes)
    {
        string[] units = ["B", "KB", "MB", "GB", "TB"];
        double size = bytes;
        var order = 0;
        while (size >= 1024 && order < units.Length - 1)
        {
            order++;
            size /= 1024;
        }
        return $"{size:0.##} {units[order]}";
    }
}
