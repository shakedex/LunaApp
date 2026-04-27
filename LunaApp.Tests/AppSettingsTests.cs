using System.Text.Json;
using LunaApp.Models;

namespace LunaApp.Tests;

public class AppSettingsTests
{
    [Fact]
    public void CameraSupportSnoozeUntil_IsNullByDefault()
    {
        var settings = new AppSettings();
        Assert.Null(settings.CameraSupportSnoozeUntil);
    }

    [Fact]
    public void CameraSupportSnoozeUntil_RoundTripsThroughJson()
    {
        var when = new DateTime(2026, 4, 27, 14, 30, 0, DateTimeKind.Utc);
        var settings = new AppSettings { CameraSupportSnoozeUntil = when };

        // Round-trip through the production source-gen path the trimmed app
        // actually uses at runtime (AppSettingsJsonContext). A reflection-based
        // JsonSerializer call would pass here but throw in production where
        // PublishTrimmed=true sets JsonSerializerIsReflectionEnabledByDefault=false.
        var json = JsonSerializer.Serialize(settings, AppSettingsJsonContext.Default.AppSettings);
        var restored = JsonSerializer.Deserialize(json, AppSettingsJsonContext.Default.AppSettings);

        Assert.NotNull(restored);
        Assert.Equal(when, restored!.CameraSupportSnoozeUntil);
    }
}
