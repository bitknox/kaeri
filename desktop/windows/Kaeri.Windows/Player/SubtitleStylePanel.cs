using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Player;

/// <summary>
/// The player's quick subtitle style: a flyout from the subtitles button with Size and Raise
/// steppers, the background as Outline, Shadow or Box, and the styled-subtitles choice. A change
/// saves to the device's style like Settings does (<c>SetSetting</c>). The panel is made once and
/// its values change in place when the engine answers, so the button pressed stays under the
/// pointer and quick presses add up from the value asked for last.
/// </summary>
sealed class SubtitleStylePanel
{
    public Flyout Flyout { get; } = new() { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.TopEdgeAlignedRight };

    readonly Ctx ctx;
    bool quiet;
    StackPanel? panel;
    Stepper? size, raise;
    RadioButtons? backgrounds;
    ComboBox? styled;
    int look;
    StyledSubtitles shownStyled;

    /// <summary>A step asked for and not yet answered: the next press steps on from it.</summary>
    (long Value, DateTime At)? sizeAsked, raiseAsked;

    static readonly (StyledSubtitles Value, string Key, string Label)[] Choices =
    [
        (StyledSubtitles.Keep, "keep", T.SettingsSubsStyledKeep),
        (StyledSubtitles.Restyle, "restyle", T.SettingsSubsStyledRestyle),
        (StyledSubtitles.Strip, "strip", T.SettingsSubsStyledStrip),
    ];

    public SubtitleStylePanel(Ctx ctx)
    {
        this.ctx = ctx;
        Flyout.Opening += (_, _) => Show();
    }

    /// <summary>The style the engine answered with, while the panel is up: its values, in place.</summary>
    public void Heard(IReadOnlyList<Event> batch)
    {
        if (Flyout.IsOpen && batch.Any(e => e is Event.Settings))
            Show();
    }

    void Show()
    {
        if (ctx.Known.Settings?.SubtitleStyle is not { } style)
        {
            Flyout.Content = panel = null;
            return;
        }
        if (panel is null)
            Build();
        quiet = true;
        var sizeNow = Settled(ref sizeAsked, style.Size);
        var raiseNow = Settled(ref raiseAsked, style.Raise);
        size!.Show(T.SettingsSubsSizeValue(sizeNow), sizeNow > 50, sizeNow < 200);
        raise!.Show(T.SettingsSubsSizeValue(raiseNow), raiseNow > 0, raiseNow < 20);
        // Outline and Shadow are edges with no box; Box is a plate behind the lines.
        look = style.Background != SubtitleBackground.NoBackground ? 2 : style.Edge == SubtitleEdge.DropShadow ? 1 : 0;
        if (backgrounds!.SelectedIndex != look)
            backgrounds.SelectedIndex = look;
        shownStyled = style.Styled;
        var index = Array.FindIndex(Choices, c => c.Value == style.Styled);
        if (styled!.SelectedIndex != index)
            styled.SelectedIndex = index;
        quiet = false;
    }

    /// <summary>The value to show: the one asked for until the engine says it, or for 2 s.</summary>
    static long Settled(ref (long Value, DateTime At)? asked, long engine)
    {
        if (asked is { } a && (a.Value == engine || DateTime.UtcNow - a.At > TimeSpan.FromSeconds(2)))
            asked = null;
        return asked?.Value ?? engine;
    }

