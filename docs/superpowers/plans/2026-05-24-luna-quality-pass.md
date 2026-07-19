# Luna Quality Pass — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Luna's toast-based notifications with three contextual surfaces (state strip + banner stack + inline banner), surface previously-silent failures with user-friendly messages, add a success-hold to the hero overlay after report generation, and clear the high-confidence backlog from the 2026-05-24 audit.

**Architecture:** Phase A introduces foundation controls and types (`ExceptionMapper`, `StateStrip`, `BannerStack`, `InlineBanner`, `OverlayState`). Phase B replaces the existing status-text / toast mechanisms with the new foundation and wires the behaviour fixes (H1, H2, H5, M1, M2, M6) plus Wave 3 copy edits. Phase C lands independent backlog fixes (H3, M3, M4, M5, M10, M11, L1, L2).

**Tech Stack:** .NET 10, Avalonia 11.3.14, CommunityToolkit.Mvvm (source-gen `[ObservableProperty]`/`[RelayCommand]`), Serilog, FFmpeg.AutoGen 7.1.1, MediaInfo.Wrapper.Core 26.1.0.

**Spec:** `docs/superpowers/specs/2026-05-24-luna-quality-pass-design.md`

**Test policy for this pass:** No new unit tests. Verification is `dotnet build` (compile success) plus a manual smoke checklist at the end of each phase. The existing test project must continue to build and pass.

---

## File Structure

**New files:**

- `Services/ExceptionMapper.cs` — static `ExceptionMapper` + `StateMessage` record + `Level` enum + `StateAction` record.
- `ViewModels/BannerItem.cs` — `BannerItem` observable record + `BannerAction` record + `BannerCollectionExtensions` (AddOrReplace / RemoveByKey).
- `ViewModels/InlineBannerState.cs` — `InlineBannerState` record with Info/Error factories.
- `ViewModels/ClampReport.cs` — collects clamp entries from `SettingsViewModel.Save()`.
- `ViewModels/OverlayState.cs` — enum `Idle | Processing | SuccessHold | Fading`.
- `Views/Controls/StateStrip.axaml` + `.axaml.cs` — bottom-of-window status row.
- `Views/Controls/BannerStack.axaml` + `.axaml.cs` — ItemsControl rendering BannerItems.
- `Views/Controls/InlineBanner.axaml` + `.axaml.cs` — dialog-scoped banner.
- `Views/Controls/LevelToBrushConverter.cs` — `Level → IBrush` value converter for control templates.

**Modified files:**

- `Models/AppSettings.cs` — add `CameraSupportDismissedUntil`; atomic-save via temp+rename (L2).
- `Models/CameraClip.cs` — update `ThumbnailIssueSummary` text per Wave 3.
- `Models/ReportSettings.cs` — token allow-list helper for `ReportNamePattern` (M3).
- `Services/ReportGenerationService.cs` — add `LastWrittenPaths` + `CleanupPartialOutputAsync` (H5).
- `Services/IReportGenerationService.cs` (if exists; otherwise interface lives in the same file) — same surface additions.
- `Services/Chappie/FfmpegThumbnailService.cs` — cancel token in decode loop (H3).
- `Services/Chappie/SonyRawViewerLocator.cs` — Windows guard on hard-coded path (M11).
- `ViewModels/MainWindowViewModel.cs` — replace `StatusText` with `State`; add `Banners`, `OverlayState`, `SuccessLabel`, `OnSettingsSaved`, `OpenOutputFolderCommand`, `OpenLastReportCommand`; debounce search (M4); ETA smoothing (M10).
- `ViewModels/MainWindowViewModel.Import.cs` — exception mapping (H1); partial file cleanup (H5); hero success state (M6); auto-fill collision (M5); cancel-during-tail guard (L1).
- `ViewModels/MainWindowViewModel.Update.cs` — convert toast properties to push BannerItems.
- `ViewModels/MainWindowViewModel.CameraSupport.cs` — dismiss persistence (M1); convert to BannerItem.
- `ViewModels/MainWindowViewModel.ClipActions.cs` — surface reveal failure via state strip.
- `ViewModels/SettingsViewModel.cs` — `SaveBanner`, `ClampReport`, `Clamp()` helper, `SaveCompleted` payload change; M3 pattern validation; detect-failure copy.
- `ViewModels/CreditsViewModel.cs` — improved log message for link open failure.
- `Views/MainWindow.axaml` — replace status bar with `StateStrip`, remove update + camera-support toast blocks, add `BannerStack` below header, add hero success layer, Wave 3 copy edits, Clear confirmation dialog wiring.
- `Views/MainWindow.axaml.cs` — Clear confirmation handler.
- `Views/SettingsWindow.axaml` — add `InlineBanner`, `(Optional)` labels, intro rewrite.
- `Views/SettingsWindow.axaml.cs` — pass ClampReport payload to MainWindow via existing `SaveCompleted` wiring.

**Renames / deletions:** none.

---

## Phase A — Foundation

## Task 1: ExceptionMapper + StateMessage + Level

**Files:**
- Create: `Services/ExceptionMapper.cs`

- [ ] **Step 1: Create the file with the types and mapper**

```csharp
using System;
using System.IO;
using CommunityToolkit.Mvvm.Input;

namespace LunaApp.Services;

/// <summary>Severity level shared by StateStrip, BannerStack, and InlineBanner.</summary>
public enum Level
{
    Info,
    Success,
    Warning,
    Error,
}

/// <summary>Optional inline action button on a state strip or banner.</summary>
public sealed record StateAction(string Label, IRelayCommand Command);

/// <summary>
/// Single payload bound by the StateStrip control. Replaces the old
/// <c>StatusText</c> string. The factory helpers cover every shape the
/// app needs without exposing the constructor.
/// </summary>
public sealed record StateMessage(
    Level Level,
    string Text,
    StateAction? Action = null,
    bool IsDismissible = false)
{
    public static StateMessage Info(string text, StateAction? action = null) =>
        new(Level.Info, text, action, IsDismissible: false);

    public static StateMessage Success(string text, StateAction? action = null) =>
        new(Level.Success, text, action, IsDismissible: false);

    public static StateMessage Warning(string text, StateAction? action = null) =>
        new(Level.Warning, text, action, IsDismissible: true);

    public static StateMessage Error(string text, StateAction? action = null) =>
        new(Level.Error, text, action, IsDismissible: true);

    /// <summary>Neutral idle state — used as the initial value on app start.</summary>
    public static StateMessage Idle(string text) => new(Level.Info, text);
}

/// <summary>
/// Maps caught exceptions to a friendly <see cref="StateMessage"/>. The raw
/// exception should always be logged at the call site before invoking this —
/// the user gets a clean message, the dev gets the full stack.
/// </summary>
public static class ExceptionMapper
{
    public static StateMessage ToUserMessage(Exception ex, string contextVerb) => ex switch
    {
        OperationCanceledException        => StateMessage.Info($"{contextVerb} cancelled"),
        UnauthorizedAccessException       => StateMessage.Error("Permission denied — check folder access"),
        DirectoryNotFoundException        => StateMessage.Error("Folder not found"),
        FileNotFoundException             => StateMessage.Error("File not found"),
        PathTooLongException              => StateMessage.Error("File path is too long"),
        IOException io when IsDiskFull(io)=> StateMessage.Error("Not enough disk space"),
        IOException                       => StateMessage.Error("File system error — the file may be open in another app"),
        _                                 => StateMessage.Error("Something went wrong"),
    };

    // ERROR_DISK_FULL = 0x70 on Windows; ENOSPC = 0x1C on Linux/macOS. Both
    // surface as the low 16 bits of HResult.
    private static bool IsDiskFull(IOException io) =>
        (io.HResult & 0xFFFF) is 0x70 or 0x1C;
}
```

- [ ] **Step 2: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds. The new file has no callers yet.

- [ ] **Step 3: Commit**

```bash
git add Services/ExceptionMapper.cs
git commit -m "feat: add ExceptionMapper + StateMessage record"
```

---

## Task 2: LevelToBrushConverter

**Files:**
- Create: `Views/Controls/LevelToBrushConverter.cs`

- [ ] **Step 1: Create the converter**

```csharp
using System;
using System.Globalization;
using Avalonia;
using Avalonia.Data.Converters;
using Avalonia.Media;
using LunaApp.Services;

namespace LunaApp.Views.Controls;

/// <summary>
/// Resolves a <see cref="Level"/> to one of the Luna theme brushes. Used by
/// StateStrip, BannerStack item template, and InlineBanner so a single point
/// owns the level→color mapping. Returns <c>LunaBgTertiary</c> for Info,
/// <c>LunaSuccess</c> for Success, <c>LunaWarning</c> for Warning,
/// <c>LunaDanger</c> for Error.
/// </summary>
public sealed class LevelToBrushConverter : IValueConverter
{
    public static readonly LevelToBrushConverter Instance = new();

    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        if (value is not Level level) return null;
        var key = level switch
        {
            Level.Info    => "LunaBgTertiary",
            Level.Success => "LunaSuccess",
            Level.Warning => "LunaWarning",
            Level.Error   => "LunaDanger",
            _             => "LunaBgTertiary",
        };
        return Application.Current?.TryFindResource(key, out var brush) == true
            ? brush
            : Brushes.Transparent;
    }

    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
```

- [ ] **Step 2: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 3: Commit**

```bash
git add Views/Controls/LevelToBrushConverter.cs
git commit -m "feat: add LevelToBrushConverter for theming the new surfaces"
```

---

## Task 3: StateStrip UserControl

**Files:**
- Create: `Views/Controls/StateStrip.axaml`
- Create: `Views/Controls/StateStrip.axaml.cs`

- [ ] **Step 1: Create the XAML**

