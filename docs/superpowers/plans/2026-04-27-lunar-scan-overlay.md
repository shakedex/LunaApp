# Lunar Scan Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the static moon-icon processing overlay with a hero shader-rendered moon, orbital rings, atmospheric aura, and a phase-driven progress reskin.

**Architecture:** A new self-contained Avalonia control (`LunaShaderMoon`) composed of three layers — Avalonia primitives for the aura and orbital rings, and a SkiaSharp SKSL fragment shader for the moon body. A single `DispatcherTimer` (60 Hz, gated by `IsActive`) drives ring rotation, aura breathing, and shader uniform updates. Phase changes are tweened in C# over 600ms and pushed to the shader as uniforms.

**Tech Stack:** Avalonia 11, SkiaSharp 3.119 (already referenced), CommunityToolkit.Mvvm, .NET 10. No new packages.

**Spec:** [docs/superpowers/specs/2026-04-27-lunar-scan-overlay-design.md](../specs/2026-04-27-lunar-scan-overlay-design.md)

**Verification approach:** No test project exists in this repo. Pure-function logic (phase → terminator/tint mapping) is verified via `Debug.Assert` calls invoked once at startup. Visual layers are verified via a new `DevSimulateProcessingCommand` in the existing developer panel that cycles through every `ProcessingPhase` with a 2s dwell on each.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `Views/Controls/LunaPhaseLook.cs` | Create | Pure static map: `ProcessingPhase` → terminator angle (radians), aura tint (`Color`). Plus `Lerp` helper between two `PhaseLook` records and a `RunSelfChecks()` method invoked from `App.OnFrameworkInitializationCompleted`. |
| `Views/Controls/MoonShader.sksl` | Create | SKSL fragment shader. Embedded as `AvaloniaResource`. Read via `AssetLoader` at control init. |
| `Views/Controls/LunaShaderMoon.cs` | Create | Avalonia `TemplatedControl` exposing `Phase`, `IsActive`, `OrbitalThroughput`. Composes aura `Border`, moon-body subcontrol, two ring `Ellipse`s + dot. Owns the 60Hz `DispatcherTimer` and the phase tween state. |
| `Views/Controls/MoonBody.cs` | Create | Inner `Control` subclass (110×110) that owns the SKSL `SKRuntimeEffect`. Overrides `Render` to draw the shader with current uniforms. Exposes `SetUniforms(float terminator, Color dayTint, Vector lightOffset)`. |
| `LunaApp.csproj` | Modify | Add `<AvaloniaResource Include="Views\Controls\MoonShader.sksl" />`. |
| `ViewModels/MainWindowViewModel.cs` | Modify | Add `[ObservableProperty] _currentPhase`. Assign in `ApplyReport`. Add `DevSimulateProcessingCommand`. |
| `Views/MainWindow.axaml` | Modify | Replace lines 459–464 with `<controls:LunaShaderMoon />`. Add `xmlns:controls`. Add `Classes="luna-cosmic"` to ProgressBar. Add `Classes="luna-eta"` to ETA TextBlock. Add the new dev-panel button. |
| `Styles/Controls.axaml` | Modify | Add `ProgressBar.luna-cosmic` style and `TextBlock.luna-eta` style. |
| `App.axaml.cs` | Modify | One-line call to `LunaPhaseLook.RunSelfChecks()` in `OnFrameworkInitializationCompleted` (Debug build only). |

---

## Task 1: Add `CurrentPhase` to the ViewModel

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs:41-44` (add property next to existing phase fields)
- Modify: `ViewModels/MainWindowViewModel.cs:332-350` (set in `ApplyReport`)

- [ ] **Step 1: Add the observable property**

In `ViewModels/MainWindowViewModel.cs`, locate the block at lines 41–44 starting with `// Phased progress (populated from ReportGenerationService.ProgressReported)`. Add this line directly under `[ObservableProperty] private string _etaText = string.Empty;`:

```csharp
[ObservableProperty] private LunaApp.Models.ProcessingPhase _currentPhase = LunaApp.Models.ProcessingPhase.Idle;
```

- [ ] **Step 2: Set the property in `ApplyReport`**

In the same file, in the `ApplyReport` method, find the block starting at line 342:

```csharp
Progress = report.Percent;
PhaseLabel = report.PhaseLabel;
PhaseDetail = BuildDetail(report);
EtaText = BuildEta(report);
```

Add `CurrentPhase = report.Phase;` as the first line of that block, so it becomes:

```csharp
CurrentPhase = report.Phase;
Progress = report.Percent;
PhaseLabel = report.PhaseLabel;
PhaseDetail = BuildDetail(report);
EtaText = BuildEta(report);
```

- [ ] **Step 3: Build to verify**

Run: `dotnet build LunaApp.csproj -c Debug`
Expected: Build succeeds with no new warnings.


---

## Task 2: Create `LunaPhaseLook` (pure phase → look mapping)

**Files:**
- Create: `Views/Controls/LunaPhaseLook.cs`
- Modify: `App.axaml.cs` (call `RunSelfChecks` in Debug)

