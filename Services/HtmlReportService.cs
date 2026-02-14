using LunaApp.Models;
using Serilog;
using System.Text;
using System.Web;

namespace LunaApp.Services;

/// <summary>
/// Service for generating HTML camera reports.
/// </summary>
public sealed class HtmlReportService
{
    /// <summary>
    /// Generate HTML report for a single reel
    /// </summary>
    public async Task<string> GenerateReportAsync(
        CameraReel reel, 
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return await Task.Run(() => GenerateReport(reel, settings), cancellationToken);
    }
    
    /// <summary>
    /// Generate unified HTML report for multiple reels (project mode)
    /// </summary>
    public async Task<string> GenerateProjectReportAsync(
        IReadOnlyList<CameraReel> reels,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return await Task.Run(() => GenerateProjectReport(reels, settings), cancellationToken);
    }
    
    /// <summary>
    /// Generate and save unified project HTML report
    /// </summary>
    public async Task<string> GenerateAndSaveProjectReportAsync(
        IReadOnlyList<CameraReel> reels,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        var html = await GenerateProjectReportAsync(reels, settings, cancellationToken);
        
        // Create output directory using project name
        var reportName = $"{settings.ProjectName}_{DateTime.Now:yyyy-MM-dd}";
        var reportFolder = Path.Combine(settings.OutputFolder, reportName);
        Directory.CreateDirectory(reportFolder);
        
        // Save HTML
        var htmlPath = Path.Combine(reportFolder, $"{reportName}.html");
        await File.WriteAllTextAsync(htmlPath, html, cancellationToken);
        
        Log.Information("Project HTML report saved: {Path}", htmlPath);
        
        return htmlPath;
    }
    
    /// <summary>
    /// Generate and save HTML report for single reel
    /// </summary>
    public async Task<string> GenerateAndSaveReportAsync(
        CameraReel reel,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        var html = await GenerateReportAsync(reel, settings, cancellationToken);
        
        // Create output directory
        var reportName = settings.GenerateReportName(reel);
        var reportFolder = Path.Combine(settings.OutputFolder, reportName);
        Directory.CreateDirectory(reportFolder);
        
        // Save HTML
        var htmlPath = Path.Combine(reportFolder, $"{reportName}.html");
        await File.WriteAllTextAsync(htmlPath, html, cancellationToken);
        
        Log.Information("HTML report saved: {Path}", htmlPath);
        
        return htmlPath;
    }
    
