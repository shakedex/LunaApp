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

    private float _terminator = (float)Math.PI;
    private SKColor _dayTint = new(0xC9, 0xD4, 0xFF, 0xFF);

    public void SetUniforms(double terminatorRadians, Color dayTint)
    {
        _terminator = (float)terminatorRadians;
        _dayTint = new SKColor(dayTint.R, dayTint.G, dayTint.B, dayTint.A);
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

        context.Custom(new ShaderDrawOp(new Rect(Bounds.Size), effect, _terminator, _dayTint));
    }

    private void DrawFallback(DrawingContext context)
    {
        // Bright red fallback so the failure is impossible to miss in QA.
        // If you see this, the SKSL shader did not compile — check Debug output
        // for "[LunaShaderMoon] SKSL compile failed".
        var brush = new SolidColorBrush(Color.FromRgb(0xff, 0x33, 0x55));
        context.DrawEllipse(brush, null, new Rect(Bounds.Size).Center, Bounds.Width / 2, Bounds.Height / 2);
    }

    private static SKRuntimeEffect? LoadEffect()
    {
        try
        {
            using var stream = AssetLoader.Open(new Uri("avares://Luna/Views/Controls/MoonShader.sksl"));
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

        public ShaderDrawOp(Rect bounds, SKRuntimeEffect effect, float terminator, SKColor dayTint)
        {
            Bounds = bounds;
            _effect = effect;
            _terminator = terminator;
            _dayTint = dayTint;
        }

        public Rect Bounds { get; }
        public void Dispose() { }
        public bool Equals(ICustomDrawOperation? other) => false;
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

                using var shader = _effect.ToShader(uniforms);
                using var paint = new SKPaint { Shader = shader, IsAntialias = true };
                canvas.DrawRect(0, 0, w, h, paint);
            }
        }
    }
}