- [ ] **Step 1: Create the file with the full mapping**

Create `Views/Controls/LunaPhaseLook.cs` with this exact content:

```csharp
using System;
using System.Collections.Generic;
using System.Diagnostics;
using Avalonia.Media;
using LunaApp.Models;

namespace LunaApp.Views.Controls;

/// <summary>
/// Pure mapping from <see cref="ProcessingPhase"/> to a visual "look" used by
/// <c>LunaShaderMoon</c>: a terminator angle (radians) for the moon shader,
/// and an aura tint for the surrounding glow and progress-bar accent.
///
/// All values are constants — no runtime mutation. Lerp between two
/// <see cref="PhaseLook"/> records to drive the 600 ms phase tween.
/// </summary>
public static class LunaPhaseLook
{
    public readonly record struct PhaseLook(double TerminatorRadians, Color AuraTint);

    private static readonly IReadOnlyDictionary<ProcessingPhase, PhaseLook> Map =
        new Dictionary<ProcessingPhase, PhaseLook>
        {
            // Idle is hidden — overlay isn't shown. Terminator wraps to keep the
            // crescent off-screen, tint is transparent.
            [ProcessingPhase.Idle]           = new(Math.PI,           Color.FromArgb(0,   0,   0,   0)),
            [ProcessingPhase.Scanning]       = new(2.6179938779,      Color.FromRgb(0x7a, 0x8c, 0xd6)), // 150°  cool blue
            [ProcessingPhase.Extracting]    = new(Math.PI / 2.0,      Color.FromRgb(0x9a, 0xaa, 0xde)), //  90°  blue→silver
            [ProcessingPhase.Grouping]       = new(Math.PI / 4.0,     Color.FromRgb(0xc9, 0xd4, 0xff)), //  45°  silver
            [ProcessingPhase.GeneratingHtml] = new(0.3490658504,      Color.FromRgb(0xdc, 0xd6, 0xc4)), //  20°  warm silver
            [ProcessingPhase.GeneratingPdf]  = new(0.1745329252,      Color.FromRgb(0xe6, 0xdc, 0xb8)), //  10°  warmer silver
            [ProcessingPhase.Finalizing]     = new(0.0,               Color.FromRgb(0xf3, 0xd2, 0x7a)), //   0°  gold
        };

    public static PhaseLook For(ProcessingPhase phase) =>
        Map.TryGetValue(phase, out var look) ? look : Map[ProcessingPhase.Idle];

    /// <summary>Linear interpolation between two looks. <paramref name="t"/> in [0..1].</summary>
    public static PhaseLook Lerp(PhaseLook a, PhaseLook b, double t)
    {
        t = Math.Clamp(t, 0.0, 1.0);
        var term = a.TerminatorRadians + (b.TerminatorRadians - a.TerminatorRadians) * t;
        var tint = Color.FromArgb(
            (byte)(a.AuraTint.A + (b.AuraTint.A - a.AuraTint.A) * t),
            (byte)(a.AuraTint.R + (b.AuraTint.R - a.AuraTint.R) * t),
            (byte)(a.AuraTint.G + (b.AuraTint.G - a.AuraTint.G) * t),
            (byte)(a.AuraTint.B + (b.AuraTint.B - a.AuraTint.B) * t));
        return new PhaseLook(term, tint);
    }

    /// <summary>
    /// Smoothstep ease-in-out: 3t² − 2t³. Cheap, no trig.
    /// </summary>
    public static double EaseInOut(double t)
    {
        t = Math.Clamp(t, 0.0, 1.0);
        return t * t * (3.0 - 2.0 * t);
    }

    /// <summary>
    /// Asserts every defined <see cref="ProcessingPhase"/> has a mapping and that
    /// terminator angles trend toward 0 as work progresses (Scanning → Finalizing).
    /// Called once at startup in Debug builds only.
    /// </summary>
    [Conditional("DEBUG")]
    public static void RunSelfChecks()
    {
        foreach (ProcessingPhase phase in Enum.GetValues<ProcessingPhase>())
        {
            Debug.Assert(Map.ContainsKey(phase), $"LunaPhaseLook missing mapping for {phase}");
        }

        Debug.Assert(For(ProcessingPhase.Scanning).TerminatorRadians > For(ProcessingPhase.Extracting).TerminatorRadians);
        Debug.Assert(For(ProcessingPhase.Extracting).TerminatorRadians > For(ProcessingPhase.Grouping).TerminatorRadians);
        Debug.Assert(For(ProcessingPhase.Grouping).TerminatorRadians > For(ProcessingPhase.GeneratingHtml).TerminatorRadians);
        Debug.Assert(For(ProcessingPhase.GeneratingHtml).TerminatorRadians > For(ProcessingPhase.GeneratingPdf).TerminatorRadians);
        Debug.Assert(For(ProcessingPhase.GeneratingPdf).TerminatorRadians > For(ProcessingPhase.Finalizing).TerminatorRadians);

        var midway = Lerp(For(ProcessingPhase.Scanning), For(ProcessingPhase.Finalizing), 0.5);
        Debug.Assert(midway.TerminatorRadians > 0 && midway.TerminatorRadians < Math.PI);
    }
}
```