    private string GenerateReport(CameraReel reel, ReportSettings settings)
    {
        var sb = new StringBuilder();
        
        sb.AppendLine("<!DOCTYPE html>");
        sb.AppendLine("<html lang=\"en\">");
        sb.AppendLine("<head>");
        sb.AppendLine("    <meta charset=\"UTF-8\">");
        sb.AppendLine("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">");
        sb.AppendLine($"    <title>{HttpUtility.HtmlEncode(reel.DisplayLabel)} - Camera Report</title>");
        sb.AppendLine(GenerateStyles());
        sb.AppendLine("</head>");
        sb.AppendLine("<body>");
        
        // Header with branding
        sb.AppendLine(GenerateHeader(reel, settings));
        
        // Summary section
        sb.AppendLine(GenerateSummary(reel));
        
        // Clips section
        sb.AppendLine("<main class=\"clips-container\">");
        foreach (var clip in reel.Clips)
        {
            sb.AppendLine(GenerateClipCard(clip));
        }
        sb.AppendLine("</main>");
        
        // Footer
        sb.AppendLine(GenerateFooter());
        
        sb.AppendLine("</body>");
        sb.AppendLine("</html>");
        
        return sb.ToString();
    }
    
    private string GenerateProjectReport(IReadOnlyList<CameraReel> reels, ReportSettings settings)
    {
        var sb = new StringBuilder();
        var totalClips = reels.Sum(r => r.Clips.Count);
        var totalDuration = TimeSpan.FromTicks(reels.Sum(r => r.TotalDuration.Ticks));
        
        sb.AppendLine("<!DOCTYPE html>");
        sb.AppendLine("<html lang=\"en\">");
        sb.AppendLine("<head>");
        sb.AppendLine("    <meta charset=\"UTF-8\">");
        sb.AppendLine("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">");
        sb.AppendLine($"    <title>{HttpUtility.HtmlEncode(settings.ProjectName)} - Project Report</title>");
        sb.AppendLine(GenerateStyles());
        sb.AppendLine(GenerateProjectStyles());
        sb.AppendLine("</head>");
        sb.AppendLine("<body>");
        
        // Project header
        sb.AppendLine(GenerateProjectHeader(reels, settings, totalClips, totalDuration));
        
        // Table of contents for reels
        sb.AppendLine(GenerateReelToc(reels));
        
        // Each reel as a section
        sb.AppendLine("<main class=\"project-container\">");
        foreach (var reel in reels)
        {
            sb.AppendLine(GenerateReelSection(reel));
        }
        sb.AppendLine("</main>");
        
        // Footer
        sb.AppendLine(GenerateFooter());
        
        sb.AppendLine("</body>");
        sb.AppendLine("</html>");
        
        return sb.ToString();
    }
    
    private static string GenerateProjectStyles()
    {
        return """
            <style>
                .project-header {
                    background: linear-gradient(135deg, var(--bg-secondary) 0%, var(--bg-tertiary) 100%);
                    border-bottom: 2px solid var(--accent);
                }
                
                .project-stats {
                    display: flex;
                    gap: 3rem;
                    margin-top: 0.5rem;
                }
                
                .project-stat {
                    text-align: center;
                }
                
                .project-stat .number {
                    font-size: 1.5rem;
                    font-weight: 700;
                    color: var(--accent);
                }
                
                .project-stat .label {
                    font-size: 0.75rem;
                    color: var(--text-muted);
                    text-transform: uppercase;
                }
                
                .toc {
                    background: var(--bg-secondary);
                    padding: 1rem 2rem;
                    border-bottom: 1px solid var(--border);
                }
                
                .toc-content {
                    max-width: 1400px;
                    margin: 0 auto;
                }
                
                .toc h2 {
                    font-size: 0.875rem;
                    color: var(--text-muted);
                    text-transform: uppercase;
                    margin-bottom: 0.75rem;
                }
                
                .toc-list {
                    display: flex;
                    gap: 1rem;
                    flex-wrap: wrap;
                }
                
                .toc-item {
                    background: var(--bg-tertiary);
                    padding: 0.5rem 1rem;
                    border-radius: 6px;
                    color: var(--text-primary);
                    text-decoration: none;
                    transition: background 0.2s;
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                }
                
                .toc-item:hover {
                    background: var(--accent);
                    color: var(--bg-primary);
                }
                
                .toc-item .clip-count {
                    background: var(--bg-secondary);
                    padding: 0.125rem 0.5rem;
                    border-radius: 4px;
                    font-size: 0.75rem;
                    color: var(--text-secondary);
                }
                
                .project-container {
                    max-width: 1400px;
                    margin: 0 auto;
                    padding: 2rem;
                }
                
                .reel-section {
                    margin-bottom: 3rem;
                    border: 1px solid var(--border);
                    border-radius: 12px;
                    overflow: hidden;
                }
                
                .reel-section-header {
                    background: var(--bg-tertiary);
                    padding: 1.5rem 2rem;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    border-bottom: 1px solid var(--border);
                }
                
                .reel-section-header h2 {
                    font-size: 1.25rem;
                    display: flex;
                    align-items: center;
                    gap: 0.75rem;
                }
                
                .reel-section-header .reel-id {
                    background: var(--accent);
                    color: var(--bg-primary);
                    padding: 0.25rem 0.75rem;
                    border-radius: 4px;
                    font-size: 0.875rem;
                    font-weight: 600;
                }
                
                .reel-section-header .camera-name {
                    color: var(--text-secondary);
                    font-weight: normal;
                }
                
                .reel-section-stats {
                    display: flex;
                    gap: 2rem;
                }
                
                .reel-clips-container {
                    padding: 1.5rem;
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                }
                
                @media print {
                    .reel-section {
                        page-break-inside: avoid;
                    }
                }
            </style>
            """;
    }
    
    private static string GenerateProjectHeader(IReadOnlyList<CameraReel> reels, ReportSettings settings, int totalClips, TimeSpan totalDuration)
    {
        var sb = new StringBuilder();
        
        sb.AppendLine("<header class=\"header project-header\">");
        sb.AppendLine("    <div class=\"header-content\">");
        sb.AppendLine("        <div class=\"branding\">");
        
        // Logo
        if (!string.IsNullOrEmpty(settings.LogoBase64))
        {
            sb.AppendLine($"            <img src=\"data:image/png;base64,{settings.LogoBase64}\" alt=\"Logo\">");
        }
        
        sb.AppendLine("            <div class=\"project-info\">");
        sb.AppendLine($"                <h1>{HttpUtility.HtmlEncode(settings.ProjectName)}</h1>");
        
        var subtitle = new List<string>();
        if (!string.IsNullOrEmpty(settings.ProductionCompany)) subtitle.Add(settings.ProductionCompany);
        if (!string.IsNullOrEmpty(settings.DitName)) subtitle.Add($"DIT: {settings.DitName}");
        if (subtitle.Count > 0)
        {
            sb.AppendLine($"                <div class=\"subtitle\">{HttpUtility.HtmlEncode(string.Join(" • ", subtitle))}</div>");
        }
        
        sb.AppendLine("            </div>");
        sb.AppendLine("        </div>");
        
        // Project stats
        sb.AppendLine("        <div class=\"project-stats\">");
        sb.AppendLine($"            <div class=\"project-stat\"><span class=\"number\">{reels.Count}</span><span class=\"label\">Cards</span></div>");
        sb.AppendLine($"            <div class=\"project-stat\"><span class=\"number\">{totalClips}</span><span class=\"label\">Clips</span></div>");
        sb.AppendLine($"            <div class=\"project-stat\"><span class=\"number\">{totalDuration:hh\\:mm\\:ss}</span><span class=\"label\">Duration</span></div>");
        sb.AppendLine("        </div>");
        
        sb.AppendLine("    </div>");
        sb.AppendLine("</header>");
        
        return sb.ToString();
    }
    
    private static string GenerateReelToc(IReadOnlyList<CameraReel> reels)
    {
        var sb = new StringBuilder();
        
        sb.AppendLine("<nav class=\"toc\">");
        sb.AppendLine("    <div class=\"toc-content\">");
        sb.AppendLine("        <h2>Cards / Reels</h2>");
        sb.AppendLine("        <div class=\"toc-list\">");
        
        foreach (var reel in reels)
        {
            var id = HttpUtility.HtmlAttributeEncode(reel.DetectedReelId);
            sb.AppendLine($"            <a href=\"#reel-{id}\" class=\"toc-item\">");
            sb.AppendLine($"                {HttpUtility.HtmlEncode(reel.DisplayLabel)}");
            sb.AppendLine($"                <span class=\"clip-count\">{reel.Clips.Count} clips</span>");
            sb.AppendLine("            </a>");
        }
        
        sb.AppendLine("        </div>");
        sb.AppendLine("    </div>");
        sb.AppendLine("</nav>");
        
        return sb.ToString();
    }
    
    private string GenerateReelSection(CameraReel reel)
    {
        var sb = new StringBuilder();
        var reelId = HttpUtility.HtmlAttributeEncode(reel.DetectedReelId);
        
        sb.AppendLine($"<section class=\"reel-section\" id=\"reel-{reelId}\">");
        
        // Reel header
        sb.AppendLine("    <div class=\"reel-section-header\">");
        sb.AppendLine("        <h2>");
        sb.AppendLine($"            <span class=\"reel-id\">{HttpUtility.HtmlEncode(reel.DisplayLabel)}</span>");
        if (!string.IsNullOrEmpty(reel.CameraName))
        {
            sb.AppendLine($"            <span class=\"camera-name\">{HttpUtility.HtmlEncode(reel.CameraName)}</span>");
        }
        sb.AppendLine("        </h2>");
        
        // Reel stats
        sb.AppendLine("        <div class=\"reel-section-stats\">");
        sb.AppendLine($"            <div class=\"summary-item\"><span class=\"label\">Clips</span><span class=\"value\">{reel.Clips.Count}</span></div>");
        sb.AppendLine($"            <div class=\"summary-item\"><span class=\"label\">Duration</span><span class=\"value\">{reel.TotalDuration:hh\\:mm\\:ss}</span></div>");
        sb.AppendLine("        </div>");
        sb.AppendLine("    </div>");
        
        // Clips
        sb.AppendLine("    <div class=\"reel-clips-container\">");
        foreach (var clip in reel.Clips)
        {
            sb.AppendLine(GenerateClipCard(clip));
        }
        sb.AppendLine("    </div>");
        
        sb.AppendLine("</section>");
        
        return sb.ToString();
    }
    
    private static string GenerateStyles()
    {
        return """
            <style>
                :root {
                    --bg-primary: #0d1117;
                    --bg-secondary: #161b22;
                    --bg-tertiary: #21262d;
                    --text-primary: #e6edf3;
                    --text-secondary: #8b949e;
                    --text-muted: #6e7681;
                    --accent: #58a6ff;
                    --accent-hover: #79b8ff;
                    --border: #30363d;
                    --success: #3fb950;
                    --warning: #d29922;
                }
                
                * {
                    margin: 0;
                    padding: 0;
                    box-sizing: border-box;
                }
                
                body {
                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans', Helvetica, Arial, sans-serif;
                    background: var(--bg-primary);
                    color: var(--text-primary);
                    line-height: 1.6;
                    min-height: 100vh;
                }
                
                .header {
                    background: var(--bg-secondary);
                    border-bottom: 1px solid var(--border);
                    padding: 1.5rem 2rem;
                    position: sticky;
                    top: 0;
                    z-index: 100;
                }
                
                .header-content {
                    max-width: 1400px;
                    margin: 0 auto;
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 2rem;
                }
                
                .branding {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                }
                
                .branding img {
                    height: 48px;
                    width: auto;
                }
                
                .project-info h1 {
                    font-size: 1.5rem;
                    font-weight: 600;
                    color: var(--text-primary);
                }
                
                .project-info .subtitle {
                    font-size: 0.875rem;
                    color: var(--text-secondary);
                }
                
                .reel-badge {
                    background: var(--accent);
                    color: var(--bg-primary);
                    padding: 0.5rem 1rem;
                    border-radius: 6px;
                    font-weight: 600;
                    font-size: 1.25rem;
                }
                
                .summary {
                    background: var(--bg-secondary);
                    border-bottom: 1px solid var(--border);
                    padding: 1rem 2rem;
                }
                
                .summary-content {
                    max-width: 1400px;
                    margin: 0 auto;
                    display: flex;
                    gap: 2rem;
                    flex-wrap: wrap;
                }
                
                .summary-item {
                    display: flex;
                    flex-direction: column;
                }
                
                .summary-item .label {
                    font-size: 0.75rem;
                    color: var(--text-muted);
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                }
                
                .summary-item .value {
                    font-size: 1.125rem;
                    font-weight: 500;
                    color: var(--text-primary);
                }
                
                .clips-container {
                    max-width: 1400px;
                    margin: 0 auto;
                    padding: 2rem;
                    display: flex;
                    flex-direction: column;
                    gap: 1.5rem;
                }
                
                .clip-card {
                    background: var(--bg-secondary);
                    border: 1px solid var(--border);
                    border-radius: 8px;
                    overflow: hidden;
                }
                
                .clip-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    padding: 1rem 1.5rem;
                    background: var(--bg-tertiary);
                    border-bottom: 1px solid var(--border);
                }
                
                .clip-name {
                    font-weight: 600;
                    font-size: 1rem;
                    color: var(--text-primary);
                }
                
                .clip-timecode {
                    font-family: 'SF Mono', 'Consolas', monospace;
                    font-size: 0.875rem;
                    color: var(--accent);
                }
                
                .clip-content {
                    display: grid;
                    grid-template-columns: minmax(300px, 1fr) 300px;
                    gap: 1.5rem;
                    padding: 1.5rem;
                }
                
                @media (max-width: 768px) {
                    .clip-content {
                        grid-template-columns: 1fr;
                    }
                }
                
                .thumbnails {
                    display: flex;
                    gap: 0.5rem;
                    overflow-x: auto;
                }
                
                .thumbnail {
                    flex-shrink: 0;
                    border-radius: 4px;
                    overflow: hidden;
                    border: 1px solid var(--border);
                }
                
                .thumbnail img {
                    display: block;
                    width: auto;
                    height: 120px;
                    object-fit: cover;
                }
                
                .thumbnail-label {
                    background: var(--bg-tertiary);
                    padding: 0.25rem 0.5rem;
                    font-size: 0.75rem;
                    color: var(--text-secondary);
                    text-align: center;
                    font-family: 'SF Mono', 'Consolas', monospace;
                }
                
                .metadata {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: 0.75rem;
                }
                
                .metadata-item {
                    display: flex;
                    flex-direction: column;
                }
                
                .metadata-item .label {
                    font-size: 0.7rem;
                    color: var(--text-muted);
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                }
                
                .metadata-item .value {
                    font-size: 0.875rem;
                    color: var(--text-primary);
                }
                
                .footer {
                    text-align: center;
                    padding: 2rem;
                    color: var(--text-muted);
                    font-size: 0.875rem;
                    border-top: 1px solid var(--border);
                    margin-top: 2rem;
                }
                
                .footer a {
                    color: var(--accent);
                    text-decoration: none;
                }
                
                .footer a:hover {
                    text-decoration: underline;
                }
                
                @media print {
                    body {
                        background: white;
                        color: black;
                    }
                    
                    .header, .summary, .clip-card {
                        background: white;
                        border-color: #ddd;
                    }
                    
                    .clip-header {
                        background: #f5f5f5;
                    }
                }
            </style>
        """;
    }
    
    private static string GenerateHeader(CameraReel reel, ReportSettings settings)
    {
        var sb = new StringBuilder();
        
        sb.AppendLine("<header class=\"header\">");
        sb.AppendLine("    <div class=\"header-content\">");
        sb.AppendLine("        <div class=\"branding\">");
        
        if (settings.HasLogo)
        {
            var logoSrc = !string.IsNullOrEmpty(settings.LogoBase64) 
                ? $"data:image/png;base64,{settings.LogoBase64}"
                : settings.LogoPath ?? "";
            sb.AppendLine($"            <img src=\"{HttpUtility.HtmlAttributeEncode(logoSrc)}\" alt=\"Logo\">");
        }
        
        sb.AppendLine("            <div class=\"project-info\">");
        sb.AppendLine($"                <h1>{HttpUtility.HtmlEncode(settings.ProjectName ?? "Camera Report")}</h1>");
        
        var subtitleParts = new List<string>();
        if (!string.IsNullOrEmpty(settings.ProductionCompany))
            subtitleParts.Add(settings.ProductionCompany);
        if (!string.IsNullOrEmpty(settings.DitName))
            subtitleParts.Add($"DIT: {settings.DitName}");
        
        if (subtitleParts.Count > 0)
        {
            sb.AppendLine($"                <div class=\"subtitle\">{HttpUtility.HtmlEncode(string.Join(" • ", subtitleParts))}</div>");
        }
        
        sb.AppendLine("            </div>");
        sb.AppendLine("        </div>");
        sb.AppendLine($"        <div class=\"reel-badge\">{HttpUtility.HtmlEncode(reel.DisplayLabel)}</div>");
        sb.AppendLine("    </div>");
        sb.AppendLine("</header>");
        
        return sb.ToString();
    }
    
    private static string GenerateSummary(CameraReel reel)
    {
        var sb = new StringBuilder();
        
        sb.AppendLine("<section class=\"summary\">");
        sb.AppendLine("    <div class=\"summary-content\">");
        
        sb.AppendLine("        <div class=\"summary-item\">");
        sb.AppendLine("            <span class=\"label\">Clips</span>");
        sb.AppendLine($"            <span class=\"value\">{reel.ClipCount}</span>");
        sb.AppendLine("        </div>");
        
        sb.AppendLine("        <div class=\"summary-item\">");
        sb.AppendLine("            <span class=\"label\">Total Duration</span>");
        sb.AppendLine($"            <span class=\"value\">{reel.TotalDuration:hh\\:mm\\:ss}</span>");
        sb.AppendLine("        </div>");
        
        if (reel.RecordedDate.HasValue)
        {
            sb.AppendLine("        <div class=\"summary-item\">");
            sb.AppendLine("            <span class=\"label\">Recorded</span>");
            sb.AppendLine($"            <span class=\"value\">{reel.RecordedDate:yyyy-MM-dd}</span>");
            sb.AppendLine("        </div>");
        }
        
        if (!string.IsNullOrEmpty(reel.CameraName))
        {
            sb.AppendLine("        <div class=\"summary-item\">");
            sb.AppendLine("            <span class=\"label\">Camera</span>");
            sb.AppendLine($"            <span class=\"value\">{HttpUtility.HtmlEncode(reel.CameraName)}</span>");
            sb.AppendLine("        </div>");
        }
        
        sb.AppendLine("    </div>");
        sb.AppendLine("</section>");
        
        return sb.ToString();
    }
    
    private static string GenerateClipCard(CameraClip clip)
    {
        var sb = new StringBuilder();
        
        sb.AppendLine("<article class=\"clip-card\">");
        
        // Header
        sb.AppendLine("    <div class=\"clip-header\">");
        sb.AppendLine($"        <span class=\"clip-name\">{HttpUtility.HtmlEncode(clip.FileName)}</span>");
        if (!string.IsNullOrEmpty(clip.Timecode))
        {
            sb.AppendLine($"        <span class=\"clip-timecode\">{HttpUtility.HtmlEncode(clip.Timecode)}</span>");
        }
        sb.AppendLine("    </div>");
        
        // Content
        sb.AppendLine("    <div class=\"clip-content\">");
        
        // Thumbnails
        if (clip.Thumbnails.Count > 0)
        {
            sb.AppendLine("        <div class=\"thumbnails\">");
            foreach (var thumb in clip.Thumbnails)
            {
                sb.AppendLine("            <div class=\"thumbnail\">");
                sb.AppendLine($"                <img src=\"{thumb.ImageSource}\" alt=\"Frame\">");
                sb.AppendLine($"                <div class=\"thumbnail-label\">{HttpUtility.HtmlEncode(thumb.Timecode ?? "-")}</div>");
                sb.AppendLine("            </div>");
            }
            sb.AppendLine("        </div>");
        }
        
        // Metadata
        sb.AppendLine("        <div class=\"metadata\">");
        
        AddMetadataItem(sb, "Resolution", clip.Resolution);
        AddMetadataItem(sb, "Codec", clip.Codec);
        AddMetadataItem(sb, "Frame Rate", $"{clip.FrameRate:0.##} fps");
        AddMetadataItem(sb, "Duration", clip.DurationFormatted);
        AddMetadataItem(sb, "Size", clip.FileSizeFormatted);
        
        if (clip.Iso.HasValue)
            AddMetadataItem(sb, "ISO", clip.Iso.ToString()!);
        if (clip.WhiteBalance.HasValue)
            AddMetadataItem(sb, "White Balance", $"{clip.WhiteBalance}K");
        if (!string.IsNullOrEmpty(clip.Lens))
            AddMetadataItem(sb, "Lens", clip.Lens);
        if (!string.IsNullOrEmpty(clip.FocalLength))
            AddMetadataItem(sb, "Focal Length", clip.FocalLength);
        if (!string.IsNullOrEmpty(clip.TStop))
            AddMetadataItem(sb, "Aperture", clip.TStop);
        if (!string.IsNullOrEmpty(clip.ShutterAngle))
            AddMetadataItem(sb, "Shutter", clip.ShutterAngle);
        if (!string.IsNullOrEmpty(clip.Gamma))
            AddMetadataItem(sb, "Gamma", clip.Gamma);
        
        sb.AppendLine("        </div>");
        
        sb.AppendLine("    </div>");
        sb.AppendLine("</article>");
        
        return sb.ToString();
    }
    
    private static void AddMetadataItem(StringBuilder sb, string label, string? value)
    {
        if (string.IsNullOrEmpty(value)) return;
        
        sb.AppendLine("            <div class=\"metadata-item\">");
        sb.AppendLine($"                <span class=\"label\">{HttpUtility.HtmlEncode(label)}</span>");
        sb.AppendLine($"                <span class=\"value\">{HttpUtility.HtmlEncode(value)}</span>");
        sb.AppendLine("            </div>");
    }
    
    private static string GenerateFooter()
    {
        return $"""
            <footer class="footer">
                <p>Generated by Luna • {DateTime.Now:yyyy-MM-dd HH:mm}</p>
            </footer>
        """;
    }
}