```xml
<UserControl xmlns="https://github.com/avaloniaui"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:controls="using:LunaApp.Views.Controls"
             xmlns:mi="clr-namespace:Material.Icons.Avalonia;assembly=Material.Icons.Avalonia"
             xmlns:services="using:LunaApp.Services"
             x:Class="LunaApp.Views.Controls.StateStrip"
             x:Name="Root">

    <UserControl.Resources>
        <controls:LevelToBrushConverter x:Key="LevelToBrush"/>
    </UserControl.Resources>

    <Border BorderThickness="0,1,0,0"
            BorderBrush="{DynamicResource LunaBorder}"
            Background="{DynamicResource LunaBgSecondary}"
            Padding="16,8">
        <Grid ColumnDefinitions="Auto,*,Auto,Auto"
              IsVisible="{Binding Message, ElementName=Root, Converter={x:Static ObjectConverters.IsNotNull}}">

            <!-- Level dot -->
            <Ellipse Grid.Column="0"
                     Width="8" Height="8"
                     VerticalAlignment="Center"
                     Margin="0,0,12,0"
                     Fill="{Binding Message.Level, ElementName=Root, Converter={StaticResource LevelToBrush}}"/>

            <!-- Text -->
            <TextBlock Grid.Column="1"
                       Text="{Binding Message.Text, ElementName=Root}"
                       Foreground="{DynamicResource LunaTextPrimary}"
                       FontSize="{DynamicResource LunaFontSizeSm}"
                       VerticalAlignment="Center"
                       TextWrapping="NoWrap"
                       TextTrimming="CharacterEllipsis"/>

            <!-- Optional inline action -->
            <Button Grid.Column="2"
                    Classes="secondary"
                    Margin="12,0,0,0"
                    IsVisible="{Binding Message.Action, ElementName=Root, Converter={x:Static ObjectConverters.IsNotNull}}"
                    Command="{Binding Message.Action.Command, ElementName=Root}"
                    Content="{Binding Message.Action.Label, ElementName=Root}"/>

            <!-- Dismiss -->
            <Button Grid.Column="3"
                    Classes="iconOnly"
                    Margin="8,0,0,0"
                    Padding="6"
                    IsVisible="{Binding Message.IsDismissible, ElementName=Root}"
                    Click="OnDismissClick"
                    ToolTip.Tip="Dismiss">
                <mi:MaterialIcon Kind="Close" Width="14" Height="14"
                                 Foreground="{DynamicResource LunaTextSecondary}"/>
            </Button>
        </Grid>
    </Border>
</UserControl>
```

- [ ] **Step 2: Create the code-behind**

```csharp
using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Interactivity;
using LunaApp.Services;

namespace LunaApp.Views.Controls;

public partial class StateStrip : UserControl
{
    public static readonly StyledProperty<StateMessage?> MessageProperty =
        AvaloniaProperty.Register<StateStrip, StateMessage?>(nameof(Message));

    public StateMessage? Message
    {
        get => GetValue(MessageProperty);
        set => SetValue(MessageProperty, value);
    }

    public event EventHandler? DismissRequested;

    public StateStrip()
    {
        InitializeComponent();
    }

    private void OnDismissClick(object? sender, RoutedEventArgs e) =>
        DismissRequested?.Invoke(this, EventArgs.Empty);
}
```

- [ ] **Step 3: Add iconOnly button class to Controls.axaml if missing**

Read `Styles/Controls.axaml`. If there is no `Selector="Button.iconOnly"` style, append:

```xml
<Style Selector="Button.iconOnly">
    <Setter Property="Background" Value="Transparent"/>
    <Setter Property="BorderThickness" Value="0"/>
    <Setter Property="Padding" Value="6"/>
    <Setter Property="Cursor" Value="Hand"/>
</Style>
<Style Selector="Button.iconOnly:pointerover">
    <Setter Property="Background" Value="{DynamicResource LunaBgTertiary}"/>
</Style>
```

If a similar style exists under a different name, prefer that name in `StateStrip.axaml` instead of adding the new class.

- [ ] **Step 4: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds. Control is not wired into any window yet.

- [ ] **Step 5: Commit**

```bash
git add Views/Controls/StateStrip.axaml Views/Controls/StateStrip.axaml.cs Styles/Controls.axaml
git commit -m "feat: add StateStrip UserControl"
```

---

## Task 4: BannerItem + BannerStack

**Files:**
- Create: `ViewModels/BannerItem.cs`
- Create: `Views/Controls/BannerStack.axaml`
- Create: `Views/Controls/BannerStack.axaml.cs`

- [ ] **Step 1: Create BannerItem.cs**

```csharp
using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using LunaApp.Services;

namespace LunaApp.ViewModels;

/// <summary>Primary or secondary action attached to a banner.</summary>
public sealed record BannerAction(string Label, IRelayCommand Command);

/// <summary>
/// One row in the BannerStack. Observable so a VM partial can mutate
/// Body/PrimaryAction in place (e.g. update download progress) without
/// removing and re-adding the item.
/// </summary>
public sealed partial class BannerItem : ObservableObject
{
    public required string Key { get; init; }
    public required Level Level { get; init; }
    public required string Title { get; init; }

    [ObservableProperty] private string? _body;
    [ObservableProperty] private BannerAction? _primaryAction;
    [ObservableProperty] private BannerAction? _secondaryAction;

    public bool IsDismissible { get; init; }
    public IRelayCommand? OnDismiss { get; init; }
}

public static class BannerCollectionExtensions
{
    /// <summary>
    /// Add the item, or replace the existing item with the same Key. Used by
    /// VM partials whose state may change repeatedly (update progress, etc.).
    /// </summary>
    public static void AddOrReplace(this ObservableCollection<BannerItem> items, BannerItem item)
    {
        for (var i = 0; i < items.Count; i++)
        {
            if (items[i].Key == item.Key)
            {
                items[i] = item;
                return;
            }
        }
        items.Add(item);
    }

    public static void RemoveByKey(this ObservableCollection<BannerItem> items, string key)
    {
        for (var i = items.Count - 1; i >= 0; i--)
        {
            if (items[i].Key == key) items.RemoveAt(i);
        }
    }
}
```

- [ ] **Step 2: Create BannerStack.axaml**

```xml
<UserControl xmlns="https://github.com/avaloniaui"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:controls="using:LunaApp.Views.Controls"
             xmlns:vm="using:LunaApp.ViewModels"
             xmlns:mi="clr-namespace:Material.Icons.Avalonia;assembly=Material.Icons.Avalonia"
             x:Class="LunaApp.Views.Controls.BannerStack"
             x:Name="Root">

    <UserControl.Resources>
        <controls:LevelToBrushConverter x:Key="LevelToBrush"/>
    </UserControl.Resources>

    <ItemsControl ItemsSource="{Binding Items, ElementName=Root}">
        <ItemsControl.ItemsPanel>
            <ItemsPanelTemplate>
                <StackPanel Orientation="Vertical" Spacing="0"/>
            </ItemsPanelTemplate>
        </ItemsControl.ItemsPanel>
        <ItemsControl.ItemTemplate>
            <DataTemplate DataType="vm:BannerItem">
                <Border BorderBrush="{DynamicResource LunaBorder}"
                        BorderThickness="0,0,0,1"
                        Padding="24,10"
                        Background="{DynamicResource LunaBgSecondary}">
                    <Grid ColumnDefinitions="Auto,*,Auto,Auto,Auto">
                        <!-- Level dot -->
                        <Ellipse Grid.Column="0"
                                 Width="8" Height="8"
                                 VerticalAlignment="Center"
                                 Margin="0,0,12,0"
                                 Fill="{Binding Level, Converter={StaticResource LevelToBrush}}"/>

                        <!-- Title + body -->
                        <StackPanel Grid.Column="1" Orientation="Horizontal" Spacing="10" VerticalAlignment="Center">
                            <TextBlock Text="{Binding Title}"
                                       Foreground="{DynamicResource LunaTextPrimary}"
                                       FontSize="{DynamicResource LunaFontSizeSm}"
                                       FontWeight="SemiBold"
                                       VerticalAlignment="Center"/>
                            <TextBlock Text="{Binding Body}"
                                       Foreground="{DynamicResource LunaTextSecondary}"
                                       FontSize="{DynamicResource LunaFontSizeSm}"
                                       VerticalAlignment="Center"
                                       IsVisible="{Binding Body, Converter={x:Static ObjectConverters.IsNotNull}}"/>
                        </StackPanel>

                        <!-- Secondary action -->
                        <Button Grid.Column="2"
                                Classes="secondary"
                                Margin="0,0,8,0"
                                IsVisible="{Binding SecondaryAction, Converter={x:Static ObjectConverters.IsNotNull}}"
                                Command="{Binding SecondaryAction.Command}"
                                Content="{Binding SecondaryAction.Label}"/>

                        <!-- Primary action -->
                        <Button Grid.Column="3"
                                Classes="primary"
                                IsVisible="{Binding PrimaryAction, Converter={x:Static ObjectConverters.IsNotNull}}"
                                Command="{Binding PrimaryAction.Command}"
                                Content="{Binding PrimaryAction.Label}"/>

                        <!-- Dismiss -->
                        <Button Grid.Column="4"
                                Classes="iconOnly"
                                Margin="8,0,0,0"
                                IsVisible="{Binding IsDismissible}"
                                Command="{Binding OnDismiss}"
                                ToolTip.Tip="Dismiss">
                            <mi:MaterialIcon Kind="Close" Width="14" Height="14"
                                             Foreground="{DynamicResource LunaTextSecondary}"/>
                        </Button>
                    </Grid>
                </Border>
            </DataTemplate>
        </ItemsControl.ItemTemplate>
    </ItemsControl>
</UserControl>
```

- [ ] **Step 3: Create BannerStack.axaml.cs**

```csharp
using System.Collections.ObjectModel;
using Avalonia;
using Avalonia.Controls;
using LunaApp.ViewModels;

namespace LunaApp.Views.Controls;

public partial class BannerStack : UserControl
{
    public static readonly StyledProperty<ObservableCollection<BannerItem>?> ItemsProperty =
        AvaloniaProperty.Register<BannerStack, ObservableCollection<BannerItem>?>(nameof(Items));

    public ObservableCollection<BannerItem>? Items
    {
        get => GetValue(ItemsProperty);
        set => SetValue(ItemsProperty, value);
    }

    public BannerStack()
    {
        InitializeComponent();
    }
}
```

- [ ] **Step 4: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds. BannerStack is not yet wired into any window.

- [ ] **Step 5: Commit**

```bash
git add ViewModels/BannerItem.cs Views/Controls/BannerStack.axaml Views/Controls/BannerStack.axaml.cs
git commit -m "feat: add BannerItem + BannerStack UserControl"
```

---

## Task 5: InlineBanner control

**Files:**
- Create: `ViewModels/InlineBannerState.cs`
- Create: `Views/Controls/InlineBanner.axaml`
- Create: `Views/Controls/InlineBanner.axaml.cs`

- [ ] **Step 1: Create InlineBannerState.cs**

```csharp
using CommunityToolkit.Mvvm.Input;
using LunaApp.Services;

namespace LunaApp.ViewModels;

/// <summary>
/// State for the InlineBanner control inside SettingsWindow. Carries an
/// optional retry command for the Error case.
/// </summary>
public sealed record InlineBannerState(
    Level Level,
    string Text,
    IRelayCommand? RetryCommand = null)
{
    public static InlineBannerState Info(string text) => new(Level.Info, text);
    public static InlineBannerState Error(string text, IRelayCommand? retryCommand = null) =>
        new(Level.Error, text, retryCommand);
}
```