- [ ] **Step 2: Wire `RunSelfChecks` into app startup**

Open `App.axaml.cs` and find the `OnFrameworkInitializationCompleted` method. At the very start of the method body, add:

```csharp
LunaApp.Views.Controls.LunaPhaseLook.RunSelfChecks();
```

(`[Conditional("DEBUG")]` makes this a no-op in Release builds.)

- [ ] **Step 3: Build and run in Debug**

Run: `dotnet build LunaApp.csproj -c Debug`
Then: `dotnet run --project LunaApp.csproj -c Debug` and let the window appear.
Expected: App launches without any `Debug.Assert` dialog. If an assertion dialog appears, the mapping is inconsistent — fix the offending row in `Map`.


---

## Task 3: Create the SKSL shader resource

**Files:**
- Create: `Views/Controls/MoonShader.sksl`
- Modify: `LunaApp.csproj` (register as `AvaloniaResource`)

- [ ] **Step 1: Create the shader file**

Create `Views/Controls/MoonShader.sksl` with this content:

```glsl
// Luna moon-body fragment shader. Renders a lit sphere with procedural craters,
// a terminator line driven by the host C# tween, and earth-shine on the dark
// limb so crescent phases never look invisible.
//
// Uniforms are pushed every frame from LunaShaderMoon.MoonBody.Render.

uniform float2 uResolution;     // Render size in pixels
uniform float  uTerminator;     // Radians; 0 = full lit, PI = new moon
uniform float3 uDayTint;        // RGB tint for the lit side
uniform float2 uLightOffset;    // Cursor parallax, each axis in [-1, +1]

float hash(float2 p) {
    p = fract(p * float2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float craterNoise(float2 uv) {
    // Two-octave hash noise. Static — same craters every frame.
    float n = 0.0;
    n += hash(floor(uv * 6.0)) * 0.6;
    n += hash(floor(uv * 13.0)) * 0.4;
    return n; // 0..1
}

half4 main(float2 fragCoord) {
    float2 uv = (fragCoord - 0.5 * uResolution) / (0.5 * uResolution.y);
    float r = length(uv);

    // Anti-alias the disc edge; alpha falls off in the last 2% of the radius.
    float edge = smoothstep(1.0, 0.98, r);
    if (r > 1.0) {
        return half4(0.0);
    }

    // Sphere normal at this UV.
    float z = sqrt(max(0.0, 1.0 - r * r));
    float3 n = float3(uv.x, uv.y, z);

    // Light direction from terminator + cursor parallax.
    float t = uTerminator;
    float3 L = normalize(float3(
        cos(t) + 0.15 * uLightOffset.x,
        0.3    + 0.15 * uLightOffset.y,
        sin(t)));

    float diffuse = max(dot(n, L), 0.0);

    // Albedo from craters: range 0.7..1.0 so dark spots are visible but not black.
    float albedo = 0.7 + 0.3 * craterNoise(uv);

    // Earth-shine on the dark side. 5% intensity, cool tint.
    float3 darkSide = (1.0 - diffuse) * 0.05 * float3(0.4, 0.5, 0.9);

    float3 lit = uDayTint * albedo * diffuse + darkSide;
    return half4(half3(lit) * half(edge), half(edge));
}
```

- [ ] **Step 2: Register as an Avalonia resource**

Open `LunaApp.csproj`. Locate the `<ItemGroup>` at lines 44–48 containing `<AvaloniaResource Include="Assets\**" />`. Add a new line below it:

```xml
<AvaloniaResource Include="Views\Controls\MoonShader.sksl" />
```

The block should now look like:

```xml
<ItemGroup>
    <Folder Include="Models\" />
    <Folder Include="Services\" />
    <AvaloniaResource Include="Assets\**" />
    <AvaloniaResource Include="Views\Controls\MoonShader.sksl" />
</ItemGroup>
```

- [ ] **Step 3: Build to verify the resource is picked up**

Run: `dotnet build LunaApp.csproj -c Debug`
Expected: Build succeeds. Any SKSL syntax errors won't show up here — they surface at runtime when `SKRuntimeEffect.Create` is called in Task 4.
---

## Task 4: Implement `MoonBody` (the shader-rendering subcontrol)

**Files:**
- Create: `Views/Controls/MoonBody.cs`

- [ ] **Step 1: Create the file**

Create `Views/Controls/MoonBody.cs` with this content:

```csharp
using System;
using System.Diagnostics;
using System.IO;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Platform;
using Avalonia.Rendering.SceneGraph;
using Avalonia.Skia;
using SkiaSharp;

namespace LunaApp.Views.Controls;

/// <summary>
/// Inner moon-body subcontrol. Owns the SKSL <see cref="SKRuntimeEffect"/> and
/// renders it each frame with the current uniforms. The host
/// <c>LunaShaderMoon</c> drives the uniforms via <see cref="SetUniforms"/> and
/// triggers re-render via <see cref="InvalidateVisual"/>.
///
/// If the shader fails to compile (driver/platform issue) the control draws a
/// static <see cref="RadialGradientBrush"/> ellipse approximating the prior
/// moon icon — overlay never crashes, always renders something.
/// </summary>
internal sealed class MoonBody : Control
{
    private static readonly Lazy<SKRuntimeEffect?> _effect = new(LoadEffect);

    private float _terminator = (float)Math.PI;          // Idle default
    private SKColor _dayTint = new(0xC9, 0xD4, 0xFF, 0xFF);
    private SKPoint _lightOffset = new(0f, 0f);

    public void SetUniforms(double terminatorRadians, Color dayTint, Vector lightOffset)
    {
        _terminator = (float)terminatorRadians;
        _dayTint = new SKColor(dayTint.R, dayTint.G, dayTint.B, dayTint.A);
        _lightOffset = new SKPoint(
            (float)Math.Clamp(lightOffset.X, -1.0, 1.0),
            (float)Math.Clamp(lightOffset.Y, -1.0, 1.0));
        InvalidateVisual();
    }

    public override void Render(DrawingContext context)
    {
        if (Bounds.Width <= 0 || Bounds.Height <= 0) return;

        var effect = _effect.Value;
        if (effect is null)
        {
            DrawFallback(context);
            return;
        }

        context.Custom(new ShaderDrawOp(new Rect(Bounds.Size), effect, _terminator, _dayTint, _lightOffset));
    }

    private void DrawFallback(DrawingContext context)
    {
        var brush = new RadialGradientBrush
        {
            GradientStops =
            {
                new GradientStop(Color.FromRgb(0xC9, 0xD4, 0xFF), 0),
                new GradientStop(Color.FromRgb(0x66, 0x79, 0xB8), 0.5),
                new GradientStop(Color.FromRgb(0x1A, 0x23, 0x42), 1),
            },
            Center = new RelativePoint(0.35, 0.35, RelativeUnit.Relative),
            GradientOrigin = new RelativePoint(0.35, 0.35, RelativeUnit.Relative),
            RadiusX = new RelativeScalar(0.5, RelativeUnit.Relative),
            RadiusY = new RelativeScalar(0.5, RelativeUnit.Relative),
        };
        context.DrawEllipse(brush, null, new Rect(Bounds.Size).Center, Bounds.Width / 2, Bounds.Height / 2);
    }

    private static SKRuntimeEffect? LoadEffect()
    {
        try
        {
            using var stream = AssetLoader.Open(new Uri("avares://LunaApp/Views/Controls/MoonShader.sksl"));
            using var reader = new StreamReader(stream);
            var sksl = reader.ReadToEnd();

            var effect = SKRuntimeEffect.CreateShader(sksl, out var error);
            if (effect is null)
            {
                Debug.WriteLine($"[LunaShaderMoon] SKSL compile failed: {error}");
            }
            return effect;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[LunaShaderMoon] Failed to load MoonShader.sksl: {ex}");
            return null;
        }
    }

    private sealed class ShaderDrawOp : ICustomDrawOperation
    {
        private readonly SKRuntimeEffect _effect;
        private readonly float _terminator;
        private readonly SKColor _dayTint;
        private readonly SKPoint _lightOffset;

        public ShaderDrawOp(Rect bounds, SKRuntimeEffect effect, float terminator, SKColor dayTint, SKPoint lightOffset)
        {
            Bounds = bounds;
            _effect = effect;
            _terminator = terminator;
            _dayTint = dayTint;
            _lightOffset = lightOffset;
        }

        public Rect Bounds { get; }
        public void Dispose() { }
        public bool Equals(ICustomDrawOperation? other) => false; // always re-draw
        public bool HitTest(Point p) => false;

        public void Render(ImmediateDrawingContext context)
        {
            var lease = context.TryGetFeature<ISkiaSharpApiLeaseFeature>()?.Lease();
            if (lease is null) return;
            using (lease)
            {
                var canvas = lease.SkCanvas;
                var w = (float)Bounds.Width;
                var h = (float)Bounds.Height;

                using var uniforms = new SKRuntimeEffectUniforms(_effect);
                uniforms.Add("uResolution", new[] { w, h });
                uniforms.Add("uTerminator", _terminator);
                uniforms.Add("uDayTint", new[] { _dayTint.Red / 255f, _dayTint.Green / 255f, _dayTint.Blue / 255f });
                uniforms.Add("uLightOffset", new[] { _lightOffset.X, _lightOffset.Y });

                using var shader = _effect.ToShader(uniforms);
                using var paint = new SKPaint { Shader = shader, IsAntialias = true };
                canvas.DrawRect(0, 0, w, h, paint);
            }
        }
    }
}
```

- [ ] **Step 2: Build to verify**

Run: `dotnet build LunaApp.csproj -c Debug`
Expected: Build succeeds. References to `ISkiaSharpApiLeaseFeature` resolve from `Avalonia.Skia` namespace.

---

## Task 5: Implement `LunaShaderMoon` (the composed control)

**Files:**
- Create: `Views/Controls/LunaShaderMoon.cs`

- [ ] **Step 1: Create the file**

