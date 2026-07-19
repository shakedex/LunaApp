# Lunar Scan Overlay — Design Spec

**Date:** 2026-04-27
**Status:** Approved (brainstorm phase)
**Scope:** Replace the static moon-icon overlay shown during scan/parse with a hero shader-rendered moon, orbital rings, atmospheric aura, and phase-driven progress reskin.

## Goal

The processing overlay (`MainWindow.axaml` lines 452–514) shown while the app scans folders and extracts clip metadata currently displays a `MoonWaningCrescent` material icon with a `pulse` CSS class. The pulse is barely noticeable and provides no perceptual feedback about which phase is active. This spec defines a replacement that:

- Reads as the "wow moment" of the app
- Stays visually anchored to the LunaApp brand (lunar/celestial)
- Communicates which `ProcessingPhase` is active at a glance, without reading the label
- Costs nothing when the overlay is hidden

## Direction

Lunar / Celestial Tech. A hero element only — no full-screen scene, no nebula background. The current dark overlay background is preserved.

## Architecture

A single new Avalonia control: **`LunaShaderMoon`** in `Views/Controls/`.

The control composes three independent visual layers, bottom to top:

1. **Aura** — `Border` with animated `BoxShadow`, color and opacity tween with phase. Avalonia primitives, no shader.
2. **Moon body** — `Control` subclass overriding `Render`, draws a SKSL fragment shader via `SKRuntimeEffect`/`SKShader` into the available bounds.
3. **Orbital rings** — `Canvas` with two dashed `Ellipse` paths and a single travelling dot, animated via `RotateTransform`.

A single `DispatcherTimer` (60 Hz, started/stopped with `IsActive`) drives ring rotation, aura breathing, and shader uniform updates. Phase tweening is computed in C# and pushed to the shader as uniforms each tick.

### Layout sizes

The control is 180×180 overall. Inner elements are sized so each layer is visually distinct (moon clearly inside aura, aura clearly inside rings):

| Layer | Diameter / size |
|---|---|
| `LunaShaderMoon` control | 180×180 |
| Outer ring (Ellipse) | 170 |
| Inner ring (Ellipse) | 140 |
| Aura halo (`Border` with `BoxShadow`) | 130×130 |
| Moon body shader region | 110×110 |

The moon body is implemented as a child `Control` sized 110×110, centered in the parent `Grid`. Its `Render` override paints the SKSL shader only over its own 110×110 bounds, not the full 180. The aura `Border`'s `BoxShadow` blur extends outward past the moon edge so the glow appears around it without clipping.

### Public API

| Property | Type | Purpose |
|---|---|---|
| `Phase` | `ProcessingPhase` | Drives terminator angle and aura tint. Setting triggers a 600ms ease-in-out tween from the current values to the new target. |
| `IsActive` | `bool` | Starts/stops the internal `DispatcherTimer`. When `false`, the control consumes zero CPU/GPU. |
| `OrbitalThroughput` | `double` (default 1.0) | Multiplier on ring rotation speed. Reserved for a future binding to clips-per-second; not wired in this spec. |

### Files

New:
- `Views/Controls/LunaShaderMoon.cs` — control class
- `Views/Controls/LunaPhaseLook.cs` — static phase → (terminator, aura tint) lookup, kept separate so it is unit-testable
- `Views/Controls/MoonShader.sksl` — embedded resource containing the SKSL source

Modified:
- `Views/MainWindow.axaml` — replace the `Grid` at lines 459–464 with `<controls:LunaShaderMoon … />` and reskin the surrounding text + progress bar
- `ViewModels/MainWindowViewModel.cs` — add `CurrentPhase` observable property; assign in `OnProgressReported`
- `App.axaml` — new `ProgressBar.luna-cosmic` style and `TextBlock.luna-eta` style (mono numerals)

## Phase Mapping

Each `ProcessingPhase` maps to a terminator angle (moon shape) and an aura tint. Transitions animate over 600ms ease-in-out when `Phase` changes.

