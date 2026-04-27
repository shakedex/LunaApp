using System.Text.Json.Serialization;

namespace LunaApp.Models;

/// <summary>
/// Source-generated JSON type info for <see cref="AppSettings"/>.
///
/// We can't rely on reflection-based serialization because the app publishes
/// with <c>PublishTrimmed=true</c> — the runtime sets
/// <c>JsonSerializerIsReflectionEnabledByDefault = false</c> in that mode and
/// any reflection-based <c>JsonSerializer.Serialize/Deserialize</c> call
/// throws <see cref="System.InvalidOperationException"/> at runtime. The
/// source generator emits all the metadata ahead-of-time and is trim-safe.
///
/// Enum values are emitted as strings (e.g. <c>"Dark"</c> instead of <c>1</c>)
/// so the settings file stays human-readable and survives enum renumbering.
/// </summary>
[JsonSourceGenerationOptions(
    WriteIndented = true,
    UseStringEnumConverter = true,
    DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull)]
[JsonSerializable(typeof(AppSettings))]
internal partial class AppSettingsJsonContext : JsonSerializerContext
{
}