Create `Views/Controls/LunaShaderMoon.cs` with this content:

```csharp
using System;
using Avalonia;
using Avalonia.Animation.Easings;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using LunaApp.Models;

namespace LunaApp.Views.Controls;

/// <summary>
/// The processing-overlay hero element: an aura halo, a SKSL-shaded moon body,
/// and two orbital rings. Phase changes tween over 600 ms; cursor parallax
/// nudges the shader light direction. Idle when <see cref="IsActive"/> is false
/// — timer stopped, control draws nothing.
/// </summary>
public sealed class LunaShaderMoon : TemplatedControl
{
    public static readonly StyledProperty<ProcessingPhase> PhaseProperty =
        AvaloniaProperty.Register<LunaShaderMoon, ProcessingPhase>(nameof(Phase), ProcessingPhase.Idle);

    public static readonly StyledProperty<bool> IsActiveProperty =
        AvaloniaProperty.Register<LunaShaderMoon, bool>(nameof(IsActive));

    public static readonly StyledProperty<double> OrbitalThroughputProperty =
        AvaloniaProperty.Register<LunaShaderMoon, double>(nameof(OrbitalThroughput), 1.0);

    public ProcessingPhase Phase
    {
        get => GetValue(PhaseProperty);
        set => SetValue(PhaseProperty, value);
    }

    public bool IsActive
    {
        get => GetValue(IsActiveProperty);
        set => SetValue(IsActiveProperty, value);
    }

    public double OrbitalThroughput
    {
        get => GetValue(OrbitalThroughputProperty);
        set => SetValue(OrbitalThroughputProperty, value);
    }

    private static readonly TimeSpan PhaseTweenDuration = TimeSpan.FromMilliseconds(600);
    private const double OuterRingPeriodSeconds = 30.0;
    private const double InnerRingPeriodSeconds = 45.0;

    private readonly DispatcherTimer _timer;
    private readonly MoonBody _moonBody;
    private readonly Border _aura;
    private readonly Ellipse _outerRing;
    private readonly Ellipse _innerRing;
    private readonly Ellipse _ringDot;

    private DateTime _lastTick = DateTime.UtcNow;
    private double _outerRingAngle;
    private double _innerRingAngle;

    private LunaPhaseLook.PhaseLook _currentLook = LunaPhaseLook.For(ProcessingPhase.Idle);
    private LunaPhaseLook.PhaseLook _tweenFrom;
    private LunaPhaseLook.PhaseLook _tweenTo;
    private DateTime _tweenStart;
    private bool _tweening;

    private Vector _lightOffset;

    private double _auraBreathT; // 0..1, drives sine breath

    public LunaShaderMoon()
    {
        Width = 180;
        Height = 180;

        _aura = new Border
        {
            Width = 130, Height = 130,
            CornerRadius = new CornerRadius(65),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            BoxShadow = new BoxShadows(new BoxShadow
            {
                Blur = 60,
                Spread = 0,
                Color = Colors.Transparent,
            }),
        };

        _outerRing = new Ellipse
        {
            Width = 170, Height = 170,
            Stroke = new SolidColorBrush(Color.FromArgb(0x40, 0xb4, 0xc8, 0xff)),
            StrokeThickness = 1,
            StrokeDashArray = new AvaloniaList<double> { 3, 6 },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransform = new RotateTransform(),
            RenderTransformOrigin = RelativePoint.Center,
        };

        _innerRing = new Ellipse
        {
            Width = 140, Height = 140,
            Stroke = new SolidColorBrush(Color.FromArgb(0x26, 0xb4, 0xc8, 0xff)),
            StrokeThickness = 1,
            StrokeDashArray = new AvaloniaList<double> { 2, 8 },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransform = new RotateTransform(),
            RenderTransformOrigin = RelativePoint.Center,
        };

        _ringDot = new Ellipse
        {
            Width = 4, Height = 4,
            Fill = new SolidColorBrush(Color.FromRgb(0xc9, 0xd4, 0xff)),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Effect = new Avalonia.Media.DropShadowEffect
            {
                Color = Color.FromRgb(0xc9, 0xd4, 0xff),
                BlurRadius = 8,
                OffsetX = 0,
                OffsetY = 0,
                Opacity = 0.9,
            },
            RenderTransform = new TranslateTransform(),
        };

        _moonBody = new MoonBody
        {
            Width = 110, Height = 110,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var grid = new Grid();
        grid.Children.Add(_aura);
        grid.Children.Add(_outerRing);
        grid.Children.Add(_innerRing);
        grid.Children.Add(_ringDot);
        grid.Children.Add(_moonBody);

        // TemplatedControl: install via VisualChildren since we don't use a template.
        ((ISetLogicalParent)grid).SetParent(this);
        VisualChildren.Add(grid);
        LogicalChildren.Add(grid);

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(16) };
        _timer.Tick += OnTick;
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        ApplyLookImmediate(LunaPhaseLook.For(Phase));
        if (IsActive) StartTimer();

        // Parallax: track cursor on the parent overlay (or this control if no parent).
        AddHandler(PointerMovedEvent, OnPointerMoved, Avalonia.Interactivity.RoutingStrategies.Tunnel | Avalonia.Interactivity.RoutingStrategies.Bubble);
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        StopTimer();
        RemoveHandler(PointerMovedEvent, OnPointerMoved);
        base.OnDetachedFromVisualTree(e);
    }

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == IsActiveProperty)
        {
            if ((bool)change.NewValue!) StartTimer();
            else StopTimer();
        }
        else if (change.Property == PhaseProperty)
        {
            BeginTween(LunaPhaseLook.For((ProcessingPhase)change.NewValue!));
        }
    }

    private void StartTimer()
    {
        _lastTick = DateTime.UtcNow;
        if (!_timer.IsEnabled) _timer.Start();
    }

    private void StopTimer()
    {
        if (_timer.IsEnabled) _timer.Stop();
    }

    private void BeginTween(LunaPhaseLook.PhaseLook target)
    {
        _tweenFrom = _currentLook;
        _tweenTo = target;
        _tweenStart = DateTime.UtcNow;
        _tweening = true;
    }

    private void ApplyLookImmediate(LunaPhaseLook.PhaseLook look)
    {
        _currentLook = look;
        _tweening = false;
        PushLookToVisuals(look);
    }

    private void PushLookToVisuals(LunaPhaseLook.PhaseLook look)
    {
        _moonBody.SetUniforms(look.TerminatorRadians, look.AuraTint, _lightOffset);

        // Aura BoxShadow color follows the tint; opacity from breathing oscillator.
        var auraColor = look.AuraTint;
        var breathOpacity = 0.6 + 0.25 * (Math.Sin(_auraBreathT * Math.PI * 2) * 0.5 + 0.5);
        var auraColorWithBreath = Color.FromArgb((byte)(auraColor.A * breathOpacity), auraColor.R, auraColor.G, auraColor.B);

        _aura.BoxShadow = new BoxShadows(new BoxShadow
        {
            Blur = 60,
            Spread = 0,
            Color = auraColorWithBreath,
        });

        // Ring dot tint follows aura too.
        if (_ringDot.Fill is SolidColorBrush dotBrush) dotBrush.Color = auraColor;
    }

    private void OnTick(object? sender, EventArgs e)
    {
        var now = DateTime.UtcNow;
        var dt = (now - _lastTick).TotalSeconds;
        _lastTick = now;

        // Phase tween.
        if (_tweening)
        {
            var elapsed = (now - _tweenStart).TotalMilliseconds;
            var t = LunaPhaseLook.EaseInOut(elapsed / PhaseTweenDuration.TotalMilliseconds);
            _currentLook = LunaPhaseLook.Lerp(_tweenFrom, _tweenTo, t);
            if (elapsed >= PhaseTweenDuration.TotalMilliseconds)
            {
                _currentLook = _tweenTo;
                _tweening = false;
            }
        }

        // Aura breathing oscillator: 3s period.
        _auraBreathT = (_auraBreathT + dt / 3.0) % 1.0;

        // Ring rotation (degrees), modulated by OrbitalThroughput.
        var throughput = Math.Max(0.0, OrbitalThroughput);
        _outerRingAngle = (_outerRingAngle + 360.0 * dt / OuterRingPeriodSeconds * throughput) % 360.0;
        _innerRingAngle = (_innerRingAngle - 360.0 * dt / InnerRingPeriodSeconds * throughput + 360.0) % 360.0;

        if (_outerRing.RenderTransform is RotateTransform o) o.Angle = _outerRingAngle;
        if (_innerRing.RenderTransform is RotateTransform i) i.Angle = _innerRingAngle;

        // Ring dot travels along outer ring radius (170/2 = 85).
        const double dotRadius = 85.0;
        var rad = _outerRingAngle * Math.PI / 180.0;
        if (_ringDot.RenderTransform is TranslateTransform tr)
        {
            tr.X = Math.Cos(rad) * dotRadius;
            tr.Y = Math.Sin(rad) * dotRadius;
        }

        PushLookToVisuals(_currentLook);
    }

    private void OnPointerMoved(object? sender, PointerEventArgs e)
    {
        var pos = e.GetPosition(this);
        var cx = Bounds.Width / 2.0;
        var cy = Bounds.Height / 2.0;
        // Normalize to [-1, +1] across the control width/height.
        var nx = (pos.X - cx) / cx;
        var ny = (pos.Y - cy) / cy;
        _lightOffset = new Vector(Math.Clamp(nx, -1.0, 1.0), Math.Clamp(ny, -1.0, 1.0));
    }
}
```