- [ ] **Step 2: Create InlineBanner.axaml**

```xml
<UserControl xmlns="https://github.com/avaloniaui"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:controls="using:LunaApp.Views.Controls"
             xmlns:mi="clr-namespace:Material.Icons.Avalonia;assembly=Material.Icons.Avalonia"
             x:Class="LunaApp.Views.Controls.InlineBanner"
             x:Name="Root">

    <UserControl.Resources>
        <controls:LevelToBrushConverter x:Key="LevelToBrush"/>
    </UserControl.Resources>

    <Border Padding="14,10"
            CornerRadius="{DynamicResource LunaRadiusMd}"
            Background="{DynamicResource LunaBgTertiary}"
            BorderThickness="{DynamicResource LunaBorderThin}"
            BorderBrush="{Binding State.Level, ElementName=Root, Converter={StaticResource LevelToBrush}}"
            IsVisible="{Binding State, ElementName=Root, Converter={x:Static ObjectConverters.IsNotNull}}">
        <Grid ColumnDefinitions="Auto,*,Auto">
            <Ellipse Grid.Column="0"
                     Width="8" Height="8"
                     VerticalAlignment="Center"
                     Margin="0,0,12,0"
                     Fill="{Binding State.Level, ElementName=Root, Converter={StaticResource LevelToBrush}}"/>
            <TextBlock Grid.Column="1"
                       Text="{Binding State.Text, ElementName=Root}"
                       Foreground="{DynamicResource LunaTextPrimary}"
                       FontSize="{DynamicResource LunaFontSizeSm}"
                       VerticalAlignment="Center"
                       TextWrapping="Wrap"/>
            <Button Grid.Column="2"
                    Classes="secondary"
                    Margin="12,0,0,0"
                    Content="Retry"
                    Command="{Binding State.RetryCommand, ElementName=Root}"
                    IsVisible="{Binding State.RetryCommand, ElementName=Root, Converter={x:Static ObjectConverters.IsNotNull}}"/>
        </Grid>
    </Border>
</UserControl>
```

- [ ] **Step 3: Create InlineBanner.axaml.cs**

```csharp
using Avalonia;
using Avalonia.Controls;
using LunaApp.ViewModels;

namespace LunaApp.Views.Controls;

public partial class InlineBanner : UserControl
{
    public static readonly StyledProperty<InlineBannerState?> StateProperty =
        AvaloniaProperty.Register<InlineBanner, InlineBannerState?>(nameof(State));

    public InlineBannerState? State
    {
        get => GetValue(StateProperty);
        set => SetValue(StateProperty, value);
    }

    public InlineBanner()
    {
        InitializeComponent();
    }
}
```

- [ ] **Step 4: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 5: Commit**

```bash
git add ViewModels/InlineBannerState.cs Views/Controls/InlineBanner.axaml Views/Controls/InlineBanner.axaml.cs
git commit -m "feat: add InlineBanner for SettingsWindow"
```

---

## Task 6: OverlayState enum + ClampReport

**Files:**
- Create: `ViewModels/OverlayState.cs`
- Create: `ViewModels/ClampReport.cs`

- [ ] **Step 1: Create OverlayState.cs**

```csharp
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
```

- [ ] **Step 2: Create ClampReport.cs**

```csharp
using System.Collections.Generic;
using System.Linq;

namespace LunaApp.ViewModels;

/// <summary>
/// Collects entries when <see cref="SettingsViewModel.Save"/> clamps fields.
/// Hand-off shape from Settings to MainWindow so the state strip can surface
/// "what got changed" after the dialog closes.
/// </summary>
public sealed class ClampReport
{
    private readonly List<string> _labels = [];

    public void Add(string label, int originalLength, int clampedTo) => _labels.Add(label);

    public bool HasAny => _labels.Count > 0;

    public string Describe()
    {
        if (_labels.Count == 0) return string.Empty;
        if (_labels.Count == 1) return $"{_labels[0]} trimmed to fit";
        if (_labels.Count == 2) return $"{_labels[0]} and {_labels[1]} trimmed to fit";

        var head = string.Join(", ", _labels.Take(_labels.Count - 1));
        return $"{head}, and {_labels.Last()} trimmed to fit";
    }
}
```

- [ ] **Step 3: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 4: Commit**

```bash
git add ViewModels/OverlayState.cs ViewModels/ClampReport.cs
git commit -m "feat: add OverlayState enum and ClampReport"
```

---

## Phase A checkpoint

Manual smoke check — none required. Phase A introduced types only; no observable behaviour change.

`dotnet build` should be clean. Existing tests should still pass:

```bash
dotnet test LunaApp.Tests/LunaApp.Tests.csproj
```

---

## Phase B — Wire foundation + behaviour fixes

## Task 7: Replace `StatusText` with `State` in MainWindowViewModel

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs`
- Modify: `ViewModels/MainWindowViewModel.Import.cs`
- Modify: `ViewModels/MainWindowViewModel.Update.cs` (only the call sites that set StatusText, if any)
- Modify: `ViewModels/MainWindowViewModel.CameraSupport.cs` (only the call sites that set StatusText, if any)
- Modify: `Views/MainWindow.axaml`

This is the biggest single edit. Done in one task because every `StatusText` reference must move atomically.

- [ ] **Step 1: Replace the property in MainWindowViewModel.cs**

In `ViewModels/MainWindowViewModel.cs`, locate `[ObservableProperty] private string _statusText = "Ready - Drop camera footage to begin";` (around line 26). Replace with:

```csharp
[ObservableProperty]
private StateMessage _state = StateMessage.Idle("Ready — drop camera footage to begin");
```

Add `using LunaApp.Services;` at top of file if not already imported.

- [ ] **Step 2: Update Import.cs status assignments**

In `ViewModels/MainWindowViewModel.Import.cs`, replace every `StatusText = "..."` and `StatusText = $"..."` with `State = StateMessage.Info(...)` or appropriate factory. Specifically:

```csharp
// Line 63
State = StateMessage.Info($"Counting files in {Path.GetFileName(folderPath)}…");

// Line 68
State = StateMessage.Warning("No video files found in this folder");

// Line 95
State = StateMessage.Info($"Found {count} video clip(s) ready to scan");

// Line 99
State = StateMessage.Info("Scan cancelled");

// Line 104 (covered fully in Task 8; for now leave as raw exception)
State = StateMessage.Error($"Error: {ex.Message}");

// Line 134
State = StateMessage.Info($"Processing {PendingFolderName}…");

// Line 147
State = StateMessage.Info($"Found {ReelCount} reel(s) with {TotalClipCount} clips");

// Line 151
State = StateMessage.Info("Processing cancelled");

// Line 159 (covered fully in Task 8)
State = StateMessage.Error($"Error: {ex.Message}");

// Line 174
State = StateMessage.Idle("Ready — drop camera footage to begin");

// Line 187
State = StateMessage.Info("Cancelling…");

// Line 285 (will be overwritten by Task 11 for the success-action variant; for now leave as info)
State = StateMessage.Success($"Reports saved to {settings.OutputFolder}");

// Line 294
State = StateMessage.Info("Generation cancelled");

// Line 299
State = StateMessage.Error($"Error: {ex.Message}");

// Line 326
State = StateMessage.Idle("Ready — drop camera footage to begin");
```

Add `using LunaApp.Services;` at top of Import.cs.

- [ ] **Step 3: Search for any other StatusText references**

```bash
grep -rn "StatusText" ViewModels/
```

If any references remain in Update.cs, CameraSupport.cs, ClipActions.cs, or LogViewer.cs, replace each with the equivalent `State = StateMessage.*(...)` call. Use `Info` for neutral status, `Error` for failures.

- [ ] **Step 4: Update MainWindow.axaml status bar**

In `Views/MainWindow.axaml`, find the status bar block (Grid.Row=4, around line 635-645) showing `{Binding StatusText}`. Replace the entire status bar Border with:

```xml
<controls:StateStrip Grid.Row="4"
                     Message="{Binding State}"/>
```

If the existing status bar also had a mini progress bar, retain it inside or beside the StateStrip — but a simpler initial wiring is fine. The progress bar lives separately under the overlay.

- [ ] **Step 5: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds. If there are remaining `StatusText` references, the build will fail with `CS0103 The name 'StatusText' does not exist` — fix each and rebuild.

- [ ] **Step 6: Manual smoke test**

```bash
dotnet run --project LunaApp.csproj
```

Drop a folder of clips. Verify: status strip at the bottom shows the count, then processing messages, then completion. No crashes.

- [ ] **Step 7: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs ViewModels/MainWindowViewModel.Import.cs ViewModels/MainWindowViewModel.Update.cs ViewModels/MainWindowViewModel.CameraSupport.cs ViewModels/MainWindowViewModel.ClipActions.cs ViewModels/MainWindowViewModel.LogViewer.cs Views/MainWindow.axaml
git commit -m "refactor: replace StatusText with StateMessage and StateStrip control"
```

---

## Task 8: H1 — exception mapping at call sites

**Files:**
- Modify: `ViewModels/MainWindowViewModel.Import.cs`
- Modify: `ViewModels/MainWindowViewModel.ClipActions.cs`
- Modify: `ViewModels/CreditsViewModel.cs`

- [ ] **Step 1: Update Import.cs catch blocks**

In `ViewModels/MainWindowViewModel.Import.cs`, replace the three `catch (Exception ex)` blocks:

```csharp
// QuickScanFolderAsync (around line 101-105)
catch (Exception ex)
{
    Log.Error(ex, "Failed to scan folder: {Path}", folderPath);
    State = ExceptionMapper.ToUserMessage(ex, "Scan");
}

// StartProcessingAsync (around line 156-160)
catch (Exception ex)
{
    Log.Error(ex, "Failed to process folder: {Path}", folderPath);
    State = ExceptionMapper.ToUserMessage(ex, "Processing");
}

// GenerateReportsAsync (around line 296-300)
catch (Exception ex)
{
    Log.Error(ex, "Report generation failed");
    State = ExceptionMapper.ToUserMessage(ex, "Generation");
}
```

- [ ] **Step 2: Surface report-open failure**

In `ViewModels/MainWindowViewModel.Import.cs`, update `EndOperationAsync` around line 231-234:

```csharp
catch (Exception ex)
{
    Log.Warning(ex, "Failed to open report file: {Path}", path);
    State = StateMessage.Warning(
        $"Report saved, but couldn't open it. Find it at: {Path.GetDirectoryName(path)}",
        new StateAction("Open folder", OpenOutputFolderCommand));
}
```

(`OpenOutputFolderCommand` is added in Task 11. For this task, just write the call — it'll compile if the command exists; otherwise wait until Task 11 and verify build there. To keep this task self-contained, use a temporary fallback without the action: `State = StateMessage.Warning($"Report saved, but couldn't open it. Find it at: {Path.GetDirectoryName(path)}");` — drop the action argument until Task 11.)

- [ ] **Step 3: Update ClipActions.cs reveal failure**

In `ViewModels/MainWindowViewModel.ClipActions.cs`, find the `RevealClipInExplorer` catch block (around line 56-60):

```csharp
catch (Exception ex)
{
    Log.Warning(ex, "Failed to reveal {File} in file manager", clip.FilePath);
    State = StateMessage.Warning("Couldn't reveal that clip in the file manager.");
}
```

Add `using LunaApp.Services;` at top of file if not already present.

- [ ] **Step 4: Improve CreditsViewModel.cs link-open log**

In `ViewModels/CreditsViewModel.cs`, find the `OpenLink` catch block (around line 89-92). Credits is a modal window and has no state strip, so just enhance the log so the user can recover manually:

```csharp
catch (Exception ex)
{
    Log.Warning(ex, "Failed to open link {Url} — copy manually from this log entry", url);
}
```

- [ ] **Step 5: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 6: Manual smoke test**

Run the app. Try dropping a folder that doesn't exist (rename a folder mid-drop). Verify the state strip shows "Folder not found" not the raw exception text.

- [ ] **Step 7: Commit**

```bash
git add ViewModels/MainWindowViewModel.Import.cs ViewModels/MainWindowViewModel.ClipActions.cs ViewModels/CreditsViewModel.cs
git commit -m "feat(H1): map exceptions to friendly StateMessages at user-facing call sites"
```

---

## Task 9: H5 — partial file cleanup on cancel

**Files:**
- Modify: `Services/ReportGenerationService.cs` (interface + class)
- Modify: `ViewModels/MainWindowViewModel.Import.cs`

- [ ] **Step 1: Read the existing ReportGenerationService to find the interface**

```bash
```

Read `Services/ReportGenerationService.cs`. Locate the interface declaration (or `IReportGenerationService` in a sibling file). Note the existing method signatures.

- [ ] **Step 2: Add `LastWrittenPaths` property and `CleanupPartialOutputAsync` to the interface**

In the interface (`IReportGenerationService` if separate, otherwise the public abstract surface):

```csharp
/// <summary>
/// Files written by the most recent GenerateReportsAsync call. Populated
/// incrementally as the service writes; reset at the start of each call.
/// Read by callers in OperationCanceledException paths so they can clean up.
/// </summary>
IReadOnlyList<string> LastWrittenPaths { get; }

/// <summary>
/// Best-effort deletion of files in <paramref name="partialPaths"/> plus any
/// sibling <c>*.tmp</c> files in their parent directories. Safe when
/// <paramref name="partialPaths"/> is empty. Failures are logged, never thrown.
/// </summary>
Task CleanupPartialOutputAsync(ReportSettings settings, IReadOnlyList<string> partialPaths);
```

- [ ] **Step 3: Implement on the class**

In `Services/ReportGenerationService.cs`:

```csharp
private readonly List<string> _lastWrittenPaths = new();
public IReadOnlyList<string> LastWrittenPaths => _lastWrittenPaths;

public Task CleanupPartialOutputAsync(ReportSettings settings, IReadOnlyList<string> partialPaths)
{
    foreach (var path in partialPaths)
    {
        TryDelete(path);

        // Sibling temp files in the same folder (e.g. PDF writer's tmp).
        var dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir) && Directory.Exists(dir))
        {
            try
            {
                foreach (var tmp in Directory.EnumerateFiles(dir, "*.tmp"))
                    TryDelete(tmp);
            }
            catch (Exception ex)
            {
                Log.Debug(ex, "Cleanup: failed to enumerate temp files in {Dir}", dir);
            }
        }
    }
    return Task.CompletedTask;

    static void TryDelete(string path)
    {
        try { if (File.Exists(path)) File.Delete(path); }
        catch (Exception ex) { Log.Debug(ex, "Cleanup: failed to delete {Path}", path); }
    }
}
```

- [ ] **Step 4: Track writes inside GenerateReportsAsync**

Inside `GenerateReportsAsync` (existing method on the same class), at the very top:

```csharp
_lastWrittenPaths.Clear();
```

Then everywhere the service writes a file (HTML report, PDF report, per-reel files) and currently appends the path to its return list, also call:

```csharp
_lastWrittenPaths.Add(writtenPath);
```

If the service already builds `outputPaths` incrementally, you can simply substitute `_lastWrittenPaths` for the existing local list and return `_lastWrittenPaths` (cast/copy as IReadOnlyList).

- [ ] **Step 5: Wire cleanup into the cancellation handler**

In `ViewModels/MainWindowViewModel.Import.cs`, update the `GenerateReportsAsync` cancellation catch (around line 292-295):

```csharp
catch (OperationCanceledException)
{
    if (_reportService.LastWrittenPaths is { Count: > 0 } partials)
    {
        try
        {
            await _reportService.CleanupPartialOutputAsync(settings, partials);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Cleanup of partial reports failed");
        }
    }
    State = StateMessage.Info("Generation cancelled");
}
```

- [ ] **Step 6: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 7: Manual smoke test**

Run the app. Drop a folder with several clips. Start generation, then click Cancel mid-generation. Verify no half-written `.html` / `.pdf` / `.tmp` files remain in the output folder.

- [ ] **Step 8: Commit**

```bash
git add Services/ReportGenerationService.cs ViewModels/MainWindowViewModel.Import.cs
git commit -m "feat(H5): clean up partial report output when generation is cancelled"
```

---

## Task 10: M1 — persist camera-support dismissal

**Files:**
- Modify: `Models/AppSettings.cs`
- Modify: `ViewModels/MainWindowViewModel.CameraSupport.cs`

- [ ] **Step 1: Add the persisted property**

In `Models/AppSettings.cs`, after the `CameraSupportSnoozeUntil` property (around line 46):

```csharp
/// <summary>
/// When non-null and in the future, the camera-support banner is hidden
/// permanently for this session and across restarts. Set when the user
/// chooses "Dismiss" on the banner. Cleared automatically when the
/// previously missing tools become installed, so the user isn't hidden
/// from re-discovering changed state. Mirrors the shape of
/// <see cref="CameraSupportSnoozeUntil"/>.
/// </summary>
public DateTime? CameraSupportDismissedUntil { get; set; }
```

The source-generated JSON context (`Models/AppSettingsJsonContext.cs`) reflects the whole `AppSettings` type — it picks up the new property automatically.

- [ ] **Step 2: Update CameraSupportToastState.Dismiss to persist**

In `ViewModels/MainWindowViewModel.CameraSupport.cs`, replace the `Dismiss()` method (line 138-143) with:

```csharp
public void Dismiss()
{
    _settings.CameraSupportDismissedUntil = DateTime.MaxValue;
    _save(_settings);
    HasMissingCameraSupport = false;
    MissingCameraSupportSummary = string.Empty;
}
```

- [ ] **Step 3: Widen the IsSnoozed check**

Replace lines 103-104:

```csharp
private bool IsSnoozed =>
    (_settings.CameraSupportSnoozeUntil is DateTime snooze && snooze > DateTime.Now) ||
    (_settings.CameraSupportDismissedUntil is DateTime dismiss && dismiss > DateTime.Now);
```

- [ ] **Step 4: Clear dismissal when state changes for the better**

Modify `OnStatusChanged()` (line 128) to clear the dismissal flag if the previously missing tools become installed. Replace:

```csharp
public void OnStatusChanged() => Refresh();
```

with:

```csharp
public void OnStatusChanged()
{
    // If the user dismissed because of a specific missing set, and that set
    // is now installed, clear the dismissal so they're not permanently
    // hidden from future changes (e.g. a different vendor goes missing
    // because the user uninstalled it).
    if (_settings.CameraSupportDismissedUntil is not null)
    {
        var missing = _status.ResolveMissing();
        if (missing.Count == 0)
        {
            _settings.CameraSupportDismissedUntil = null;
            _save(_settings);
            _dismissedThisSession = false;
        }
    }

    Refresh();
}
```

- [ ] **Step 5: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 6: Manual smoke test**

Run the app on a machine where ARRI/Sony tools are missing. Dismiss the camera-support banner. Close and reopen the app. Verify the banner does not reappear. (The banner conversion happens in Task 14 — for now the existing toast is what you'll see, but its hidden state persists.)

- [ ] **Step 7: Commit**

```bash
git add Models/AppSettings.cs ViewModels/MainWindowViewModel.CameraSupport.cs
git commit -m "feat(M1): persist camera-support dismissal across sessions"
```

---

## Task 11: M6 — Hero success state + Open folder / Open report commands

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs`
- Modify: `ViewModels/MainWindowViewModel.Import.cs`
- Modify: `Views/MainWindow.axaml`

- [ ] **Step 1: Add OverlayState + SuccessLabel + completed paths to the VM**

In `ViewModels/MainWindowViewModel.cs`, add private fields and properties:

```csharp
private OverlayState _overlayState = OverlayState.Idle;
public OverlayState OverlayState
{
    get => _overlayState;
    private set => SetProperty(ref _overlayState, value);
}

[ObservableProperty] private string? _successLabel;

private IReadOnlyList<string>? _completedReportPaths;
private bool _userClosedOverlay;

private static readonly TimeSpan SuccessHoldDelay = TimeSpan.FromMilliseconds(2000);
```

If the class doesn't inherit from `ObservableObject` directly but uses `ViewModelBase` or similar, use whichever `SetProperty` is already in use elsewhere in the file.

- [ ] **Step 2: Add commands**

In `ViewModels/MainWindowViewModel.cs`:

```csharp
[RelayCommand]
private void OpenOutputFolder()
{
    if (_completedReportPaths is { Count: > 0 } paths)
    {
        var folder = Path.GetDirectoryName(paths[0]);
        if (!string.IsNullOrEmpty(folder) && Directory.Exists(folder))
        {
            try
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = folder,
                    UseShellExecute = true,
                });
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Failed to open output folder {Folder}", folder);
                State = StateMessage.Warning("Couldn't open the output folder.");
            }
        }
    }
    _userClosedOverlay = true;
}

