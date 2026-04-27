using LunaApp.Models;
using LunaApp.Services.Reports;
using LunaApp.Services.Reports.Html;
using QuestPDF.Fluent;
using QuestPDF.Helpers;
using QuestPDF.Infrastructure;
using Serilog;

namespace LunaApp.Services;

/// <summary>
/// Generates PDF camera reports. Layout rules:
///  - No dedicated cover page: page 1 opens with an extended "report header"
///    (branding + meta + hero stats), then flows straight into clip content.
///  - No running page header — just a minimal footer with project name, date
///    and page x/y, so ink isn't wasted on chrome.
///  - Each clip card uses <c>ShowEntire()</c> so it never splits across pages.
///  - A light/dark palette is chosen from <see cref="ReportSettings.Theme"/>.
/// </summary>
public sealed class PdfReportService
{
    static PdfReportService()
    {
        QuestPDF.Settings.License = LicenseType.Community;
    }

    public async Task<string> GenerateAndSaveReportAsync(
        CameraReel reel,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return await Task.Run(() =>
        {
            var reportName = settings.GenerateReportName(reel);
            var pdfPath = ResolveOutputPath(settings, reportName);
            BuildReelDocument(reel, settings).GeneratePdf(pdfPath);
            Log.Information("PDF report saved: {Path}", pdfPath);
            return pdfPath;
        }, cancellationToken);
    }

    public async Task<string> GenerateAndSaveProjectReportAsync(
        IReadOnlyList<CameraReel> reels,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return await Task.Run(() =>
        {
            var reportName = ReportNaming.ProjectReportName(settings);
            var pdfPath = ResolveOutputPath(settings, reportName);
            BuildProjectDocument(reels, settings).GeneratePdf(pdfPath);
            Log.Information("Project PDF report saved: {Path}", pdfPath);
            return pdfPath;
        }, cancellationToken);
    }

    private static string ResolveOutputPath(ReportSettings settings, string reportName)
    {
        if (settings.GroupPdfsInSeparateFolder)
        {
            var pdfFolder = Path.Combine(settings.OutputFolder, "PDFs");
            Directory.CreateDirectory(pdfFolder);
            return Path.Combine(pdfFolder, $"{reportName}.pdf");
        }

        var reportFolder = Path.Combine(settings.OutputFolder, reportName);
        Directory.CreateDirectory(reportFolder);
        return Path.Combine(reportFolder, $"{reportName}.pdf");
    }

    // ============================================================
    // Palette
    // ============================================================

    private readonly record struct Palette(
        string Bg,
        string Surface,
        string SurfaceAlt,
        string InkPrimary,
        string InkSecondary,
        string InkMuted,
        string Line,
        string LineMuted,
        string Accent,
        string AccentTint)
    {
        public static Palette Light => new(
            Bg:           "#FFFFFF",
            Surface:      "#FFFFFF",
            SurfaceAlt:   "#FAFAFA",
            InkPrimary:   "#0F172A",
            InkSecondary: "#334155",
            InkMuted:     "#64748B",
            Line:         "#E5E7EB",
            LineMuted:    "#F1F5F9",
            Accent:       "#2563EB",
            AccentTint:   "#EFF6FF");

        public static Palette Dark => new(
            Bg:           "#0D1117",
            Surface:      "#161B22",
            SurfaceAlt:   "#1C222B",
            InkPrimary:   "#E6EDF3",
            InkSecondary: "#B4BFCA",
            InkMuted:     "#8B949E",
            Line:         "#30363D",
            LineMuted:    "#21262D",
            Accent:       "#58A6FF",
            AccentTint:   "#1F3552");
    }

    private static Palette PaletteFor(ReportSettings s) =>
        s.Theme == ReportTheme.Dark ? Palette.Dark : Palette.Light;

    // ============================================================
    // Document composition
    // ============================================================

