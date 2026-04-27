using LunaApp.Models;
using LunaApp.Services;
using LunaApp.Services.CameraSupport;
using LunaApp.Services.Chappie;
using LunaApp.ViewModels;
using Microsoft.Extensions.DependencyInjection;

namespace LunaApp;

internal static class ServiceRegistration
{
    public static IServiceCollection AddLunaServices(this IServiceCollection services)
    {
        services.AddSingleton<MediaProcessingOptions>(_ => MediaProcessingOptions.Default);

        // Baseline metadata + thumbnail services that GenericCameraSupport wires together.
        services.AddSingleton<IMetadataExtractor, MediaInfoMetadataExtractor>();
        services.AddSingleton<IMetadataEnricher, SonyXmlEnricher>();
        services.AddSingleton<FfmpegFormatReader>();
        services.AddSingleton<IMetadataEnricher, ArriQuickTimeEnricher>();

        // ART CLI — locator + auto-installer. The metadata enricher and
        // thumbnail service kick in once the user has installed art-cmd.
        services.AddSingleton<ArtCliLocator>();
        services.AddSingleton<ArtCliInstaller>();
        services.AddSingleton<IMetadataEnricher, ArtCliMetadataEnricher>();

        // Thumbnail-generator chain. Lower Priority runs first; NoDecoder
        // cascades to the next generator. Adding a new vendor decoder is
        // one new IThumbnailGenerator implementation + one line below.
        services.AddSingleton<FfmpegThumbnailService>();
        services.AddSingleton<IThumbnailGenerator>(sp => sp.GetRequiredService<FfmpegThumbnailService>());
        services.AddSingleton<ArtCliThumbnailService>();
        services.AddSingleton<IThumbnailGenerator>(sp => sp.GetRequiredService<ArtCliThumbnailService>());
        services.AddSingleton<SonyRawViewerLocator>();
        services.AddSingleton<SonyRawExporterThumbnailService>();
        services.AddSingleton<IThumbnailGenerator>(sp => sp.GetRequiredService<SonyRawExporterThumbnailService>());

        // Camera support — one class per camera family. Adding a new format
        // is one new ICameraSupport implementation + one AddSingleton line.
        services.AddSingleton<ICameraSupport, GenericCameraSupport>();
        services.AddSingleton<ICameraSupport, ArriCameraSupport>();
        services.AddSingleton<ICameraSupport, BlackmagicRawCameraSupport>();
        services.AddSingleton<ICameraSupport, SonyVeniceCameraSupport>();
        services.AddSingleton<CameraSupportRegistry>();

        services.AddSingleton<ChappieEngine>();
        services.AddSingleton<ReelDetectionService>();
        services.AddSingleton<HtmlReportService>();
        services.AddSingleton<PdfReportService>();
        services.AddSingleton<ReportGenerationService>();
        services.AddSingleton<UpdateService>();

        services.AddTransient<MainWindowViewModel>();
        services.AddTransient<SettingsViewModel>();

        return services;
    }
}
