using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Kaeri.Windows.Player;
using CommunityToolkit.WinUI.Controls;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Settings, Playback, Subtitles: the preview, then the ten rows of the spec in their order. A
/// slider shows its value in the preview while it is dragged and saves when it is let go, so the
/// page does not redraw under the pointer. The style is the engine's: every change is a
/// <c>SetSetting</c>, and the engine answers with the style in effect.
/// </summary>
sealed class SubtitleSettings
{
    /// <summary>Which tab the preview shows, kept while the page redraws.</summary>
    static bool plainTab = true;

    readonly Ctx ctx;
    readonly SubtitlePreview preview = new();
    readonly Func<bool> quiet;
    SubtitleStyle shown;

    public SubtitleSettings(Ctx ctx, SubtitleStyle style, Func<bool> quiet)
    {
        this.ctx = ctx;
        this.quiet = quiet;
        shown = style;
    }

    /// <summary>The group's elements, under a heading the page adds.</summary>
    public IEnumerable<FrameworkElement> Rows()
    {
        var style = shown;
        var system = SubtitleStyling.System;
        // The platform has a caption style to follow; the rows say so while they do.
        var following = style.FollowSystem && system is not null;

        var tabs = new RadioButtons { MaxColumns = 2, Margin = new Thickness(0, 0, 0, 4) };
        tabs.Items.Add(new RadioButton { Content = T.SettingsSubsPreviewPlain, Tag = true });
        tabs.Items.Add(new RadioButton { Content = T.SettingsSubsPreviewStyled, Tag = false });
        tabs.SelectedIndex = plainTab ? 0 : 1;
        tabs.SelectionChanged += (_, _) =>
        {
            if (tabs.SelectedItem is RadioButton { Tag: bool plain })
            {
                plainTab = plain;
                preview.Draw(shown, plainTab, SubtitleStyling.System);
            }
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(tabs, T.SettingsSubsPreview);
        preview.Draw(style, plainTab, system);
        var top = new StackPanel { Spacing = 8, Margin = new Thickness(0, 0, 0, 8) };
        top.Children.Add(tabs);
        top.Children.Add(preview);
        yield return top;

        // Follow system caption style: on by default, and there is one to follow on Windows.
        var follow = new ToggleSwitch { IsOn = style.FollowSystem, OnContent = "", OffContent = "", MinWidth = 0 };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(follow, T.SettingsSystemCaptions);
        follow.Toggled += (_, _) =>
        {
            if (!quiet())
                ctx.Send(new Command.SetSetting(SettingKey.FollowSystemCaptions, new SettingValue.Bool(follow.IsOn)));
        };
        yield return Card(T.SettingsSystemCaptions, T.SettingsSystemCaptionsDetail, follow);

        yield return SliderCard(T.SettingsSubsSize, SettingKey.SubtitleSize, style.Size, 50, 200, 10, T.SettingsSubsSizeValue,
            v => shown with { Size = (uint)v });

        yield return following
            ? FromSystem(T.SettingsSubsFont)
            : Pick(T.SettingsSubsFont, SettingKey.SubtitleFont, style.Font,
            [
                (SubtitleFont.Default, "default", T.SettingsSubsFontDefault),
                (SubtitleFont.ZenKakuGothicNew, "zen-kaku-gothic-new", "Zen Kaku Gothic New"),
                (SubtitleFont.Serif, "serif", T.SettingsSubsFontSerif),
                (SubtitleFont.Monospace, "monospace", T.SettingsSubsFontMono),
            ], v => shown with { Font = v });

        yield return following ? FromSystem(T.SettingsSubsColour) : ColourCard(style.Colour);

        yield return following
            ? FromSystem(T.SettingsSubsEdge)
            : Pick(T.SettingsSubsEdge, SettingKey.SubtitleEdge, style.Edge,
            [
                (SubtitleEdge.Outline, "outline", T.SettingsSubsEdgeOutline),
                (SubtitleEdge.DropShadow, "drop-shadow", T.SettingsSubsEdgeShadow),
                (SubtitleEdge.NoEdge, "none", T.SettingsSubsEdgeNone),
            ], v => shown with { Edge = v });
        if (!following && style.Edge == SubtitleEdge.Outline)
            yield return Pick(T.SettingsSubsThickness, SettingKey.SubtitleThickness, style.Thickness,
            [
                (SubtitleThickness.Thin, "thin", T.SettingsSubsThin),
                (SubtitleThickness.Medium, "medium", T.SettingsSubsMedium),
                (SubtitleThickness.Thick, "thick", T.SettingsSubsThick),
            ], v => shown with { Thickness = v });

        yield return following
            ? FromSystem(T.SettingsSubsBackground)
            : Pick(T.SettingsSubsBackground, SettingKey.SubtitleBackground, style.Background,
            [
                (SubtitleBackground.NoBackground, "none", T.SettingsSubsBackgroundNone),
                (SubtitleBackground.EachLine, "each-line", T.SettingsSubsBackgroundLine),
                (SubtitleBackground.AllLines, "all-lines", T.SettingsSubsBackgroundAll),
            ], v => shown with { Background = v });
        if (following)
            yield return SystemLink();
        else if (style.Background != SubtitleBackground.NoBackground)
            yield return Pick(T.SettingsSubsOpacity, SettingKey.SubtitleOpacity, (int)style.Opacity,
            [
                (50, 50L, T.SettingsSubsSizeValue(50)),
                (75, 75L, T.SettingsSubsSizeValue(75)),
                (100, 100L, T.SettingsSubsSizeValue(100)),
            ], v => shown with { Opacity = (uint)v });

        yield return SliderCard(T.SettingsSubsRaise, SettingKey.SubtitleRaise, style.Raise, 0, 20, 2, T.SettingsSubsRaiseValue,
            v => shown with { Raise = (uint)v });

        var styledDetail = style.Styled switch
        {
            StyledSubtitles.Restyle => T.SettingsSubsStyledRestyleDetail,
            StyledSubtitles.Strip => T.SettingsSubsStyledStripDetail,
            _ => T.SettingsSubsStyledKeepDetail,
        };
        yield return Pick(T.SettingsSubsStyled, SettingKey.StyledSubtitles, style.Styled,
        [
            (StyledSubtitles.Keep, "keep", T.SettingsSubsStyledKeep),
            (StyledSubtitles.Restyle, "restyle", T.SettingsSubsStyledRestyle),
            (StyledSubtitles.Strip, "strip", T.SettingsSubsStyledStrip),
        ], v => shown with { Styled = v }, styledDetail);

        var reset = new Button { Content = T.SettingsSubsReset, Margin = new Thickness(0, 8, 0, 0) };
        reset.Click += (_, _) =>
        {
            ctx.Known.StyleBeforeReset = shown;
            ctx.Send(new Command.ResetSubtitleStyle());
        };
        yield return reset;
    }

    static SettingsCard Card(string title, string? description, FrameworkElement? control) =>
        new() { Header = title, Description = description, Content = control };

    /// <summary>A row that shows the system's value, dimmed, while the style follows the system's captions.</summary>
    static SettingsCard FromSystem(string title)
    {
        var value = new TextBlock { Text = T.SettingsSubsSystemValue, VerticalAlignment = VerticalAlignment.Center };
        var card = Card(title, null, value);
        card.Opacity = 0.6;
        return card;
    }

    FrameworkElement SystemLink()
    {
        var link = new HyperlinkButton { Content = T.SettingsSubsSystemChange, Padding = new Thickness(4, 0, 0, 0) };
        link.Click += async (_, _) =>
        {
            try
            {
                await global::Windows.System.Launcher.LaunchUriAsync(new Uri("ms-settings:easeofaccess-closedcaptioning"));
            }
            catch (Exception)
            {
            }
        };
        return link;
    }

    SettingsCard Pick<TValue>(string title, SettingKey key, TValue current, (TValue Value, object Name, string Shown)[] options,
        Func<TValue, SubtitleStyle> preview, string? detail = null) where TValue : notnull
    {
        var box = new ComboBox { MinWidth = 200, ItemsSource = options.Select(o => o.Shown).ToList() };
        box.SelectedIndex = Array.FindIndex(options, o => EqualityComparer<TValue>.Default.Equals(o.Value, current));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, title);
        box.SelectionChanged += (_, _) =>
        {
            if (quiet() || box.SelectedIndex < 0)
                return;
            var chosen = options[box.SelectedIndex];
            if (EqualityComparer<TValue>.Default.Equals(chosen.Value, current))
                return;
            shown = preview(chosen.Value);
            this.preview.Draw(shown, plainTab, SubtitleStyling.System);
            ctx.Send(new Command.SetSetting(key, chosen.Name switch
            {
                long n => new SettingValue.Int(n),
                string s => new SettingValue.Str(s),
                _ => throw new InvalidOperationException(),
            }));
        };
        return Card(title, detail, box);
    }