| Phase | Terminator | Visual | Aura tint |
|---|---|---|---|
| Idle | 180° | hidden | none (overlay not shown) |
| Scanning | 150° | thin waxing crescent | cool blue `#7a8cd6` |
| Extracting | 90° | first quarter | cool blue → silver `#9aaade` |
| Grouping | 45° | waxing gibbous | silver `#c9d4ff` |
| GeneratingHtml | 20° | nearly full | warm silver `#dcd6c4` |
| GeneratingPdf | 10° | almost full | warm silver `#e6dcb8` |
| Finalizing | 0° | full bright | gold `#f3d27a` |

The mapping table lives in `LunaPhaseLook.cs` as a `static IReadOnlyDictionary<ProcessingPhase, PhaseLook>`. No per-percent morphing within a phase — phase change is the only animation trigger for the terminator. Progress percentage is communicated by the progress bar reskin, not by the moon shape.

## SKSL Shader (moon body)

A single fragment shader, ~60 lines of SKSL. Compiled once via `SKRuntimeEffect.Create` and cached in a static field on `LunaShaderMoon`.

### Uniforms

- `uResolution` (vec2) — current render size in pixels
- `uTerminator` (float, radians) — current tweened terminator angle
- `uDayTint` (vec3) — current tweened RGB for the lit side
- `uLightOffset` (vec2) — cursor offset for parallax (–1..+1 each axis, clamped)

### Per-fragment pipeline

1. Compute `uv = (sk_FragCoord - 0.5 * uResolution) / (0.5 * uResolution.y)` — centered, unit-radius circle
2. `r = length(uv)`. Discard outside `r > 1.0`. Anti-alias the edge via `smoothstep(1.0, 0.98, r)` as alpha
3. Sphere normal: `n = vec3(uv.x, uv.y, sqrt(max(0.0, 1.0 - r*r)))`
4. Light direction from `uTerminator`, nudged by parallax: `L = normalize(vec3(cos(t) + 0.15*uLightOffset.x, 0.3 + 0.15*uLightOffset.y, sin(t)))`
5. Lambert diffuse: `d = max(dot(n, L), 0.0)`
6. Procedural craters: 2-octave hash-noise on `n.xy * 6.0`, multiplied into albedo (range 0.7–1.0). Static — same craters every frame, no time uniform.
7. Earth-shine on dark side: `darkSide = (1 - d) * 0.05 * vec3(0.4, 0.5, 0.9)` — keeps the dark limb visible during crescent phases instead of going invisible
8. Final color: `(uDayTint * albedo * d + darkSide)` with edge alpha; output as premultiplied

### Out of scope (YAGNI)

- No `uTime` uniform. The shader is static aside from terminator/tint/parallax uniforms driven from C#.
- No surface ripples, no normal-mapped crater bumps, no animated clouds. Add in a follow-up iteration if the result feels flat.

## Other Layers

### Orbital rings

- Two `Ellipse` elements in a `Canvas` centered on the moon
- Outer: 180px diameter, 1px dashed stroke, alpha 0.25, rotates clockwise, ~30s/revolution
- Inner: 150px diameter, 1px dashed stroke, alpha 0.15, rotates counter-clockwise, ~45s/revolution
- A 3px accent-colored `Ellipse` dot with a soft glow travels around the outer ring (parameterized by current angle)
- All driven by the single `DispatcherTimer`. `OrbitalThroughput` multiplies the angular delta per tick.

### Parallax tilt

- `PointerMoved` on the overlay computes the cursor offset from the moon center, normalized to [–1, +1] in each axis
- Stored on the control as `_lightOffset` and pushed to the shader as `uLightOffset` each frame
- Effect: lit side shifts toward the cursor, giving an authentic 3D-sphere-lit-by-viewer feel
- No transforms applied to rings or aura. Only the shader light direction moves.