    private static Document BuildProjectDocument(IReadOnlyList<CameraReel> reels, ReportSettings settings)
    {
        var p = PaletteFor(settings);
        var totalClips = reels.Sum(r => r.Clips.Count);
        var totalDuration = TimeSpan.FromTicks(reels.Sum(r => r.TotalDuration.Ticks));
        var totalBytes = reels.SelectMany(r => r.Clips).Sum(c => c.FileSizeBytes);

        return Document.Create(container =>
        {
            container.Page(page =>
            {
                ConfigurePage(page, p);
                page.Content().Column(col =>
                {
                    col.Item().Element(c => ComposeReportHeader(c, p, settings,
                        reelCount: reels.Count,
                        clipCount: totalClips,
                        totalDuration: totalDuration,
                        totalBytes: totalBytes));

                    col.Item().PaddingTop(18);

                    for (var i = 0; i < reels.Count; i++)
                    {
                        if (i > 0) col.Item().PaddingTop(22);
                        col.Item().Element(c => ComposeReelSection(c, p, reels[i]));
                    }
                });
                page.Footer().Element(c => ComposeFooter(c, p, settings));
            });
        });
    }

    private static Document BuildReelDocument(CameraReel reel, ReportSettings settings)
    {
        var p = PaletteFor(settings);
        var bytes = reel.Clips.Sum(c => c.FileSizeBytes);

        return Document.Create(container =>
        {
            container.Page(page =>
            {
                ConfigurePage(page, p);
                page.Content().Column(col =>
                {
                    col.Item().Element(c => ComposeReportHeader(c, p, settings,
                        reelCount: 1,
                        clipCount: reel.ClipCount,
                        totalDuration: reel.TotalDuration,
                        totalBytes: bytes));
                    col.Item().PaddingTop(18).Element(c => ComposeReelSection(c, p, reel));
                });
                page.Footer().Element(c => ComposeFooter(c, p, settings));
            });
        });
    }

    private static void ConfigurePage(PageDescriptor page, Palette p)
    {
        page.Size(PageSizes.A4);
        page.Margin(36);
        page.DefaultTextStyle(TextStyle.Default
            .FontSize(9.5f)
            .FontColor(p.InkPrimary)
            .FontFamily("Segoe UI", "Inter", "Arial"));
        page.PageColor(p.Bg);
    }

    // ============================================================
    // First-page report header (replaces the old separate cover)
    // ============================================================

    private static void ComposeReportHeader(
        IContainer container,
        Palette p,
        ReportSettings settings,
        int reelCount,
        int clipCount,
        TimeSpan totalDuration,
        long totalBytes)
    {
        container.Column(col =>
        {
            // Branding row: title + meta on left, logo on right
            col.Item().Row(row =>
            {
                row.RelativeItem().Column(c =>
                {
                    c.Item().Text("CAMERA REPORT")
                        .FontSize(8).FontColor(p.InkMuted).LetterSpacing(0.22f).SemiBold();
                    c.Item().PaddingTop(4).Text(ReportNaming.EffectiveProjectName(settings))
                        .FontSize(22).Bold().LineHeight(1.15f).FontColor(p.InkPrimary);

                    var meta = BuildMetaLine(settings);
                    if (meta.Count > 0)
                    {
                        c.Item().PaddingTop(6).Text(text =>
                        {
                            text.DefaultTextStyle(TextStyle.Default.FontSize(9).FontColor(p.InkSecondary));
                            for (var i = 0; i < meta.Count; i++)
                            {
                                if (i > 0) text.Span("  ·  ").FontColor(p.InkMuted);
                                text.Span(meta[i]);
                            }
                        });
                    }
                });

                var logo = TryLoadLogo(settings);
                if (logo is not null)
                {
                    row.ConstantItem(120).AlignTop().AlignRight().Height(54).Image(logo).FitHeight();
                }
            });

            // Divider
            col.Item().PaddingVertical(14).LineHorizontal(0.75f).LineColor(p.Line);

            // Hero stats: Cards / Clips / Duration / Total Size
            col.Item().Row(row =>
            {
                Stat(row, p, reelCount.ToString(), reelCount == 1 ? "Card" : "Cards");
                Stat(row, p, clipCount.ToString(), clipCount == 1 ? "Clip" : "Clips");
                Stat(row, p, totalDuration.ToString(@"hh\:mm\:ss"), "Duration");
                Stat(row, p, FormatBytes(totalBytes), "Total size");
            });
        });
    }