    /// <summary>The four colours as swatches with their names: colour is never the only signal.</summary>
    SettingsCard ColourCard(SubtitleColour current)
    {
        var choices = new (SubtitleColour Value, string Name, string Shown, global::Windows.UI.Color Swatch)[]
        {
            (SubtitleColour.White, "white", T.SettingsSubsColourWhite, Colors.White),
            (SubtitleColour.Yellow, "yellow", T.SettingsSubsColourYellow, Colors.Yellow),
            (SubtitleColour.Cyan, "cyan", T.SettingsSubsColourCyan, Colors.Cyan),
            (SubtitleColour.Green, "green", T.SettingsSubsColourGreen, Colors.Lime),
        };
        var group = new RadioButtons { MaxColumns = 4 };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(group, T.SettingsSubsColour);
        foreach (var c in choices)
        {
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            content.Children.Add(new Border
            {
                Width = 16,
                Height = 16,
                CornerRadius = new CornerRadius(8),
                Background = new SolidColorBrush(c.Swatch),
                BorderBrush = new SolidColorBrush(Colors.Gray),
                BorderThickness = new Thickness(1),
            });
            content.Children.Add(new TextBlock { Text = c.Shown });
            group.Items.Add(new RadioButton { Content = content, Tag = c.Name, MinWidth = 0 });
        }
        group.SelectedIndex = Array.FindIndex(choices, c => c.Value == current);
        group.SelectionChanged += (_, _) =>
        {
            if (quiet() || group.SelectedIndex < 0 || choices[group.SelectedIndex].Value == current)
                return;
            shown = shown with { Colour = choices[group.SelectedIndex].Value };
            preview.Draw(shown, plainTab, SubtitleStyling.System);
            ctx.Send(new Command.SetSetting(SettingKey.SubtitleColour, new SettingValue.Str(choices[group.SelectedIndex].Name)));
        };
        return Card(T.SettingsSubsColour, null, group);
    }

