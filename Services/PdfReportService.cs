using LunaApp.Models;
using QuestPDF.Fluent;
using QuestPDF.Helpers;
using QuestPDF.Infrastructure;
using Serilog;
using SkiaSharp;

namespace LunaApp.Services;

/// <summary>
/// Service for generating PDF camera reports using QuestPDF.
/// </summary>
public sealed class PdfReportService
{
    static PdfReportService()
    {
        // QuestPDF Community License
        QuestPDF.Settings.License = LicenseType.Community;
    }
    
    /// <summary>
    /// Generate and save PDF report for a single reel
    /// </summary>
    public async Task<string> GenerateAndSaveReportAsync(
        CameraReel reel,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return await Task.Run(() =>
        {
            var reportName = settings.GenerateReportName(reel);
            string pdfPath;
            
            if (settings.GroupPdfsInSeparateFolder)
            {
                var pdfFolder = Path.Combine(settings.OutputFolder, "PDFs");
                Directory.CreateDirectory(pdfFolder);
                pdfPath = Path.Combine(pdfFolder, $"{reportName}.pdf");
            }
            else
            {
                var reportFolder = Path.Combine(settings.OutputFolder, reportName);
                Directory.CreateDirectory(reportFolder);
                pdfPath = Path.Combine(reportFolder, $"{reportName}.pdf");
            }
            
            var document = CreateDocument(reel, settings);
            document.GeneratePdf(pdfPath);
            
            Log.Information("PDF report saved: {Path}", pdfPath);
            
            return pdfPath;
        }, cancellationToken);
    }
    
    /// <summary>
    /// Generate and save unified PDF report for multiple reels (project mode)
    /// </summary>
    public async Task<string> GenerateAndSaveProjectReportAsync(
        IReadOnlyList<CameraReel> reels,
        ReportSettings settings,
        CancellationToken cancellationToken = default)
    {
        return await Task.Run(() =>
        {
            var reportName = $"{settings.ProjectName}_{DateTime.Now:yyyy-MM-dd}";
            string pdfPath;
            
            if (settings.GroupPdfsInSeparateFolder)
            {
                var pdfFolder = Path.Combine(settings.OutputFolder, "PDFs");
                Directory.CreateDirectory(pdfFolder);
                pdfPath = Path.Combine(pdfFolder, $"{reportName}.pdf");
            }
            else
            {
                var reportFolder = Path.Combine(settings.OutputFolder, reportName);
                Directory.CreateDirectory(reportFolder);
                pdfPath = Path.Combine(reportFolder, $"{reportName}.pdf");
            }
            
            var document = CreateProjectDocument(reels, settings);
            document.GeneratePdf(pdfPath);
            
            Log.Information("Project PDF report saved: {Path}", pdfPath);
            
            return pdfPath;
        }, cancellationToken);
    }
    
    private Document CreateProjectDocument(IReadOnlyList<CameraReel> reels, ReportSettings settings)
    {
        return Document.Create(container =>
        {
            container.Page(page =>
            {
                page.Size(PageSizes.A4);
                page.Margin(40);
                page.DefaultTextStyle(x => x.FontSize(10).FontColor(Colors.Grey.Darken4));
                
                page.Header().Element(c => ComposeProjectHeader(c, reels, settings));
                page.Content().Element(c => ComposeProjectContent(c, reels));
                page.Footer().Element(c => ComposeFooter(c));
            });
        });
    }
    
    private void ComposeProjectHeader(IContainer container, IReadOnlyList<CameraReel> reels, ReportSettings settings)
    {
        var totalClips = reels.Sum(r => r.Clips.Count);
        var totalDuration = TimeSpan.FromTicks(reels.Sum(r => r.TotalDuration.Ticks));
        
        container.Column(column =>
        {
            column.Item().Row(row =>
            {
                // Logo
                if (settings.HasLogo)
                {
                    try
                    {
                        var logoBytes = Convert.FromBase64String(settings.LogoBase64!);
                        row.ConstantItem(80).Image(logoBytes).FitArea();
                    }
                    catch { }
                }
                
                row.RelativeItem().Column(titleCol =>
                {
                    titleCol.Item().Text(settings.ProjectName)
                        .FontSize(20).Bold().FontColor(Colors.Grey.Darken4);
                    
                    var subtitle = new List<string>();
                    if (!string.IsNullOrEmpty(settings.ProductionCompany)) subtitle.Add(settings.ProductionCompany);
                    if (!string.IsNullOrEmpty(settings.DitName)) subtitle.Add($"DIT: {settings.DitName}");
                    if (subtitle.Count > 0)
                    {
                        titleCol.Item().Text(string.Join(" • ", subtitle))
                            .FontSize(10).FontColor(Colors.Grey.Medium);
                    }
                });
                
                row.ConstantItem(150).AlignRight().Column(statsCol =>
                {
                    statsCol.Item().Text($"{reels.Count} Cards").FontSize(12).Bold();
                    statsCol.Item().Text($"{totalClips} Clips").FontSize(10).FontColor(Colors.Grey.Medium);
                    statsCol.Item().Text($"{totalDuration:hh\\:mm\\:ss}").FontSize(10).FontColor(Colors.Grey.Medium);
                });
            });
            
            column.Item().PaddingTop(10).LineHorizontal(1).LineColor(Colors.Grey.Lighten2);
        });
    }
    