- [ ] **Step 2: Build**

Run: `dotnet build LunaApp.csproj -c Debug`
Expected: Build succeeds. If `BoxShadows`/`BoxShadow` constructor or `RenderTransform` types disagree with the installed Avalonia version, adjust per the compiler error — these are stable Avalonia 11 APIs.

---

## Task 6: Wire the new control into MainWindow.axaml

**Files:**
- Modify: `Views/MainWindow.axaml` (root xmlns + lines 459–464)

- [ ] **Step 1: Add the controls namespace to the root element**

Open `Views/MainWindow.axaml`. In the `<Window …>` opening tag (lines 1–15), add this namespace declaration alongside the existing ones:

```xml
xmlns:controls="using:LunaApp.Views.Controls"
```

It should sit beside the existing `xmlns:mi=…` line.

- [ ] **Step 2: Replace the moon Grid block**

In the same file, find the block at lines 459–464:

```xml
<!-- Moon icon with pulsing effect -->
<Grid Width="120" Height="120" HorizontalAlignment="Center">
    <Border Width="120" Height="120" CornerRadius="60" BorderBrush="{DynamicResource LunaBgTertiary}" BorderThickness="4"/>
    <Border Width="80" Height="80" CornerRadius="40" Background="{DynamicResource LunaBgSecondary}" HorizontalAlignment="Center" VerticalAlignment="Center">
        <mi:MaterialIcon Classes="pulse" Kind="MoonWaningCrescent" Width="40" Height="40" Foreground="{DynamicResource LunaAccent}"/>
    </Border>
</Grid>
```

