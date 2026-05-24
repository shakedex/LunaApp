namespace LunaApp.ViewModels;

/// <summary>
/// State machine for the hero processing overlay. <c>SuccessHold</c> is the
/// new state introduced for M6 — after a successful GenerateReportsAsync the
/// moon stays at 100% for ~2 s with inline confirmation actions before fading.
/// </summary>
public enum OverlayState
{
    Idle,
    Processing,
    SuccessHold,
    Fading,
}