    private void ComposeProjectContent(IContainer container, IReadOnlyList<CameraReel> reels)
    {
        container.Column(column =>
        {
            foreach (var reel in reels)
            {
                // Reel section header
                column.Item().PaddingTop(15).Row(row =>
                {
                    row.RelativeItem().Column(headerCol =>
                    {
                        headerCol.Item().Text(reel.DisplayLabel)
                            .FontSize(14).Bold().FontColor(Colors.Blue.Darken2);
                        
                        if (!string.IsNullOrEmpty(reel.CameraName))
                        {
                            headerCol.Item().Text(reel.CameraName)
                                .FontSize(10).FontColor(Colors.Grey.Medium);
                        }
                    });
                    
                    row.ConstantItem(100).AlignRight().Column(statsCol =>
                    {
                        statsCol.Item().Text($"{reel.Clips.Count} clips").FontSize(9);
                        statsCol.Item().Text($"{reel.TotalDuration:hh\\:mm\\:ss}").FontSize(9).FontColor(Colors.Grey.Medium);
                    });
                });
                
                column.Item().PaddingVertical(5).LineHorizontal(0.5f).LineColor(Colors.Grey.Lighten3);
                
                // Clips in this reel
                foreach (var clip in reel.Clips)
                {
                    column.Item().PaddingVertical(5).Element(c => ComposeClipCard(c, clip));
                }
            }
        });
    }
    
    private Document CreateDocument(CameraReel reel, ReportSettings settings)
    {
        return Document.Create(container =>
        {
            container.Page(page =>
            {
                page.Size(PageSizes.A4);
                page.Margin(40);
                page.DefaultTextStyle(x => x.FontSize(10).FontColor(Colors.Grey.Darken4));
                
                page.Header().Element(c => ComposeHeader(c, reel, settings));
                page.Content().Element(c => ComposeContent(c, reel));
                page.Footer().Element(c => ComposeFooter(c));
            });
        });
    }
    
    private void ComposeHeader(IContainer container, CameraReel reel, ReportSettings settings)
    {
        container.Column(column =>
        {
            column.Item().Row(row =>
            {
                // Logo
                if (settings.HasLogo)
                {
                    row.ConstantItem(80).Height(40).Element(e => 
                    {
                        try
                        {
                            byte[]? logoData = null;
                            
                            if (!string.IsNullOrEmpty(settings.LogoBase64))
                            {
                                logoData = Convert.FromBase64String(settings.LogoBase64);
                            }
                            else if (!string.IsNullOrEmpty(settings.LogoPath) && File.Exists(settings.LogoPath))
                            {
                                logoData = File.ReadAllBytes(settings.LogoPath);
                            }
                            
                            if (logoData != null)
                            {
                                e.Image(logoData).FitArea();
                            }
                        }
                        catch
                        {
                            // Skip logo if it fails to load
                        }
                    });
                }
                
                // Project info
                row.RelativeItem().Column(col =>
                {
                    col.Item().Text(settings.ProjectName ?? "Camera Report")
                        .FontSize(18).Bold();
                    
                    var subtitleParts = new List<string>();
                    if (!string.IsNullOrEmpty(settings.ProductionCompany))
                        subtitleParts.Add(settings.ProductionCompany);
                    if (!string.IsNullOrEmpty(settings.DitName))
                        subtitleParts.Add($"DIT: {settings.DitName}");
                    
                    if (subtitleParts.Count > 0)
                    {
                        col.Item().Text(string.Join(" • ", subtitleParts))
                            .FontSize(10).FontColor(Colors.Grey.Darken1);
                    }
                });
                
                // Reel badge
                row.ConstantItem(100).AlignRight().AlignMiddle()
                    .Background(Colors.Blue.Darken2)
                    .Padding(8)
                    .Text(reel.DisplayLabel)
                    .FontSize(14).Bold().FontColor(Colors.White);
            });
            
            column.Item().PaddingVertical(10).LineHorizontal(1).LineColor(Colors.Grey.Lighten2);
            
            // Summary row
            column.Item().Row(row =>
            {
                row.RelativeItem().Text($"Clips: {reel.ClipCount}").FontSize(9);
                row.RelativeItem().Text($"Duration: {reel.TotalDuration:hh\\:mm\\:ss}").FontSize(9);
                if (reel.RecordedDate.HasValue)
                    row.RelativeItem().Text($"Recorded: {reel.RecordedDate:yyyy-MM-dd}").FontSize(9);
                if (!string.IsNullOrEmpty(reel.CameraName))
                    row.RelativeItem().Text($"Camera: {reel.CameraName}").FontSize(9);
            });
            
            column.Item().PaddingTop(10);
        });
    }
    