    SettingsCard SliderCard(string title, SettingKey key, uint current, double low, double high, double step,
        Func<long, string> text, Func<long, SubtitleStyle> preview)
    {
        var slider = new Slider
        {
            Minimum = low,
            Maximum = high,
            StepFrequency = step,
            SmallChange = step,
            LargeChange = step,
            Value = current,
            Width = 180,
            TickFrequency = step,
        };
        var value = new TextBlock { Text = text(current), MinWidth = 90, VerticalAlignment = VerticalAlignment.Center, TextAlignment = TextAlignment.Right };
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        row.Children.Add(slider);
        row.Children.Add(value);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(slider, title);
        var held = false;
        var saved = (long)current;
        void Save()
        {
            var now = (long)slider.Value;
            if (now == saved)
                return;
            saved = now;
            ctx.Send(new Command.SetSetting(key, new SettingValue.Int(now)));
        }
        slider.ValueChanged += (_, _) =>
        {
            var now = (long)slider.Value;
            value.Text = text(now);
            if (quiet())
                return;
            shown = preview(now);
            this.preview.Draw(shown, plainTab, SubtitleStyling.System);
            // A key or a wheel step saves at once; a drag waits until it is let go.
            if (!held)
                Save();
        };
        slider.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) => held = true), true);
        void Released(object s, PointerRoutedEventArgs e)
        {
            held = false;
            Save();
        }
        slider.AddHandler(UIElement.PointerReleasedEvent, new PointerEventHandler(Released), true);
        slider.AddHandler(UIElement.PointerCaptureLostEvent, new PointerEventHandler(Released), true);
        return Card(title, null, row);
    }
}