    private static List<string> BuildMetaLine(ReportSettings s)
    {
        var meta = new List<string>();
        if (!string.IsNullOrEmpty(s.ProductionCompany)) meta.Add(s.ProductionCompany!);
        if (!string.IsNullOrEmpty(s.DitName))           meta.Add($"DIT · {s.DitName}");
        if (!string.IsNullOrEmpty(s.Director))          meta.Add($"Director · {s.Director}");
        if (!string.IsNullOrEmpty(s.Dp))                meta.Add($"DP · {s.Dp}");
        meta.Add(DateTime.Now.ToString("MMMM d, yyyy"));
        return meta;
    }

    private static void Stat(RowDescriptor row, Palette p, string value, string label)
    {
        row.RelativeItem().Column(c =>
        {
            c.Item().Text(value).FontSize(18).Bold().FontColor(p.InkPrimary);
            c.Item().PaddingTop(1).Text(label.ToUpperInvariant())
                .FontSize(7.5f).FontColor(p.InkMuted).LetterSpacing(0.18f).SemiBold();
        });
    }

    // ============================================================
    // Reel section
    // ============================================================

    private static void ComposeReelSection(IContainer container, Palette p, CameraReel reel)
    {
        container.Column(col =>
        {
            // Keep the reel header WITH its first clip so the header never
            // orphans at the bottom of a page. ShowEntire on the joined block
            // moves both to the next page together if they won't fit here.
            if (reel.Clips.Count > 0)
            {
                col.Item().ShowEntire().Column(inner =>
                {
                    inner.Item().Element(c => ComposeReelHeader(c, p, reel));
                    inner.Item().PaddingTop(8);
                    inner.Item().Element(c => ComposeClipCard(c, p, reel.Clips[0]));
                });
            }
            else
            {
                col.Item().Element(c => ComposeReelHeader(c, p, reel));
            }

            // Remaining clips — each wrapped independently in ShowEntire so
            // a card is never split between its header and its body.
            for (var i = 1; i < reel.Clips.Count; i++)
            {
                var clip = reel.Clips[i];
                col.Item().PaddingTop(8).ShowEntire()
                    .Element(c => ComposeClipCard(c, p, clip));
            }
        });
    }

    private static void ComposeReelHeader(IContainer container, Palette p, CameraReel reel)
    {
        container.Row(row =>
        {
            row.RelativeItem().Row(inner =>
            {
                inner.AutoItem().Background(p.InkPrimary).PaddingHorizontal(8).PaddingVertical(3)
                    .AlignMiddle()
                    .Text(reel.DisplayLabel).FontColor(p.Bg).FontSize(10).SemiBold();
                if (!string.IsNullOrEmpty(reel.CameraName))
                {
                    inner.AutoItem().PaddingLeft(10).AlignMiddle()
                        .Text(reel.CameraName!).FontSize(10).FontColor(p.InkSecondary);
                }
            });

            row.ConstantItem(180).AlignRight().Row(r =>
            {
                r.RelativeItem().AlignRight().Column(c =>
                {
                    c.Item().AlignRight().Text("CLIPS").FontSize(7).FontColor(p.InkMuted).LetterSpacing(0.18f).SemiBold();
                    c.Item().AlignRight().Text(reel.Clips.Count.ToString()).FontSize(10).SemiBold();
                });
                r.RelativeItem().AlignRight().PaddingLeft(16).Column(c =>
                {
                    c.Item().AlignRight().Text("DURATION").FontSize(7).FontColor(p.InkMuted).LetterSpacing(0.18f).SemiBold();
                    c.Item().AlignRight().Text(reel.TotalDuration.ToString(@"hh\:mm\:ss")).FontSize(10).SemiBold();
                });
            });
        });
    }

    // ============================================================
    // Clip card — thumbnails on top, metadata in two columns below
    // ============================================================