[RelayCommand]
private void OpenLastReport()
{
    if (_completedReportPaths is { Count: > 0 } paths)
    {
        var report = paths.FirstOrDefault(p => p.EndsWith(".html")) ?? paths[0];
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = report,
                UseShellExecute = true,
            });
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to open report file {Path}", report);
            State = StateMessage.Warning("Couldn't open the report.");
        }
    }
    _userClosedOverlay = true;
}
```

Add required `using` statements: `System.Diagnostics`, `System.IO`, `System.Linq`, `Serilog`, `LunaApp.Services`.

- [ ] **Step 3: Update GenerateReportsAsync happy path**

In `ViewModels/MainWindowViewModel.Import.cs`, replace the happy-path block (around line 284-290):

```csharp
var outputPaths = await _reportService.GenerateReportsAsync(settings, cts.Token);
_completedReportPaths = outputPaths;
_overlayState = OverlayState.SuccessHold;
SuccessLabel = "Report saved";
_userClosedOverlay = false;

State = StateMessage.Success(
    $"Reports saved to {settings.OutputFolder}",
    new StateAction("Open folder", OpenOutputFolderCommand));

if (OpenWhenDone && outputPaths.Count > 0)
{
    _pendingOpenAfterTail = outputPaths.FirstOrDefault(p => p.EndsWith(".html")) ?? outputPaths[0];
}
```

Notify the public `OverlayState` property:

```csharp
OnPropertyChanged(nameof(OverlayState));
```

after assigning `_overlayState` (or use `OverlayState = ...` if you implemented the setter publicly).

- [ ] **Step 4: Make EndOperationAsync phase-aware**

Replace the existing `EndOperationAsync` (line 208-236) with:

```csharp
private async Task EndOperationAsync()
{
    OverallProgress = 100;
    Progress = 100;

    if (_overlayState == OverlayState.SuccessHold)
    {
        var holdEnd = DateTime.UtcNow + SuccessHoldDelay;
        while (DateTime.UtcNow < holdEnd && !_userClosedOverlay)
        {
            await Task.Delay(50).ConfigureAwait(true);
        }
    }
    else
    {
        await Task.Delay(EndTailDelay).ConfigureAwait(true);
    }

    _currentOperationCts = null;
    IsProcessing = false;
    OverallProgress = 0;
    _overlayState = OverlayState.Idle;
    SuccessLabel = null;
    OnPropertyChanged(nameof(OverlayState));
    OnPropertyChanged(nameof(CanCancel));

    if (_pendingOpenAfterTail is { } path)
    {
        _pendingOpenAfterTail = null;
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = path,
                UseShellExecute = true,
            });
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to open report file: {Path}", path);
            State = StateMessage.Warning(
                $"Report saved, but couldn't open it. Find it at: {Path.GetDirectoryName(path)}",
                new StateAction("Open folder", OpenOutputFolderCommand));
        }
    }
}
```

- [ ] **Step 5: Extend processing overlay XAML**

In `Views/MainWindow.axaml`, locate the processing overlay block (around line 500-565, the panel with `Classes="overlay"` that shows the moon and phase labels). Add a sibling layer or a state-conditional region for the success hold.

Inside the same overlay panel, after the existing `PhaseLabel`/`PhaseDetail` block, add:

```xml
<!-- Success hold layer — visible during OverlayState.SuccessHold -->
<StackPanel Spacing="16"
            HorizontalAlignment="Center"
            VerticalAlignment="Center"
            IsVisible="{Binding OverlayState, Converter={x:Static ObjectConverters.Equal}, ConverterParameter={x:Static vm:OverlayState.SuccessHold}}">
    <TextBlock Text="{Binding SuccessLabel}"
               FontSize="{DynamicResource LunaFontSizeXl}"
               FontWeight="SemiBold"
               Foreground="{DynamicResource LunaTextPrimary}"
               HorizontalAlignment="Center"/>
    <StackPanel Orientation="Horizontal" Spacing="12" HorizontalAlignment="Center">
        <Button Classes="primary"
                Content="Open folder"
                Command="{Binding OpenOutputFolderCommand}"/>
        <Button Classes="secondary"
                Content="Open report"
                Command="{Binding OpenLastReportCommand}"/>
    </StackPanel>
