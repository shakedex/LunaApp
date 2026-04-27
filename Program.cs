using Avalonia;
using LunaApp.Services;
using LunaApp.Services.Chappie;
using Microsoft.Extensions.DependencyInjection;
using Serilog;
using System;
using Velopack;

namespace LunaApp;

sealed class Program
{
    public static IServiceProvider Services { get; private set; } = null!;

    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        var logPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "Luna", "logs", "luna-.log");

        Log.Logger = new LoggerConfiguration()
            .MinimumLevel.Debug()
            .WriteTo.File(logPath,
                rollingInterval: RollingInterval.Day,
                retainedFileCountLimit: 7)
            .WriteTo.Sink(InMemoryLogSink.Instance)
            .CreateLogger();

        Log.Information("Luna starting up");

        Services = new ServiceCollection()
            .AddLunaServices()
            .BuildServiceProvider();

        // Diagnostic: `Luna --probe-clip <path>` runs a single file through the
        // camera support pipeline and prints the resulting CameraClip as JSON.
        // Useful for verifying ARRI / Sony / other enrichers without booting
        // the full Avalonia UI.
        if (args.Length >= 2 && args[0] == "--probe-clip")
        {
            ProbeClip(args[1]).GetAwaiter().GetResult();
            Log.CloseAndFlush();
            return;
        }

        // Diagnostic: dump everything MediaInfo's native lib knows about a
        // file — used to confirm whether ARRI MXF metadata sets surface
        // before we commit to ART CLI integration.
        if (args.Length >= 2 && args[0] == "--probe-mediainfo")
        {
            ProbeMediaInfo(args[1]);
            Log.CloseAndFlush();
            return;
        }

        // Diagnostic: trigger an ART CLI install via the auto-installer.
        // Useful to verify the download / extract / SHA pipeline without
        // touching the UI.
        if (args.Length >= 1 && args[0] == "--install-art-cli")
        {
            InstallArtCli().GetAwaiter().GetResult();
            Log.CloseAndFlush();
            return;
        }


        try
        {
            VelopackApp.Build().Run();

            BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
        }
        catch (Exception ex)
        {
            Log.Fatal(ex, "Application crashed");
            throw;
        }
        finally
        {
            Log.Information("Luna shutting down");
            (Services as IDisposable)?.Dispose();
            Log.CloseAndFlush();
        }
    }

    private static async Task ProbeClip(string filePath)
    {
        var engine = Services.GetRequiredService<ChappieEngine>();
        var clip = await engine.ProcessClipAsync(filePath, extractThumbnails: true, thumbnailCount: 3);

        Console.WriteLine("=== CameraClip ===");
        Console.WriteLine($"FilePath           : {clip.FilePath}");
        Console.WriteLine($"FileName           : {clip.FileName}");
        Console.WriteLine($"Container          : {clip.Container}");
        Console.WriteLine($"FileSize           : {clip.FileSizeFormatted}");
        Console.WriteLine($"Codec              : {clip.Codec}");
        Console.WriteLine($"Resolution         : {clip.Resolution}");
        Console.WriteLine($"Duration           : {clip.DurationFormatted}");
        Console.WriteLine($"FrameRate          : {clip.FrameRate:F3} fps");
        Console.WriteLine($"Timecode           : {clip.Timecode}");
        Console.WriteLine($"BitDepth           : {clip.BitDepth}");
        Console.WriteLine($"ColorSpace         : {clip.ColorSpace}");
        Console.WriteLine();
        Console.WriteLine($"CameraManufacturer : {clip.CameraManufacturer}");
        Console.WriteLine($"CameraModel        : {clip.CameraModel}");
        Console.WriteLine($"CameraSerial       : {clip.CameraSerial}");
        Console.WriteLine($"ReelName           : {clip.ReelName}");
        Console.WriteLine($"Iso                : {clip.Iso}");
        Console.WriteLine($"WhiteBalance       : {clip.WhiteBalance}");
        Console.WriteLine($"ShutterAngle       : {clip.ShutterAngle}");
        Console.WriteLine($"ShutterSpeed       : {clip.ShutterSpeed}");
        Console.WriteLine($"Lens               : {clip.Lens}");
        Console.WriteLine($"FocalLength        : {clip.FocalLength}");
        Console.WriteLine($"TStop              : {clip.TStop}");
        Console.WriteLine($"Gamma              : {clip.Gamma}");
        Console.WriteLine($"LookName           : {clip.LookName}");
        Console.WriteLine();
        Console.WriteLine($"ProcessingState    : {clip.ProcessingState}");
        Console.WriteLine($"Thumbnails         : {clip.Thumbnails.Count}");
        Console.WriteLine($"ThumbnailOutcome   : {clip.ThumbnailOutcome}{(clip.ThumbnailOutcomeDetail is null ? "" : $" — {clip.ThumbnailOutcomeDetail}")}");
        if (clip.UnsupportedNotice is { } notice)
            Console.WriteLine($"UnsupportedNotice  : {notice.DisplayName} — {notice.Reason}");
        if (clip.ProcessingError is not null)
            Console.WriteLine($"ProcessingError    : {clip.ProcessingError}");
    }

    private static async Task InstallArtCli()
    {
        var installer = Services.GetRequiredService<LunaApp.Services.Chappie.ArtCliInstaller>();
        var progress = new Progress<double>(p => Console.Write($"\r  Progress: {p * 100:F1}%       "));
        Console.WriteLine($"Installing ART CLI ({installer.CurrentRelease?.Version}, ~{installer.CurrentRelease?.DownloadSizeBytes / 1_000_000:F0} MB)...");
        var result = await installer.InstallAsync(progress);
        Console.WriteLine();
        if (result.Success)
            Console.WriteLine($"✓ Installed to: {result.Path}");
        else
            Console.WriteLine($"✗ Install failed: {result.Error}");
    }

    private static void ProbeMediaInfo(string filePath)
    {
        using var mi = new MediaInfo.MediaInfo();
        // Verbose + JSON output — text mode summarizes; JSON returns every
        // declared field on every track, which is what we need to see
        // whether ARRI MXF descriptor sets surface.
        mi.Option("Complete", "1");
        mi.Option("Output", "JSON");
        if (mi.Open(filePath) == 0)
        {
            Console.WriteLine($"MediaInfo failed to open: {filePath}");
            return;
        }
        try
        {
            Console.WriteLine(mi.Inform());
        }
        finally
        {
            mi.Close();
        }
    }

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
        => AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();
}