    void Build()
    {
        panel = new StackPanel { Spacing = 12, Width = 280 };
        panel.Children.Add(new TextBlock { Text = T.PlayerSubsStyle, Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"] });

        size = new Stepper(T.SettingsSubsSize, T.PlayerSubsSmaller, T.PlayerSubsLarger,
            by => Step(ref sizeAsked, SettingKey.SubtitleSize, ctx.Known.Settings?.SubtitleStyle?.Size, by * 10, 50, 200));
        // One step at a time, as on the Mac.
        raise = new Stepper(T.SettingsSubsRaise, T.PlayerSubsLower, T.PlayerSubsRaise,
            by => Step(ref raiseAsked, SettingKey.SubtitleRaise, ctx.Known.Settings?.SubtitleStyle?.Raise, by, 0, 20));
        panel.Children.Add(size.Row);
        panel.Children.Add(raise.Row);

        backgrounds = new RadioButtons { MaxColumns = 3 };
        AutomationProperties.SetName(backgrounds, T.SettingsSubsBackground);
        foreach (var name in new[] { T.PlayerSubsBgOutline, T.PlayerSubsBgShadow, T.PlayerSubsBgBox })
            backgrounds.Items.Add(new RadioButton { Content = name, MinWidth = 0 });
        backgrounds.SelectionChanged += (_, _) =>
        {
            if (quiet || backgrounds.SelectedIndex < 0 || backgrounds.SelectedIndex == look)
                return;
            look = backgrounds.SelectedIndex;
            switch (backgrounds.SelectedIndex)
            {
                case 0:
                    Send(SettingKey.SubtitleEdge, new SettingValue.Str("outline"));
                    Send(SettingKey.SubtitleBackground, new SettingValue.Str("none"));
                    break;
                case 1:
                    Send(SettingKey.SubtitleEdge, new SettingValue.Str("drop-shadow"));
                    Send(SettingKey.SubtitleBackground, new SettingValue.Str("none"));
                    break;
                default:
                    Send(SettingKey.SubtitleBackground, new SettingValue.Str("each-line"));
                    break;
            }
        };
        panel.Children.Add(backgrounds);

        styled = new ComboBox { Header = T.SettingsSubsStyled, HorizontalAlignment = HorizontalAlignment.Stretch, ItemsSource = Choices.Select(c => c.Label).ToList() };
        styled.SelectionChanged += (_, _) =>
        {
            if (!quiet && styled.SelectedIndex >= 0 && Choices[styled.SelectedIndex].Value != shownStyled)
            {
                shownStyled = Choices[styled.SelectedIndex].Value;
                Send(SettingKey.StyledSubtitles, new SettingValue.Str(Choices[styled.SelectedIndex].Key));
            }
        };
        panel.Children.Add(styled);
        Flyout.Content = panel;
    }

    /// <summary>A stepper press: from the value asked for last, held to its range, saved, and shown at once.</summary>
    void Step(ref (long Value, DateTime At)? asked, SettingKey key, long? engine, int by, long min, long max)
    {
        if (engine is not { } now)
            return;
        var from = asked?.Value ?? now;
        var to = Math.Clamp(from + by, min, max);
        if (to == from)
            return;
        asked = (to, DateTime.UtcNow);
        Send(key, new SettingValue.Int(to));
        Show();
    }

    void Send(SettingKey key, SettingValue value) => ctx.Send(new Command.SetSetting(key, value));

    /// <summary>A label with a value between minus and plus buttons, made once and changed in place.</summary>
    sealed class Stepper
    {
        public Grid Row { get; } = new() { ColumnSpacing = 8 };
        readonly TextBlock number = new() { HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        readonly Button less, more;

        public Stepper(string label, string lessName, string moreName, Action<int> step)
        {
            Row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(64) });
            Row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Button Make(string glyph, string name, int by)
            {
                var b = new Button { Content = new FontIcon { Glyph = glyph, FontSize = 12 }, Width = 36, Height = 32, Padding = new Thickness(0) };
                AutomationProperties.SetName(b, name);
                ToolTipService.SetToolTip(b, name);
                b.Click += (_, _) => step(by);
                return b;
            }
            less = Make("", lessName, -1);
            more = Make("", moreName, 1);
            Grid.SetColumn(less, 1);
            Grid.SetColumn(number, 2);
            Grid.SetColumn(more, 3);
            Row.Children.Add(new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center });
            Row.Children.Add(less);
            Row.Children.Add(number);
            Row.Children.Add(more);
        }

        public void Show(string value, bool canLess, bool canMore)
        {
            number.Text = value;
            less.IsEnabled = canLess;
            more.IsEnabled = canMore;
        }
    }
}