</StackPanel>
```

Avalonia doesn't ship `ObjectConverters.Equal` out of the box. Use the `EnumToBoolConverter` pattern via a simple converter declared inline or in `Styles/Controls.axaml`. Minimal local converter:

Add to `Views/MainWindow.axaml.cs`:

```csharp
public sealed class EqualsConverter : Avalonia.Data.Converters.IValueConverter
{
    public static readonly EqualsConverter Instance = new();
    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
        => value?.Equals(parameter) == true;
    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
```

And reference in XAML resources at the top of MainWindow.axaml:

```xml
<Window.Resources>
    <local:EqualsConverter x:Key="EqualsConverter"/>
</Window.Resources>
```

with `xmlns:local="using:LunaApp.Views"`. Then bind: `IsVisible="{Binding OverlayState, Converter={StaticResource EqualsConverter}, ConverterParameter={x:Static vm:OverlayState.SuccessHold}}"`. The same idea applies to gating the existing processing block on `OverlayState.Processing` (visible while processing AND IsProcessing — for the initial implementation, you can leave the existing `IsProcessing` gating in place and only add the new SuccessHold layer using the converter).

Also hide the existing PhaseLabel/PhaseDetail block during SuccessHold by gating its visibility:

```xml
IsVisible="{Binding OverlayState, Converter={StaticResource EqualsConverter}, ConverterParameter={x:Static vm:OverlayState.Processing}}"
```

- [ ] **Step 6: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 7: Manual smoke test**

Run the app. Drop a folder with 2-3 clips, scan, then generate. Verify:
1. Processing overlay shows the phase / detail / progress as before.
2. After generation completes, the overlay holds for ~2 s with "Report saved" + Open folder / Open report buttons.
3. Clicking Open folder opens the output folder and closes the overlay.
4. State strip shows "Reports saved to …" with the inline Open folder action.

- [ ] **Step 8: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs ViewModels/MainWindowViewModel.Import.cs Views/MainWindow.axaml Views/MainWindow.axaml.cs
git commit -m "feat(M6): hero success-hold state with Open folder / Open report actions"
```

---

## Task 12: H2 + M2 — Settings save banner + ClampReport hand-off

**Files:**
- Modify: `ViewModels/SettingsViewModel.cs`
- Modify: `ViewModels/MainWindowViewModel.cs`
- Modify: `Views/SettingsWindow.axaml`
- Modify: `Views/SettingsWindow.axaml.cs`

- [ ] **Step 1: Add SaveBanner + Clamp helper to SettingsViewModel**

In `ViewModels/SettingsViewModel.cs`, near the top of the class with other observable properties:

```csharp
[ObservableProperty] private InlineBannerState? _saveBanner;
```

Add a private helper method:

```csharp
private static string? Clamp(string? value, int max, string label, ClampReport report)
{
    if (string.IsNullOrWhiteSpace(value)) return null;
    var trimmed = value.Trim();
    if (trimmed.Length <= max) return trimmed;
    report.Add(label, trimmed.Length, max);
    return trimmed[..max];
}
```

- [ ] **Step 2: Change SaveCompleted signature**

Find the event declaration (likely `public event Action? SaveCompleted;`). Change to:

```csharp
public event Action<ClampReport?>? SaveCompleted;
```

- [ ] **Step 3: Update Save() to clamp text fields and surface failure**

Replace `Save()` (line 444-498) with:

```csharp
[RelayCommand]
private void Save()
{
    var settings = _appSettings.DefaultReportSettings;
    var clampReport = new ClampReport();

    settings.ProjectName       = Clamp(ProjectName, 120, "Project name", clampReport);
    settings.ProductionCompany = Clamp(ProductionCompany, 120, "Production company", clampReport);
    settings.DitName           = Clamp(DitName, 80, "DIT", clampReport);
    settings.Director          = Clamp(Director, 80, "Director", clampReport);
    settings.Dp                = Clamp(Dp, 80, "DP", clampReport);
    settings.LogoPath          = LogoPath;

    settings.OutputFolder = string.IsNullOrWhiteSpace(OutputFolder)
        ? _appSettings.DefaultReportSettings.OutputFolder
        : OutputFolder.Trim();

    settings.ThumbnailsPerClip = Math.Clamp(ThumbnailsPerClip, 0, 10);
    settings.GenerateHtml = GenerateHtmlByDefault;
    settings.GeneratePdf = GeneratePdfByDefault;
    settings.OpenReportWhenDone = OpenReportWhenDone;
    settings.GroupPdfsInSeparateFolder = GroupPdfsInSeparateFolder;
    settings.Theme = IsDarkTheme ? ReportTheme.Dark : ReportTheme.Light;

    if (!string.IsNullOrEmpty(LogoPath) && File.Exists(LogoPath))
    {
        try
        {
            var logoBytes = File.ReadAllBytes(LogoPath);
            settings.LogoBase64 = Convert.ToBase64String(logoBytes);
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Could not embed logo from {Path} — report will reference path instead", LogoPath);
        }
    }
    else if (string.IsNullOrEmpty(LogoPath))
    {
        settings.LogoBase64 = null;
    }

    if (_appSettings.Save())
    {
        Log.Information("Settings saved (project={Project}, output={Output}, theme={Theme})",
            settings.ProjectName, settings.OutputFolder, settings.Theme);
        SaveBanner = null;
        SaveCompleted?.Invoke(clampReport.HasAny ? clampReport : null);
    }
    else
    {
        Log.Warning("Settings write failed — surfacing inline banner");
        SaveBanner = InlineBannerState.Error(
            "Couldn't save settings. Check folder permissions or try again.",
            SaveCommand);
    }
}
```

- [ ] **Step 4: Add OnSettingsSaved on MainWindowViewModel**

In `ViewModels/MainWindowViewModel.cs`, add:

```csharp
public void OnSettingsSaved(ClampReport? clampReport)
{
    if (clampReport?.HasAny == true)
    {
        State = StateMessage.Info(clampReport.Describe());
    }
}
```

- [ ] **Step 5: Wire the SettingsWindow.axaml.cs subscriber**

Open `Views/SettingsWindow.axaml.cs`. Find where it subscribes to `SaveCompleted`. The handler likely closes the window. Change the subscriber to forward the ClampReport to MainWindow:

```csharp
private void OnSaveCompleted(ClampReport? clampReport)
{
    // Forward to the main window's VM if available.
    if (Owner is MainWindow mw && mw.DataContext is MainWindowViewModel mwvm)
    {
        mwvm.OnSettingsSaved(clampReport);
    }
    Close();
}
```

Subscribe `SettingsViewModel.SaveCompleted += OnSaveCompleted;` (matching whichever lifecycle hook is currently used).

- [ ] **Step 6: Add InlineBanner to SettingsWindow.axaml**

Open `Views/SettingsWindow.axaml`. Find the ScrollViewer content (Grid.Row=1 inside the dialog). At the very top of the StackPanel inside the ScrollViewer, add:

```xml
<controls:InlineBanner State="{Binding SaveBanner}"
                       Margin="0,0,0,16"
                       IsVisible="{Binding SaveBanner, Converter={x:Static ObjectConverters.IsNotNull}}"/>
```

Ensure `xmlns:controls="using:LunaApp.Views.Controls"` is declared at the top of the file.

- [ ] **Step 7: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 8: Manual smoke test**

Run the app. Open Settings. Enter a 200-char Project Name. Click Save. Verify dialog closes and the main window state strip shows *"Project name trimmed to fit"*. To test the error path, set `Models/AppSettings.cs.SettingsPath` to a read-only path temporarily (or test by setting an output folder you don't have permission for and inducing a real save failure) — the dialog should stay open with the inline red banner and Retry button.

- [ ] **Step 9: Commit**

```bash
git add ViewModels/SettingsViewModel.cs ViewModels/MainWindowViewModel.cs Views/SettingsWindow.axaml Views/SettingsWindow.axaml.cs
git commit -m "feat(H2,M2): inline save-failure banner + text-field clamping with main-window summary"
```

---

## Task 13: Convert Update toast to BannerItem

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs` (add `Banners` collection)
- Modify: `ViewModels/MainWindowViewModel.Update.cs`
- Modify: `Views/MainWindow.axaml`

- [ ] **Step 1: Add Banners collection to MainWindowViewModel**

In `ViewModels/MainWindowViewModel.cs`, add:

```csharp
public ObservableCollection<BannerItem> Banners { get; } = new();
```

Add `using System.Collections.ObjectModel;` if not already present.

- [ ] **Step 2: Add BannerStack to MainWindow.axaml**

In `Views/MainWindow.axaml`, replace the Grid.Row=1 placeholder (currently `<Border Grid.Row="1" Height="0"/>` around line 74) with:

```xml
<controls:BannerStack Grid.Row="1" Items="{Binding Banners}"/>
```

Ensure `xmlns:controls="using:LunaApp.Views.Controls"` is declared.

- [ ] **Step 3: Remove the existing update toast XAML block**

In `Views/MainWindow.axaml`, locate the update toast block inside Grid.Row=2 (Classes="fade", bound to HasUpdateAvailable; lines roughly 80-150). Delete the entire `<Border Classes="fade" ... IsVisible="{Binding HasUpdateAvailable}">…</Border>` block.

- [ ] **Step 4: Convert MainWindowViewModel.Update.cs to push BannerItems**

In `ViewModels/MainWindowViewModel.Update.cs`, add partial methods that maintain the banner. Find the existing `[ObservableProperty]` declarations. Below them, add:

```csharp
partial void OnHasUpdateAvailableChanged(bool value)
{
    if (value) PushUpdateBanner();
    else Banners.RemoveByKey("update");
}

partial void OnUpdateVersionChanged(string? value)        => RefreshUpdateBannerIfShown();
partial void OnIsDownloadingUpdateChanged(bool value)     => RefreshUpdateBannerIfShown();
partial void OnUpdateDownloadProgressChanged(int value)   => RefreshUpdateBannerIfShown();
partial void OnIsUpdateReadyChanged(bool value)           => RefreshUpdateBannerIfShown();

private void RefreshUpdateBannerIfShown()
{
    if (HasUpdateAvailable) PushUpdateBanner();
}

private void PushUpdateBanner()
{
    Banners.AddOrReplace(new BannerItem
    {
        Key = "update",
        Level = Level.Info,
        Title = $"Luna {UpdateVersion} is available",
        Body = IsDownloadingUpdate ? $"Downloading… {UpdateDownloadProgress}%"
             : IsUpdateReady       ? "Update ready"
             : null,
        PrimaryAction = IsUpdateReady
            ? new BannerAction("Restart Now", ApplyUpdateCommand)
            : new BannerAction("Download", DownloadUpdateCommand),
        SecondaryAction = new BannerAction("Later", RemindUpdateLaterCommand),
        IsDismissible = true,
        OnDismiss = DismissUpdateCommand,
    });
}
```

Add the required usings at top:

```csharp
using LunaApp.Services;
using LunaApp.ViewModels;
```

- [ ] **Step 5: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 6: Manual smoke test**

Run the app in Debug build. Use the dev tools panel to "Show Update Banner" and "Simulate Download". Verify a slim banner row appears below the header (not a floating top-right card). Verify progress updates the body text. Verify Restart Now button appears when the download completes.

- [ ] **Step 7: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs ViewModels/MainWindowViewModel.Update.cs Views/MainWindow.axaml
git commit -m "feat: convert update toast to BannerStack item"
```

---

## Task 14: Convert Camera-Support toast to BannerItem

**Files:**
- Modify: `ViewModels/MainWindowViewModel.CameraSupport.cs`
- Modify: `Views/MainWindow.axaml`

- [ ] **Step 1: Push BannerItem from the partial**

In `ViewModels/MainWindowViewModel.CameraSupport.cs`, after the existing `[ObservableProperty]` declarations (line 20-21), add partial change handlers:

```csharp
partial void OnHasMissingCameraSupportChanged(bool value)
{
    if (value) PushCameraSupportBanner();
    else Banners.RemoveByKey("camera-support");
}

partial void OnMissingCameraSupportSummaryChanged(string value)
{
    if (HasMissingCameraSupport) PushCameraSupportBanner();
}

private void PushCameraSupportBanner()
{
    Banners.AddOrReplace(new BannerItem
    {
        Key = "camera-support",
        Level = Level.Warning,
        Title = "Camera support missing",
        Body = $"{MissingCameraSupportSummary} not installed. To decode these formats, install the tools from Settings.",
        PrimaryAction = new BannerAction("Open Settings", OpenSettingsForCameraSupportCommand),
        SecondaryAction = new BannerAction("Later", RemindCameraSupportLaterCommand),
        IsDismissible = true,
        OnDismiss = DismissCameraSupportCommand,
    });
}
```

Add `using LunaApp.Services;` and `using LunaApp.ViewModels;` if not already present.

- [ ] **Step 2: Remove the existing camera-support toast XAML**

In `Views/MainWindow.axaml`, locate the camera-support toast block (around lines 155-195, bound to `HasMissingCameraSupport`). Delete the entire `<Border Classes="fade" ... IsVisible="{Binding HasMissingCameraSupport}">…</Border>` block.

- [ ] **Step 3: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 4: Manual smoke test**

Run the app on a machine where ARRI/Sony tools are missing (or simulate by editing `CameraSupportInstallationStatus.ResolveMissing` to return a placeholder list during dev). Verify a warning-colored (amber) banner appears below the header with "Open Settings" / "Later" / dismiss-X. Click Open Settings — Settings dialog opens to the Camera Support section.

- [ ] **Step 5: Commit**

```bash
git add ViewModels/MainWindowViewModel.CameraSupport.cs Views/MainWindow.axaml
git commit -m "feat: convert camera-support toast to BannerStack item (warning level)"
```

---

## Task 15: Wave 3 — copy edits + (Optional) labels + intro rewrite + detect failure copy

**Files:**
- Modify: `Views/MainWindow.axaml`
- Modify: `Views/SettingsWindow.axaml`
- Modify: `ViewModels/SettingsViewModel.cs`
- Modify: `Models/CameraClip.cs`

- [ ] **Step 1: MainWindow drop zone + pending state copy**

In `Views/MainWindow.axaml`:

- Line ~212: change `Text="Drop camera footage here"` → `Text="Drop a folder of camera footage"`.
- Line ~213: change `Text="or click to browse for a folder"` → `Text="or browse"`.
- After the Browse button (around line ~218), add a small caption:

```xml
<TextBlock Text="ARRI · BRAW · Sony VENICE · ProRes · H.264/265"
           Foreground="{DynamicResource LunaTextMuted}"
           FontSize="{DynamicResource LunaFontSizeXs}"
           HorizontalAlignment="Center"
           Margin="0,12,0,0"/>
```

- Line ~243: change `Text="video clips found"` → `Text="video clips ready to scan"`.
- Line ~255 button content: change `"Create Report"` → `"Scan & Continue"`.
- Line ~261 hint TextBlock: replace text with *"We'll read metadata and generate thumbnails. You'll review before exporting."*.

- [ ] **Step 2: MainWindow Clear confirmation**

In `Views/MainWindow.axaml`, find the "Clear & Start Over" button (around line 497). Change its `Command` binding from `{Binding ClearCommand}` to `{Binding ClearWithConfirmCommand}`.

In `ViewModels/MainWindowViewModel.cs`, add the new command. We don't have a dialog service in the VM; defer to the view via an event:

```csharp
public event Func<Task<bool>>? ClearConfirmRequested;

[RelayCommand]
private async Task ClearWithConfirmAsync()
{
    if (!HasReels) { Clear(); return; }
    var confirmed = ClearConfirmRequested is null
        ? true
        : await ClearConfirmRequested.Invoke();
    if (confirmed) Clear();
}
```

In `Views/MainWindow.axaml.cs`, subscribe to `ClearConfirmRequested` in the constructor (after DataContext is set):

```csharp
if (DataContext is MainWindowViewModel vm)
{
    vm.ClearConfirmRequested = ShowClearConfirmAsync;
}

private async Task<bool> ShowClearConfirmAsync()
{
    var dialog = new Window
    {
        Title = "Clear loaded reels?",
        Width = 400,
        Height = 160,
        SystemDecorations = SystemDecorations.BorderOnly,
        WindowStartupLocation = WindowStartupLocation.CenterOwner,
    };

    var tcs = new TaskCompletionSource<bool>();
    var panel = new StackPanel
    {
        Margin = new Thickness(20),
        Spacing = 16,
    };
    panel.Children.Add(new TextBlock
    {
        Text = "Clear all loaded reels and start over?",
        FontSize = 14,
    });
    var buttons = new StackPanel
    {
        Orientation = Orientation.Horizontal,
        Spacing = 8,
        HorizontalAlignment = HorizontalAlignment.Right,
    };
    var cancel = new Button { Content = "Cancel", Classes = { "secondary" } };
    var clear  = new Button { Content = "Clear",  Classes = { "primary" } };
    cancel.Click += (_, _) => { tcs.TrySetResult(false); dialog.Close(); };
    clear.Click  += (_, _) => { tcs.TrySetResult(true);  dialog.Close(); };
    buttons.Children.Add(cancel);
    buttons.Children.Add(clear);
    panel.Children.Add(buttons);
    dialog.Content = panel;

    await dialog.ShowDialog(this);
    return await tcs.Task;
}
```

Required usings: `Avalonia.Controls`, `Avalonia.Layout`, `Avalonia`, `System.Threading.Tasks`.

- [ ] **Step 3: SettingsWindow (Optional) labels + intro rewrite**

In `Views/SettingsWindow.axaml`:

- Line ~103 area: append `" (Optional)"` styled with `LunaTextMuted` next to "DIT", "Director", "DP" labels. Use a `Run` if styling within a single TextBlock:

```xml
<TextBlock>
    <Run Text="DIT"/>
    <Run Text=" (Optional)" Foreground="{DynamicResource LunaTextMuted}"/>
</TextBlock>
```

Repeat for Director and DP.

- Line ~177: change the section intro text to *"Install vendor SDKs to enable proprietary camera formats. Luna detects them automatically."*

- [ ] **Step 4: SettingsViewModel detect-failure text**

In `ViewModels/SettingsViewModel.cs` around line 283, replace:

```csharp
row.InstallError = "Still not detected. Make sure the installer finished — if it's still running, wait for it. If it's done and Luna can't find the install, restart Luna.";
```

with:

```csharp
row.InstallError = "Still not detected. Finish the vendor installer, then try Detect again. If it keeps failing, restart Luna.";
```

- [ ] **Step 5: CameraClip thumbnail suffix updates**

In `Models/CameraClip.cs`, find `ThumbnailIssueSummary` (around line 73-80). Replace the switch cases:

```csharp
public string? ThumbnailIssueSummary => ThumbnailOutcome switch
{
    Models.ThumbnailOutcome.NoDecoder           => "Frames unavailable — install vendor support in Settings.",
    Models.ThumbnailOutcome.SeekFailed          => "Frames unavailable — the file may still be copying. Try again after copy completes.",
    Models.ThumbnailOutcome.DecodeFailed        => "Frames unavailable — decoder couldn't process this file.",
    Models.ThumbnailOutcome.ContainerOpenFailed => "Frames unavailable — couldn't open this file. It may be corrupted or in use.",
    _                                            => null,
};
```

Also add a helper for the click-route case:

```csharp
public bool ThumbnailIssueIsActionable =>
    ThumbnailOutcome == Models.ThumbnailOutcome.NoDecoder;
```

Adjust the switch syntax to match the file's existing style (expression-bodied switch vs. switch statement).

- [ ] **Step 6: Make NoDecoder summary clickable in clip row**

In `Views/MainWindow.axaml` around line 402 the `<TextBlock Text="{Binding ThumbnailIssueSummary}" .../>` renders the summary. Replace that single TextBlock with two siblings — only one visible at a time:

```xml
<!-- Non-actionable outcomes: plain text -->
<TextBlock Text="{Binding ThumbnailIssueSummary}"
           FontSize="{DynamicResource LunaFontSizeXs}"
           Foreground="{DynamicResource LunaTextMuted}"
           IsVisible="{Binding !ThumbnailIssueIsActionable}"
           TextWrapping="Wrap"/>

<!-- NoDecoder: clickable, routes to Settings -->
<Button Content="{Binding ThumbnailIssueSummary}"
        Classes="link"
        Padding="0"
        HorizontalAlignment="Left"
        IsVisible="{Binding ThumbnailIssueIsActionable}"
        Command="{Binding DataContext.OpenSettingsForCameraSupportCommand, RelativeSource={RelativeSource AncestorType=Window}}"/>
```

Add a `Button.link` style to `Styles/Controls.axaml` if not present:

```xml
<Style Selector="Button.link">
    <Setter Property="Background" Value="Transparent"/>
    <Setter Property="BorderThickness" Value="0"/>
    <Setter Property="Foreground" Value="{DynamicResource LunaWarning}"/>
    <Setter Property="FontSize" Value="{DynamicResource LunaFontSizeXs}"/>
    <Setter Property="Cursor" Value="Hand"/>
    <Setter Property="Padding" Value="0"/>
</Style>
<Style Selector="Button.link:pointerover">
    <Setter Property="Foreground" Value="{DynamicResource LunaAccent}"/>
</Style>
```

- [ ] **Step 7: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 8: Manual smoke test**

Run the app. Verify:
- Empty drop zone shows new copy + format list caption.
- Pending state shows "ready to scan" and "Scan & Continue" button + new hint.
- "Clear & Start Over" prompts for confirmation.
- Settings dialog shows "(Optional)" on DIT/Director/DP, new intro text under Camera Support.
- On a clip with `NoDecoder` outcome, the "install vendor support" text is clickable and opens Settings.

- [ ] **Step 9: Commit**

```bash
git add Views/MainWindow.axaml Views/MainWindow.axaml.cs Views/SettingsWindow.axaml ViewModels/SettingsViewModel.cs ViewModels/MainWindowViewModel.cs Models/CameraClip.cs Styles/Controls.axaml
git commit -m "feat: Wave 3 copy + (Optional) labels + Clear confirmation + clickable NoDecoder + thumbnail messages"
```

---

## Phase B checkpoint

Manual smoke checklist:

- [ ] App launches with bottom state strip showing idle message.
- [ ] Drop a folder → state strip updates through scan → review → generate.
- [ ] Cancel mid-generate → state strip shows "Generation cancelled" and output folder has no `.tmp` or partial reports.
- [ ] Successful generate → 2 s hero hold with Open folder / Open report, then state strip success message with inline Open folder.
- [ ] Settings with 200-char Project Name → dialog closes, state strip shows "Project name trimmed to fit".
- [ ] Force a settings-save failure (read-only output folder) → dialog stays open with red Retry banner.
- [ ] Camera-support banner appears below header (warning amber) when tools missing; Dismiss persists across app restart.
- [ ] Update banner (dev tools "Show Update Banner") appears below header, not floating.
- [ ] Drop a folder that doesn't exist → state strip shows "Folder not found", not raw exception.

Phase B complete when all checkboxes pass.

---

## Phase C — Independent backlog

## Task 16: H3 — FFmpeg cancel token in decode loop

**Files:**
- Modify: `Services/Chappie/FfmpegThumbnailService.cs`

- [ ] **Step 1: Add cancellation check inside the per-position decode loop**

In `Services/Chappie/FfmpegThumbnailService.cs` line ~446, inside the `while` loop:

```csharp
while (!frameDecoded && attempts < MaxDecodeAttemptsPerPosition)
{
    cancellationToken.ThrowIfCancellationRequested();
    attempts++;
    ffmpeg.av_packet_unref(pPacket);
    // ... existing decode logic ...
}
```

- [ ] **Step 2: Reorder the catch blocks so OperationCanceledException propagates**

Find the catches at line ~516:

```csharp
catch (OperationCanceledException)
{
    throw;
}
catch (Exception ex)
{
    Log.Debug(ex, "FFmpeg extraction failed for {FileName}", fileName);
    return ThumbnailResult.DecodeFailed(ex.Message);
}
```

The existing `finally` block (line 521-531) is unchanged — it cleans native pointers either way.

- [ ] **Step 3: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 4: Manual smoke test**

Run the app. Drop a folder with several clips (the more the better — 20+ ideal). Start generation, then click Cancel within ~1 second. Verify the processing stops promptly (not after a delay of many seconds).

- [ ] **Step 5: Commit**

```bash
git add Services/Chappie/FfmpegThumbnailService.cs
git commit -m "fix(H3): honor cancellation token inside FFmpeg decode loop"
```

---

## Task 17: M3 — ReportNamePattern token allow-list

**Files:**
- Modify: `Models/ReportSettings.cs`
- Modify: `ViewModels/SettingsViewModel.cs`

- [ ] **Step 1: Add validator helper to ReportSettings**

In `Models/ReportSettings.cs`, add a static method:

```csharp
private static readonly System.Text.RegularExpressions.Regex AllowedTokens =
    new(@"\{(project|reel|date|time)\}", System.Text.RegularExpressions.RegexOptions.Compiled);

/// <summary>
/// Returns true if the pattern contains only the allowed tokens
/// ({project}, {reel}, {date}, {time}) and ordinary characters.
/// </summary>
public static bool IsValidReportNamePattern(string? pattern)
{
    if (string.IsNullOrWhiteSpace(pattern)) return true;
    // Strip allowed tokens and look for any remaining { or } — anything left
    // is an unknown token or stray brace.
    var stripped = AllowedTokens.Replace(pattern, string.Empty);
    return !stripped.Contains('{') && !stripped.Contains('}');
}

public const string DefaultReportNamePattern = "{project}_{reel}_{date}";
```

If `ReportNamePattern`'s current default literal is already defined somewhere, point its initializer to `DefaultReportNamePattern` for consistency.

- [ ] **Step 2: Validate in SettingsViewModel.Save**

In `ViewModels/SettingsViewModel.cs`, inside `Save()` (just before assigning `settings.ReportNamePattern` or just after the existing assignments — locate the line that copies the pattern), add:

```csharp
if (!ReportSettings.IsValidReportNamePattern(settings.ReportNamePattern))
{
    settings.ReportNamePattern = ReportSettings.DefaultReportNamePattern;
    clampReport.Add("Report name pattern", 0, 0); // semantic add — Describe will say "reset"
}
```

If you want the user-facing summary to say "reset" rather than "trimmed", extend `ClampReport` with a second list and adjust `Describe()` — but for this pass, the generic "trimmed to fit" message suffices.

If `ReportNamePattern` is not currently exposed in the SettingsWindow UI, the validation runs against whatever was previously persisted; that's fine.

- [ ] **Step 3: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 4: Commit**

```bash
git add Models/ReportSettings.cs ViewModels/SettingsViewModel.cs
git commit -m "feat(M3): allow-list tokens in ReportNamePattern"
```

---

## Task 18: M4 — Debounce search filter

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs`

- [ ] **Step 1: Add debounce timer field**

In `ViewModels/MainWindowViewModel.cs`, near other private fields:

```csharp
private Avalonia.Threading.DispatcherTimer? _searchDebounceTimer;
```

- [ ] **Step 2: Replace direct rebuild with debounced version**

Find `OnSearchTextChanged` (or `partial void OnSearchTextChanged(string value)` around line 209). Replace its body with:

```csharp
partial void OnSearchTextChanged(string value)
{
    _searchDebounceTimer ??= new Avalonia.Threading.DispatcherTimer
    {
        Interval = TimeSpan.FromMilliseconds(300),
    };
    _searchDebounceTimer.Stop();

    void Tick(object? sender, EventArgs e)
    {
        _searchDebounceTimer!.Stop();
        _searchDebounceTimer.Tick -= Tick;
        RebuildFilteredReels();
        OnPropertyChanged(nameof(IsSearchActive));
        OnPropertyChanged(nameof(HasFilteredReels));
        OnPropertyChanged(nameof(FilterSummary));
    }
    _searchDebounceTimer.Tick += Tick;
    _searchDebounceTimer.Start();
}
```

If the existing handler already does extra work (e.g. clears a state), preserve it inside the Tick handler.

- [ ] **Step 3: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 4: Manual smoke test**

Load a folder with several reels. Type quickly into the search box. Verify the list updates once after typing stops, not on every keystroke.

- [ ] **Step 5: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs
git commit -m "perf(M4): debounce search filter rebuild by 300 ms"
```

---

## Task 19: M5 — Auto-fill collision

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs`
- Modify: `ViewModels/MainWindowViewModel.Import.cs`

- [ ] **Step 1: Replace the collision-prone string flag with a bool**

In `ViewModels/MainWindowViewModel.cs`, locate `_autoFilledReportName` and replace with:

```csharp
private bool _userEditedReportName;
private bool _settingAutoFilledReportName;
```

Find the `OnReportNameChanged` partial method (or add one). Replace with:

```csharp
partial void OnReportNameChanged(string? value)
{
    if (!_settingAutoFilledReportName)
        _userEditedReportName = true;
}
```

- [ ] **Step 2: Update QuickScanFolderAsync auto-fill block**

In `ViewModels/MainWindowViewModel.Import.cs`, replace the existing auto-fill block (lines 88-93) with:

```csharp
if (!string.IsNullOrEmpty(folderName) && !_userEditedReportName)
{
    _settingAutoFilledReportName = true;
    ReportName = folderName;
    _settingAutoFilledReportName = false;
}
```

- [ ] **Step 3: Reset on Clear**

In `Clear()` (line 309), reset the flag:

```csharp
_userEditedReportName = false;
```

- [ ] **Step 4: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 5: Manual smoke test**

1. Drop folder A → ReportName auto-fills to folder A's name.
2. Type a custom name into Report name box.
3. Drop folder B → custom name is NOT overwritten.
4. Click Clear → drop folder C → ReportName auto-fills to folder C's name.

- [ ] **Step 6: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs ViewModels/MainWindowViewModel.Import.cs
git commit -m "fix(M5): track user-edited report name explicitly to avoid silent overwrites"
```

---

## Task 20: M10 — ETA smoothing

**Files:**
- Modify: `ViewModels/MainWindowViewModel.cs`

- [ ] **Step 1: Add smoothing state and update logic**

In `ViewModels/MainWindowViewModel.cs`, find the ETA computation (around line 389-408). Add fields:

```csharp
private double _smoothedMsPerItem;
private int _itemsProcessedThisOp;
```

Reset them at the start of each operation. In `BeginOperation` (or wherever an operation starts), add:

```csharp
_smoothedMsPerItem = 0;
_itemsProcessedThisOp = 0;
```

In the ETA update path (where `msPerItem = …` is computed today), replace with:

```csharp
_itemsProcessedThisOp++;
var newSample = elapsed.TotalMilliseconds / Math.Max(1, _itemsProcessedThisOp);

_smoothedMsPerItem = _smoothedMsPerItem == 0
    ? newSample
    : _smoothedMsPerItem * 0.7 + newSample * 0.3;

if (_itemsProcessedThisOp < 2)
{
    EtaText = null;
}
else
{
    var remaining = total - current;
    var msRemaining = remaining * _smoothedMsPerItem;
    EtaText = FormatEta(TimeSpan.FromMilliseconds(msRemaining));
}
```

Adjust variable names (`elapsed`, `current`, `total`) to whatever the existing code uses. Keep `FormatEta` (or equivalent) unchanged.

- [ ] **Step 2: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 3: Manual smoke test**

Run a generation on a 5+ clip project. Verify ETA does NOT show on the first clip, then appears stably from the second clip onward without wild swings.

- [ ] **Step 4: Commit**

```bash
git add ViewModels/MainWindowViewModel.cs
git commit -m "fix(M10): hide ETA for first item and smooth subsequent estimates"
```

---

## Task 21: M11 — Sony locator Windows guard

**Files:**
- Modify: `Services/Chappie/SonyRawViewerLocator.cs`

- [ ] **Step 1: Wrap the hard-coded path in a Windows check**

In `Services/Chappie/SonyRawViewerLocator.cs`, find around line 70:

```csharp
yield return @"C:\Program Files\Sony\RAW Viewer\rawexporter.exe";
```

Wrap:

```csharp
if (System.Runtime.InteropServices.RuntimeInformation.IsOSPlatform(System.Runtime.InteropServices.OSPlatform.Windows))
{
    yield return @"C:\Program Files\Sony\RAW Viewer\rawexporter.exe";
}
```

If the file already has a `using System.Runtime.InteropServices;` at top, the fully-qualified names can be simplified.

- [ ] **Step 2: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 3: Commit**

```bash
git add Services/Chappie/SonyRawViewerLocator.cs
git commit -m "fix(M11): only probe hard-coded Sony path on Windows"
```

---

## Task 22: L1 — Disable cancel during tail animation

**Files:**
- Modify: `ViewModels/MainWindowViewModel.Import.cs`

- [ ] **Step 1: Track tail state**

In `ViewModels/MainWindowViewModel.Import.cs`, add a private field at the top of the partial class (near `_currentOperationCts`):

```csharp
private bool _isInTail;
```

Update the existing `CanCancel` property at line 30 of `Import.cs`:

```csharp
public bool CanCancel =>
    IsProcessing
    && !_isInTail
    && _currentOperationCts is { IsCancellationRequested: false };
```

Do not redeclare `CanCancel` — modify the existing line in place. There must be exactly one definition across the partial classes.

- [ ] **Step 2: Set the flag at the start of EndOperationAsync**

In `EndOperationAsync` (now updated by Task 11), set `_isInTail = true;` immediately after `OverallProgress = 100;`. Reset to false in the cleanup block (after `_overlayState = OverlayState.Idle`):

```csharp
private async Task EndOperationAsync()
{
    OverallProgress = 100;
    Progress = 100;
    _isInTail = true;
    OnPropertyChanged(nameof(CanCancel));

    // ... existing hold/tail logic ...

    _isInTail = false;
    _currentOperationCts = null;
    IsProcessing = false;
    OverallProgress = 0;
    _overlayState = OverlayState.Idle;
    SuccessLabel = null;
    OnPropertyChanged(nameof(OverlayState));
    OnPropertyChanged(nameof(CanCancel));

    // ... existing _pendingOpenAfterTail handling ...
}
```

- [ ] **Step 3: Guard CancelProcessing**

In `CancelProcessing` (line 181-191), add the disposed guard:

```csharp
[RelayCommand]
private void CancelProcessing()
{
    if (_isInTail) return;
    if (_currentOperationCts is { IsCancellationRequested: false } cts)
    {
        try
        {
            Log.Information("User requested cancellation of current operation");
            State = StateMessage.Info("Cancelling…");
            cts.Cancel();
        }
        catch (ObjectDisposedException)
        {
            // Race: operation finished between the IsCancellationRequested
            // check and the Cancel call. Nothing to do.
        }
        OnPropertyChanged(nameof(CanCancel));
    }
}
```

- [ ] **Step 4: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 5: Manual smoke test**

Run a generation. Wait for the success hold (or scan tail). During the 2 s hold, verify the Cancel button is disabled (greyed out). After overlay fades, Cancel button is appropriately hidden because IsProcessing is false.

- [ ] **Step 6: Commit**

```bash
git add ViewModels/MainWindowViewModel.Import.cs
git commit -m "fix(L1): disable cancel during overlay tail and guard against disposed CTS"
```

---

## Task 23: L2 — Atomic AppSettings.Save

**Files:**
- Modify: `Models/AppSettings.cs`

- [ ] **Step 1: Replace Save() with temp+rename**

In `Models/AppSettings.cs`, replace `Save()` (line 82-101) with:

```csharp
public bool Save()
{
    try
    {
        var directory = Path.GetDirectoryName(SettingsPath);
        if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var json = JsonSerializer.Serialize(this, AppSettingsJsonContext.Default.AppSettings);

        var tempPath = SettingsPath + ".tmp";
        File.WriteAllText(tempPath, json);
        File.Move(tempPath, SettingsPath, overwrite: true);
        return true;
    }
    catch (Exception ex)
    {
        Serilog.Log.Error(ex, "Failed to save settings to {Path}", SettingsPath);
        return false;
    }
}
```

- [ ] **Step 2: Build to verify**

```bash
dotnet build LunaApp.csproj
```

Expected: succeeds.

- [ ] **Step 3: Verify existing tests still pass**

```bash
dotnet test LunaApp.Tests/LunaApp.Tests.csproj
```

The existing `AppSettingsTests` should still pass — they test serialization round-trip, which the atomic save preserves.

- [ ] **Step 4: Commit**

```bash
git add Models/AppSettings.cs
git commit -m "fix(L2): atomic AppSettings.Save via temp+rename"
```

---

## Phase C checkpoint

Manual smoke checklist:

- [ ] Cancel during a 20+ clip generation stops within ~1 s.
- [ ] Search box doesn't stutter on rapid typing.
- [ ] Auto-fill of Report Name respects user edits.
- [ ] ETA hides for first item, smooths thereafter.
- [ ] App still launches on macOS (M11 didn't break non-Windows path).
- [ ] Cancel button disabled during overlay tail.
- [ ] Settings file written atomically (write a settings change, kill the process between write and rename in a debug run — the existing file remains intact).

Phase C complete when all checkboxes pass.

---

## Final verification

```bash
dotnet build LunaApp.csproj
dotnet test LunaApp.Tests/LunaApp.Tests.csproj
```

Run a full smoke through Phase B + Phase C checklists above. If everything passes:

```bash
git log --oneline | head -25
```

Should show ~17 commits scoped to this quality pass (one per task plus possibly merges).