Replace it with:

```xml
<!-- Hero moon: SKSL shader body, orbital rings, atmospheric aura -->
<controls:LunaShaderMoon Width="180" Height="180"
                         Phase="{Binding CurrentPhase}"
                         IsActive="{Binding IsProcessing}"
                         HorizontalAlignment="Center"/>
```

- [ ] **Step 3: Build and run**

Run: `dotnet build LunaApp.csproj -c Debug` then `dotnet run --project LunaApp.csproj -c Debug`.
Expected: App launches normally. Trigger a folder scan; the overlay shows the new control rendering. Moon visible, rings rotating, aura glowing.

- [ ] **Step 4: Verify visually**

With a folder scan in progress, confirm:
- Moon is rendered as a lit sphere (not a flat icon)
- Two dashed rings rotate around it in opposite directions
- A small dot travels along the outer ring
- Aura softly breathes around the moon
- Moving the mouse across the overlay subtly shifts the lit side of the moon

If the moon looks wrong (e.g., upside down, off-center, not lit), inspect SKSL compile output via the existing log viewer (Ctrl-Shift-L per the dev panel). If the fallback ellipse renders instead, the shader compile failed — fix the SKSL syntax.

---

## Task 7: Reskin the progress bar and ETA text

**Files:**
- Modify: `Styles/Controls.axaml` (add new styles)
- Modify: `Views/MainWindow.axaml:486-498` (apply classes)

- [ ] **Step 1: Add the new styles**

Open `Styles/Controls.axaml`. At the end of the file (before the closing `</Styles>` tag), add:

```xml
<!-- ==================== Cosmic progress bar (processing overlay) ==================== -->
<!-- Used inside the LunaShaderMoon overlay. Foreground is a soft gradient that
     picks up a small drop-shadow glow; designed to read as "luminous", not
     "industrial". -->
<Style Selector="ProgressBar.luna-cosmic">
    <Setter Property="Background" Value="{DynamicResource LunaBgTertiary}"/>
    <Setter Property="CornerRadius" Value="4"/>
    <Setter Property="Foreground">
        <Setter.Value>
            <LinearGradientBrush StartPoint="0%,50%" EndPoint="100%,50%">
                <GradientStop Color="#7a8cd6" Offset="0"/>
                <GradientStop Color="#c9d4ff" Offset="1"/>
            </LinearGradientBrush>
        </Setter.Value>
    </Setter>
    <Setter Property="Effect">
        <Setter.Value>
            <DropShadowEffect Color="#7a8cd6" BlurRadius="10" OffsetX="0" OffsetY="0" Opacity="0.6"/>
        </Setter.Value>
    </Setter>
</Style>

<!-- ==================== ETA text (mono digits) ==================== -->
<!-- Tabular monospace stops the digits jittering as the ETA counts down. -->
<Style Selector="TextBlock.luna-eta">
    <Setter Property="FontFamily" Value="Cascadia Mono, Consolas, Courier New, monospace"/>
</Style>

<!-- ==================== Phase label (slightly tracked) ==================== -->
<Style Selector="TextBlock.luna-phase-label">
    <Setter Property="LetterSpacing" Value="0.5"/>
</Style>
```

- [ ] **Step 2: Apply classes in MainWindow.axaml**

Open `Views/MainWindow.axaml`. Find the `PhaseLabel` TextBlock at line 468 and add `Classes="luna-phase-label"`:

```xml
<TextBlock Text="{Binding PhaseLabel}"
           Classes="luna-phase-label"
           FontSize="{DynamicResource LunaFontSizeXl}"
           FontWeight="SemiBold"
           HorizontalAlignment="Center"
           TextAlignment="Center"/>
```

Find the `EtaText` TextBlock at line 480 and add `Classes="luna-eta"`:

```xml
<TextBlock Text="{Binding EtaText}"
           Classes="luna-eta"
           FontSize="{DynamicResource LunaFontSizeSm}"
           Foreground="{DynamicResource LunaTextMuted}"
           HorizontalAlignment="Center"/>
```