### Aura

- `Border` 180×180 placed behind the moon, no fill, `BoxShadow` with 60px blur radius, color = current aura tint
- "Breathing": opacity animates 0.6 → 0.85 → 0.6 over 3s, sine ease, repeats while `IsActive`
- Phase change cross-fades the shadow color over 600ms, tweened from the same source as the terminator

### Progress bar reskin (UI choice B)

A new XAML style `ProgressBar.luna-cosmic` in `App.axaml`:

- `Foreground` is a `LinearGradientBrush` from the current aura tint to a 20%-lighter version of the same tint. Stops are bound (via VM) so they animate with the phase tween.
- 4px `DropShadowEffect` on the foreground bar, color = aura tint, low blur radius
- `PhaseLabel` gets `LetterSpacing="0.5"` (about 0.05em at the current font size)
- `EtaText` switches to `FontFamily="Cascadia Mono, Consolas, monospace"` for stable digit width

## ViewModel Wiring

In `MainWindowViewModel.cs`:

- Add `[ObservableProperty] private ProcessingPhase _currentPhase = ProcessingPhase.Idle;`
- In `OnProgressReported`, after `Progress = report.Percent;` and `PhaseLabel = report.PhaseLabel;`, add `CurrentPhase = report.Phase;`
- Existing `IsProcessing` already drives the overlay's `Classes.active` and remains the source for `LunaShaderMoon.IsActive`

## XAML Change

Replace lines 459–464 of `Views/MainWindow.axaml` (the inner `<Grid Width="120" …>` block) with:

```xml
<controls:LunaShaderMoon Width="180" Height="180"
                         Phase="{Binding CurrentPhase}"
                         IsActive="{Binding IsProcessing}"
                         HorizontalAlignment="Center"/>
```

Size grows 120 → 180 to give the orbital rings room. Add `xmlns:controls="using:LunaApp.Views.Controls"` to the root element.

The surrounding text stack (`PhaseLabel`, `PhaseDetail`, `EtaText`) keeps its layout. The `ProgressBar` adds `Classes="luna-cosmic"`, and `EtaText` adds `Classes="luna-eta"`.

## Lifecycle and Performance

- `LunaShaderMoon` overrides `OnAttachedToVisualTree` and `OnDetachedFromVisualTree`
- When `IsActive` becomes `true`: start the `DispatcherTimer` at 60 Hz; each tick advances ring rotation, advances any in-flight phase tween, and calls `InvalidateVisual`
- When `IsActive` becomes `false`: stop the timer; the control draws nothing; CPU and GPU usage return to zero
- The 180×180 fragment count (~32K) is trivial for any GPU that can run Avalonia at all

## Fallback

If `SKRuntimeEffect.Create(SKSL)` returns null or throws (older driver, broken Skia build), the control logs a warning once and renders a static `RadialGradientBrush` ellipse approximating the current moon icon. The control never crashes the overlay; the overlay always renders something.

## Testing

- `LunaPhaseLook` is a pure static map, trivially unit-testable: assert each `ProcessingPhase` returns the documented terminator angle and tint.
- Visual QA: extend the existing dev tools panel (`MainWindow.axaml` lines 519+, gated by `IsDevPanelVisible`) with a new `DevSimulateProcessingCommand` that cycles through `ProcessingPhase` values with a 2s dwell on each, so the moon's phase progression and aura crossfade can be reviewed without running a full ingest.
- Shader compile-fail path: temporarily inject a syntax error in `MoonShader.sksl` during dev, confirm the fallback ellipse renders.

## Out of Scope (explicitly deferred)

- The completion flourish (final flare burst on Finalizing → done) was discussed but not selected.
- Surface data ripple during Extracting was not selected.
- Scanning sweep beam was not selected.
- Per-percent moon morphing within a phase was not selected.
- Wiring `OrbitalThroughput` to actual clips-per-second throughput. The property exists; the binding is a follow-up.
