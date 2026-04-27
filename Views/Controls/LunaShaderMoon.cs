using System;
using Avalonia;
using Avalonia.Collections;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Shapes;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

namespace LunaApp.Views.Controls;

/// <summary>
/// Processing-overlay moon. A SKSL-shaded sphere whose terminator follows
/// <see cref="Progress"/> — 0 % is a thin crescent, 100 % is fully lit. A
/// soft static aura halo and two slow-rotating dashed orbital rings frame
/// the moon to give it presence without the busyness of breathing/parallax.
/// </summary>
public sealed class LunaShaderMoon : TemplatedControl
{
    public static readonly StyledProperty<double> ProgressProperty =
        AvaloniaProperty.Register<LunaShaderMoon, double>(nameof(Progress));

    public static readonly StyledProperty<bool> IsActiveProperty =
        AvaloniaProperty.Register<LunaShaderMoon, bool>(nameof(IsActive));

    public double Progress
    {
        get => GetValue(ProgressProperty);
        set => SetValue(ProgressProperty, value);
    }

    public bool IsActive
    {
        get => GetValue(IsActiveProperty);
        set => SetValue(IsActiveProperty, value);
    }

    private static readonly TimeSpan TweenDuration = TimeSpan.FromMilliseconds(450);
    private static readonly Color DayTint = Color.FromRgb(0xc9, 0xd4, 0xff);

    private const double OuterRingPeriodSeconds = 60.0;
    private const double InnerRingPeriodSeconds = 90.0;

    private readonly DispatcherTimer _timer;
    private readonly MoonBody _moonBody;
    private readonly Border _aura;
    private readonly Ellipse _outerRing;
    private readonly Ellipse _innerRing;
    private readonly Ellipse _ringDot;

    private DateTime _lastTick = DateTime.UtcNow;
    private double _outerRingAngle;
    private double _innerRingAngle;

    private double _currentTerminator = Math.PI;
    private double _tweenFrom;
    private double _tweenTo;
    private DateTime _tweenStart;
    private bool _tweening;

    public LunaShaderMoon()
    {
        Width = 200;
        Height = 200;

        _aura = new Border
        {
            Width = 150,
            Height = 150,
            CornerRadius = new CornerRadius(75),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            BoxShadow = new BoxShadows(new BoxShadow
            {
                Blur = 35,
                Spread = 0,
                Color = Color.FromArgb(0x80, 0x7a, 0x8c, 0xd6),
            }),
        };

        _outerRing = new Ellipse
        {
            Width = 190,
            Height = 190,
            Stroke = new SolidColorBrush(Color.FromArgb(0x33, 0xb4, 0xc8, 0xff)),
            StrokeThickness = 1,
            StrokeDashArray = new AvaloniaList<double> { 2, 7 },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransform = new RotateTransform(),
            RenderTransformOrigin = RelativePoint.Center,
        };

        _innerRing = new Ellipse
        {
            Width = 160,
            Height = 160,
            Stroke = new SolidColorBrush(Color.FromArgb(0x1c, 0xb4, 0xc8, 0xff)),
            StrokeThickness = 1,
            StrokeDashArray = new AvaloniaList<double> { 1, 9 },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransform = new RotateTransform(),
            RenderTransformOrigin = RelativePoint.Center,
        };

        _ringDot = new Ellipse
        {
            Width = 3,
            Height = 3,
            Fill = new SolidColorBrush(Color.FromArgb(0xcc, 0xc9, 0xd4, 0xff)),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransform = new TranslateTransform(),
        };

        _moonBody = new MoonBody
        {
            Width = 120,
            Height = 120,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var grid = new Grid();
        grid.Children.Add(_aura);
        grid.Children.Add(_outerRing);
        grid.Children.Add(_innerRing);
        grid.Children.Add(_ringDot);
        grid.Children.Add(_moonBody);

        ((ISetLogicalParent)grid).SetParent(this);
        VisualChildren.Add(grid);
        LogicalChildren.Add(grid);

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(16) };
        _timer.Tick += OnTick;
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        _currentTerminator = TerminatorForProgress(Progress);
        _tweening = false;
        _moonBody.SetUniforms(_currentTerminator, DayTint);
        if (IsActive) StartTimer();
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        StopTimer();
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
        else if (change.Property == ProgressProperty)
        {
            BeginTween(TerminatorForProgress((double)change.NewValue!));
        }
    }

    private static double TerminatorForProgress(double progress)
    {
        var p = Math.Clamp(progress, 0.0, 100.0) / 100.0;
        return Math.PI * (1.0 - p);
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

    private void BeginTween(double target)
    {
        _tweenFrom = _currentTerminator;
        _tweenTo = target;
        _tweenStart = DateTime.UtcNow;
        _tweening = true;
        if (!_timer.IsEnabled && IsActive) _timer.Start();
    }

    private void OnTick(object? sender, EventArgs e)
    {
        var now = DateTime.UtcNow;
        var dt = (now - _lastTick).TotalSeconds;
        _lastTick = now;

        if (_tweening)
        {
            var elapsed = (now - _tweenStart).TotalMilliseconds;
            var t = Math.Clamp(elapsed / TweenDuration.TotalMilliseconds, 0.0, 1.0);
            var eased = t * t * (3.0 - 2.0 * t);
            _currentTerminator = _tweenFrom + (_tweenTo - _tweenFrom) * eased;
            if (t >= 1.0)
            {
                _currentTerminator = _tweenTo;
                _tweening = false;
            }
            _moonBody.SetUniforms(_currentTerminator, DayTint);
        }

        _outerRingAngle = (_outerRingAngle + 360.0 * dt / OuterRingPeriodSeconds) % 360.0;
        _innerRingAngle = (_innerRingAngle - 360.0 * dt / InnerRingPeriodSeconds + 360.0) % 360.0;

        if (_outerRing.RenderTransform is RotateTransform o) o.Angle = _outerRingAngle;
        if (_innerRing.RenderTransform is RotateTransform i) i.Angle = _innerRingAngle;

        const double dotRadius = 95.0;
        var rad = _outerRingAngle * Math.PI / 180.0;
        if (_ringDot.RenderTransform is TranslateTransform tr)
        {
            tr.X = Math.Cos(rad) * dotRadius;
            tr.Y = Math.Sin(rad) * dotRadius;
        }
    }
}