Find the `ProgressBar` at line 488 and add `Classes="luna-cosmic"`:

```xml
<ProgressBar Value="{Binding Progress}"
             Classes="luna-cosmic"
             Maximum="100"
             Height="8"
             Background="{DynamicResource LunaBgTertiary}"
             CornerRadius="4"/>
```

(Remove the inline `Foreground="{DynamicResource LunaAccent}"` — the new style supplies a gradient.)

- [ ] **Step 3: Build and run**

Run: `dotnet run --project LunaApp.csproj -c Debug` and trigger a scan.
Expected: Progress bar glows softly. ETA text uses monospace digits. Phase label has slightly looser letter spacing.

---

## Task 8: Add `DevSimulateProcessingCommand` for visual QA

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs` (add command)
- Modify: `Views/MainWindow.axaml` (add button in dev panel)

- [ ] **Step 1: Add the command**

In `ViewModels/MainWindowViewModel.cs`, find a suitable section near other `Dev…` commands (search for `DevShowUpdateBanner` or `DevSimulateDownload`). Add this method:

```csharp
[RelayCommand]
private async Task DevSimulateProcessingAsync()
{
    if (IsProcessing) return;
    IsProcessing = true;
    try
    {
        var phases = new[]
        {
            ProcessingPhase.Scanning,
            ProcessingPhase.Extracting,
            ProcessingPhase.Grouping,
            ProcessingPhase.GeneratingHtml,
            ProcessingPhase.GeneratingPdf,
            ProcessingPhase.Finalizing,
        };
        for (var i = 0; i < phases.Length; i++)
        {
            CurrentPhase = phases[i];
            PhaseLabel = phases[i].ToString();
            PhaseDetail = $"Simulating {phases[i]}";
            EtaText = $"{(phases.Length - i) * 2}s remaining";
            for (var p = 0; p <= 100; p += 10)
            {
                Progress = p;
                await Task.Delay(200);
            }
        }
    }
    finally
    {
        IsProcessing = false;
        CurrentPhase = ProcessingPhase.Idle;
        Progress = 0;
        PhaseLabel = string.Empty;
        PhaseDetail = string.Empty;
        EtaText = string.Empty;
    }
}
```

If `Task` and `Task.Delay` aren't already imported, add `using System.Threading.Tasks;` to the top of the file.

- [ ] **Step 2: Add the dev-panel button**

Open `Views/MainWindow.axaml`. Find the dev-tools panel `StackPanel` near line 544 (`<StackPanel Orientation="Horizontal" Spacing="8">` containing `DevShowUpdateBanner`). Add this button alongside the others:

```xml
<Button Classes="secondary" Command="{Binding DevSimulateProcessingCommand}" Padding="10,6">
    <TextBlock Text="Simulate Processing" FontSize="{DynamicResource LunaFontSizeXs}"/>
</Button>
```

- [ ] **Step 3: Build, run, exercise**

Run: `dotnet run --project LunaApp.csproj -c Debug`. Open the dev panel (Ctrl-Shift-L per the existing toggle), click "Simulate Processing".
Expected: Overlay shows. Moon cycles through Scanning → Finalizing over ~12s. Each phase change visibly tweens the terminator over ~600ms (crescent → quarter → gibbous → full). Aura color shifts blue → silver → gold across the cycle. Rings keep rotating throughout.

---

## Task 9: Final visual acceptance pass

**Files:** none — manual QA only.

- [ ] **Step 1: Run on real workload**

Run: `dotnet run --project LunaApp.csproj -c Debug` and drop a real folder of camera footage to trigger the full scan/parse pipeline.
Expected: All overlay phases (Scanning → Extracting → Grouping → GeneratingHtml → GeneratingPdf → Finalizing) render with the correct moon shape, aura color, and progress-bar tint per the spec table.

- [ ] **Step 2: Test the shader fallback**

Temporarily corrupt `Views/Controls/MoonShader.sksl` (e.g., replace `half4 main` with `half4 main BROKEN`). Rebuild and run.
Expected: A radial-gradient ellipse renders in place of the shader moon. No crash. The overlay behaves identically otherwise. Revert the SKSL file.

- [ ] **Step 3: Test idle cost**

Click "Simulate Processing", wait until it ends, observe Task Manager / process explorer.
Expected: When `IsActive=false` (overlay dismissed), `LunaApp.exe` CPU returns to its baseline. The 60Hz timer is stopped.

- [ ] **Step 4: Release-build smoke test**

Run: `dotnet build LunaApp.csproj -c Release` and `dotnet run --project LunaApp.csproj -c Release`.
Trigger a scan.
Expected: Shader compiles and renders identically in Release. `LunaPhaseLook.RunSelfChecks` is compiled out.

- [ ] **Step 5: final polish**

If nothing changed, skip this step.

---

## Out of Scope (per spec, deferred)

- Completion-flourish flare on Finalizing → done
- Surface data-ripple effect during Extracting
- Scanning sweep beam
- Per-percent moon morphing within a phase
- Wiring `OrbitalThroughput` to actual clips-per-second throughput (property exists; binding is a follow-up)
