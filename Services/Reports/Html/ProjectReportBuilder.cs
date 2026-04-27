using System.Text;
using System.Web;
using LunaApp.Models;

namespace LunaApp.Services.Reports.Html;

/// <summary>Builds the unified multi-reel project HTML report.</summary>
internal static class ProjectReportBuilder
{
    public static string Build(IReadOnlyList<CameraReel> reels, ReportSettings settings)
    {
        var projectName = string.IsNullOrWhiteSpace(settings.ProjectName) ? "Camera Report" : settings.ProjectName;
        var totalClips = reels.Sum(r => r.Clips.Count);
        var totalDuration = TimeSpan.FromTicks(reels.Sum(r => r.TotalDuration.Ticks));
        var totalBytes = reels.SelectMany(r => r.Clips).Sum(c => c.FileSizeBytes);

        var sb = new StringBuilder();
        sb.AppendLine("<!DOCTYPE html>");
        sb.AppendLine("<html lang=\"en\">");
        sb.AppendLine("<head>");
        sb.AppendLine("    <meta charset=\"UTF-8\">");
        sb.AppendLine("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">");
        sb.AppendLine($"    <title>{HttpUtility.HtmlEncode($"{projectName} · Camera Report")}</title>");
        sb.AppendLine(ReportStylesheet.Base(settings.Theme));
        sb.AppendLine(ReportStylesheet.Project);
        sb.AppendLine("</head>");
        sb.AppendLine("<body>");

        AppendCover(sb, projectName, settings);
        AppendStats(sb, reels.Count, totalClips, totalDuration, totalBytes);
        if (reels.Count > 1) AppendToc(sb, reels);

        sb.AppendLine("<main>");
        var clipCursor = 0;
        foreach (var reel in reels)
            AppendReelSection(sb, reel, ref clipCursor);
        sb.AppendLine("</main>");

        sb.AppendLine(HtmlDocumentParts.Footer());
        sb.AppendLine("</body>");
        sb.AppendLine("</html>");

        return sb.ToString();
    }

    private static void AppendCover(StringBuilder sb, string projectName, ReportSettings settings)
    {
        sb.AppendLine("<header class=\"cover\">");
        sb.AppendLine("    <div class=\"cover-inner\">");
        sb.AppendLine("        <div>");
        sb.AppendLine("            <div class=\"eyebrow\">Camera Report</div>");
        sb.AppendLine($"            <h1>{HttpUtility.HtmlEncode(projectName)}</h1>");

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

    private static void AppendStats(StringBuilder sb, int reelCount, int clipCount, TimeSpan duration, long totalBytes)
    {
        sb.AppendLine("<section class=\"stats\">");
        sb.AppendLine("    <div class=\"stats-inner\">");
        Stat(sb, reelCount.ToString(), reelCount == 1 ? "Card" : "Cards");
        Stat(sb, clipCount.ToString(), clipCount == 1 ? "Clip" : "Clips");
        Stat(sb, duration.ToString(@"hh\:mm\:ss"), "Total duration");
        Stat(sb, FormatBytes(totalBytes), "Total size");
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

    private static void AppendToc(StringBuilder sb, IReadOnlyList<CameraReel> reels)
    {
        sb.AppendLine("<nav class=\"toc\">");
        sb.AppendLine("    <div class=\"toc-inner\">");
        sb.AppendLine("        <h2>Reels</h2>");
        sb.AppendLine("        <div class=\"toc-list\">");
        foreach (var reel in reels)
        {
            var id = HttpUtility.HtmlAttributeEncode(reel.DetectedReelId);
            sb.AppendLine($"            <a href=\"#reel-{id}\" class=\"toc-chip\">");
            sb.AppendLine($"                <span>{HttpUtility.HtmlEncode(reel.DisplayLabel)}</span>");
            sb.AppendLine($"                <span class=\"count\">{reel.Clips.Count}</span>");
            sb.AppendLine("            </a>");
        }
        sb.AppendLine("        </div>");
        sb.AppendLine("    </div>");
        sb.AppendLine("</nav>");
    }

    private static void AppendReelSection(StringBuilder sb, CameraReel reel, ref int clipCursor)
    {
        var reelId = HttpUtility.HtmlAttributeEncode(reel.DetectedReelId);

        sb.AppendLine($"<section class=\"reel-section\" id=\"reel-{reelId}\">");
        sb.AppendLine("    <div class=\"reel-head\">");
        sb.AppendLine("        <h2>");
        sb.AppendLine($"            <span class=\"reel-id\">{HttpUtility.HtmlEncode(reel.DisplayLabel)}</span>");
        if (!string.IsNullOrEmpty(reel.CameraName))
            sb.AppendLine($"            <span class=\"cam\">{HttpUtility.HtmlEncode(reel.CameraName)}</span>");
        sb.AppendLine("        </h2>");

        sb.AppendLine("        <div class=\"reel-head-stats\">");
        sb.AppendLine($"            <div class=\"item\"><span class=\"label\">Clips</span><span class=\"value\">{reel.Clips.Count}</span></div>");
        sb.AppendLine($"            <div class=\"item\"><span class=\"label\">Duration</span><span class=\"value\">{reel.TotalDuration:hh\\:mm\\:ss}</span></div>");
        sb.AppendLine("        </div>");
        sb.AppendLine("    </div>");

        foreach (var clip in reel.Clips)
        {
            sb.AppendLine(ClipCardRenderer.Render(clip, clipCursor));
            clipCursor++;
        }

        sb.AppendLine("</section>");
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