    private static void ComposeClipCard(IContainer container, Palette p, CameraClip clip)
    {
        // Compact layout — target ~180pt per card so 3–4 fit per page after the
        // first-page report header, and ShowEntire can reliably keep each card
        // intact on a single page.
        container.Border(0.75f).BorderColor(p.Line).Background(p.Surface).Column(col =>
        {
            // Header row (slim)
            col.Item().Background(p.SurfaceAlt).BorderBottom(0.5f).BorderColor(p.Line)
                .PaddingVertical(5).PaddingHorizontal(10).Row(row =>
            {
                row.RelativeItem().AlignMiddle().Text(clip.FileName)
                    .SemiBold().FontColor(p.InkPrimary).FontSize(9.5f);
                if (!string.IsNullOrEmpty(clip.Timecode))
                {
                    row.AutoItem().Background(p.AccentTint).PaddingHorizontal(6).PaddingVertical(1)
                        .AlignMiddle()
                        .Text(clip.Timecode!).FontSize(8.5f).FontColor(p.Accent).FontFamily("Consolas");
                }
            });

            // Horizontal thumb strip (3 thumbs at ~55pt tall => bounded card height)
            col.Item().PaddingHorizontal(10).PaddingTop(7).Element(c => ComposeThumbs(c, p, clip));

            // Metadata in two columns below the thumbs
            col.Item().PaddingHorizontal(10).PaddingVertical(7).Row(row =>
            {
                row.RelativeItem().PaddingRight(10).Element(c => ComposeMetaColumn(c, p, BuildTechnicalRows(clip)));
                row.RelativeItem().PaddingLeft(10).Element(c => ComposeMetaColumn(c, p, BuildCameraRows(clip)));
            });
        });
    }

    private static void ComposeThumbs(IContainer container, Palette p, CameraClip clip)
    {
        if (clip.Thumbnails.Count == 0)
        {
            container.Height(40).Background(p.LineMuted).AlignCenter().AlignMiddle()
                .Text("No preview frames")
                .FontSize(8).FontColor(p.InkMuted).Italic();
            return;
        }

        // Fixed-height (64pt) horizontal strip so card height is predictable:
        // 48pt thumbnail + 4pt gap + 12pt timecode caption.
        container.Height(64).Row(row =>
        {
            var thumbs = clip.Thumbnails.Take(3).ToList();
            for (var i = 0; i < thumbs.Count; i++)
            {
                if (i > 0) row.ConstantItem(5);
                var thumb = thumbs[i];
                row.RelativeItem().Column(col =>
                {
                    col.Item().Height(48).Element(e =>
                    {
                        try
                        {
                            if (!string.IsNullOrEmpty(thumb.ImageBase64))
                            {
                                var imageData = Convert.FromBase64String(thumb.ImageBase64);
                                e.Image(imageData).FitArea();
                                return;
                            }
                            if (!string.IsNullOrEmpty(thumb.ImagePath) && File.Exists(thumb.ImagePath))
                            {
                                e.Image(thumb.ImagePath).FitArea();
                                return;
                            }
                            e.Background(p.LineMuted);
                        }
                        catch (Exception ex)
                        {
                            Log.Debug(ex, "Failed to render thumbnail in PDF for clip {File}", clip.FileName);
                            e.Background(p.LineMuted);
                        }
                    });
                    col.Item().AlignCenter().PaddingTop(3)
                        .Text(thumb.Timecode ?? "-")
                        .FontSize(7).FontColor(p.InkMuted).FontFamily("Consolas");
                });
            }
        });
    }

    private static void ComposeMetaColumn(IContainer container, Palette p, IEnumerable<(string Icon, string Key, string Value)> rows)
    {
        // Bake the muted-ink color into each SVG once — QuestPDF has no CSS,
        // so fill must be set on the SVG itself.
        var iconHex = p.InkMuted;

        container.Table(table =>
        {
            table.ColumnsDefinition(cols =>
            {
                cols.ConstantColumn(11); // icon
                cols.ConstantColumn(64); // key label
                cols.RelativeColumn();    // value
            });

            var any = false;
            foreach (var (icon, key, value) in rows)
            {
                any = true;

                var iconSvg = MaterialIconLibrary.RenderForPdf(icon, iconHex);
                var iconCell = table.Cell().BorderBottom(0.4f).BorderColor(p.LineMuted).PaddingVertical(2);
                if (iconSvg is not null)
                    iconCell.AlignMiddle().Width(9).Height(9).Svg(iconSvg);
                else
                    iconCell.Text(""); // unknown icon — leave blank

                table.Cell().BorderBottom(0.4f).BorderColor(p.LineMuted).PaddingVertical(2)
                    .Text(key.ToUpperInvariant())
                    .FontSize(7).FontColor(p.InkMuted).LetterSpacing(0.12f).SemiBold();
                table.Cell().BorderBottom(0.4f).BorderColor(p.LineMuted).PaddingVertical(2)
                    .Text(value).FontSize(8.5f);
            }
            if (!any)
            {
                table.Cell().ColumnSpan(3).Text("—").FontColor(p.InkMuted);
            }
        });
    }