    private void ComposeContent(IContainer container, CameraReel reel)
    {
        container.Column(column =>
        {
            foreach (var clip in reel.Clips)
            {
                column.Item().Element(c => ComposeClipCard(c, clip));
                column.Item().PaddingVertical(8);
            }
        });
    }
    
    private void ComposeClipCard(IContainer container, CameraClip clip)
    {
        container.Border(1).BorderColor(Colors.Grey.Lighten2).Column(column =>
        {
            // Clip header
            column.Item()
                .Background(Colors.Grey.Lighten4)
                .Padding(8)
                .Row(row =>
                {
                    row.RelativeItem().Text(clip.FileName).Bold();
                    if (!string.IsNullOrEmpty(clip.Timecode))
                    {
                        row.ConstantItem(100).AlignRight()
                            .Text(clip.Timecode).FontColor(Colors.Blue.Darken2);
                    }
                });
            
            // Content
            column.Item().Padding(10).Row(row =>
            {
                // Thumbnails
                if (clip.Thumbnails.Count > 0)
                {
                    row.RelativeItem().Row(thumbRow =>
                    {
                        foreach (var thumb in clip.Thumbnails.Take(3))
                        {
                            thumbRow.ConstantItem(100).Padding(2).Column(thumbCol =>
                            {
                                try
                                {
                                    if (!string.IsNullOrEmpty(thumb.ImageBase64))
                                    {
                                        var imageData = Convert.FromBase64String(thumb.ImageBase64);
                                        thumbCol.Item().Height(60).Image(imageData).FitArea();
                                    }
                                }
                                catch
                                {
                                    thumbCol.Item().Height(60).Background(Colors.Grey.Lighten3);
                                }
                                
                                thumbCol.Item().AlignCenter()
                                    .Text(thumb.Timecode ?? "-")
                                    .FontSize(7).FontColor(Colors.Grey.Darken1);
                            });
                        }
                    });
                }
                
                // Metadata
                row.ConstantItem(180).Padding(5).Table(table =>
                {
                    table.ColumnsDefinition(cols =>
                    {
                        cols.RelativeColumn();
                        cols.RelativeColumn();
                    });
                    
                    AddMetadataRow(table, "Resolution", clip.Resolution);
                    AddMetadataRow(table, "Codec", clip.Codec);
                    AddMetadataRow(table, "Frame Rate", $"{clip.FrameRate:0.##} fps");
                    AddMetadataRow(table, "Duration", clip.DurationFormatted);
                    AddMetadataRow(table, "Size", clip.FileSizeFormatted);
                    
                    if (clip.Iso.HasValue)
                        AddMetadataRow(table, "ISO", clip.Iso.ToString()!);
                    if (clip.WhiteBalance.HasValue)
                        AddMetadataRow(table, "WB", $"{clip.WhiteBalance}K");
                    if (!string.IsNullOrEmpty(clip.Lens))
                        AddMetadataRow(table, "Lens", clip.Lens);
                    if (!string.IsNullOrEmpty(clip.TStop))
                        AddMetadataRow(table, "Aperture", clip.TStop);
                });
            });
        });
    }
    
    private static void AddMetadataRow(TableDescriptor table, string label, string? value)
    {
        if (string.IsNullOrEmpty(value)) return;
        
        table.Cell().Text(label).FontSize(7).FontColor(Colors.Grey.Darken1);
        table.Cell().Text(value).FontSize(8);
    }
    
    private void ComposeFooter(IContainer container)
    {
        container.AlignCenter().Text(text =>
        {
            text.Span($"Generated by Luna • {DateTime.Now:yyyy-MM-dd HH:mm}")
                .FontSize(8).FontColor(Colors.Grey.Medium);
            text.Span(" • Page ");
            text.CurrentPageNumber().FontSize(8);
            text.Span(" of ");
            text.TotalPages().FontSize(8);
        });
    }
}