    private static IEnumerable<(string Icon, string Key, string Value)> BuildTechnicalRows(CameraClip c)
    {
        yield return ("aspect_ratio", "Resolution", c.Resolution);
        if (!string.IsNullOrEmpty(c.Codec))      yield return ("videocam", "Codec", c.Codec!);
        if (c.FrameRate > 0)                     yield return ("speed", "Frame rate", $"{c.FrameRate:0.##} fps");
        yield return ("schedule", "Duration", c.DurationFormatted);
        yield return ("storage", "Size", c.FileSizeFormatted);
        if (!string.IsNullOrEmpty(c.ColorSpace)) yield return ("palette", "Color space", c.ColorSpace!);
    }

    private static IEnumerable<(string Icon, string Key, string Value)> BuildCameraRows(CameraClip c)
    {
        if (!string.IsNullOrEmpty(c.CameraModel))  yield return ("videocam",      "Camera",       c.CameraModel!);
        if (c.Iso.HasValue)                        yield return ("iso",           "ISO",          c.Iso!.Value.ToString());
        if (c.WhiteBalance.HasValue)               yield return ("wb_sunny",      "White bal.",   $"{c.WhiteBalance}K");
        if (!string.IsNullOrEmpty(c.Lens))         yield return ("camera",        "Lens",         c.Lens!);
        if (!string.IsNullOrEmpty(c.FocalLength))  yield return ("straighten",    "Focal length", c.FocalLength!);
        if (!string.IsNullOrEmpty(c.TStop))        yield return ("camera_roll",   "Aperture",     c.TStop!);
        if (!string.IsNullOrEmpty(c.ShutterAngle)) yield return ("shutter_speed", "Shutter",      c.ShutterAngle!);
        if (!string.IsNullOrEmpty(c.Gamma))        yield return ("tune",          "Gamma",        c.Gamma!);
    }

    // ============================================================
    // Footer (project · date · page x/y)
    // ============================================================

    private static void ComposeFooter(IContainer container, Palette p, ReportSettings settings)
    {
        // Identical layout to the HTML footer: "Generated by Luna · yyyy-MM-dd HH:mm"
        // on the left, page x / y on the right. One source of truth for the format.
        _ = settings; // settings intentionally unused so callers stay uniform
        container.BorderTop(0.5f).BorderColor(p.Line).PaddingTop(6).Row(row =>
        {
            row.RelativeItem().Text(text =>
            {
                text.DefaultTextStyle(TextStyle.Default.FontSize(7.5f).FontColor(p.InkMuted));
                text.Span($"Generated by Luna · {DateTime.Now:yyyy-MM-dd HH:mm}");
            });

            row.RelativeItem().AlignRight().Text(text =>
            {
                text.DefaultTextStyle(TextStyle.Default.FontSize(7.5f).FontColor(p.InkMuted));
                text.Span("Page ");
                text.CurrentPageNumber();
                text.Span(" / ");
                text.TotalPages();
            });
        });
    }

    // ============================================================
    // Helpers
    // ============================================================

    private static byte[]? TryLoadLogo(ReportSettings settings)
    {
        try
        {
            if (!string.IsNullOrEmpty(settings.LogoBase64))
                return Convert.FromBase64String(settings.LogoBase64);
            if (!string.IsNullOrEmpty(settings.LogoPath) && File.Exists(settings.LogoPath))
                return File.ReadAllBytes(settings.LogoPath);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to load logo for PDF report");
        }
        return null;
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
