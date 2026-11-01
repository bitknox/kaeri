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
/// saves to the device's style like Settings does (<c>SetSetting</c>), and the panel redraws from
/// the style the engine answers with, so the picture behind it shows the result at once.
/// </summary>
sealed class SubtitleStylePanel
{
    public Flyout Flyout { get; } = new() { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.TopEdgeAlignedRight };

    readonly Ctx ctx;
    bool quiet;

    public SubtitleStylePanel(Ctx ctx)
    {
        this.ctx = ctx;
        Flyout.Opening += (_, _) => Build();
    }

    /// <summary>Redraws from the style the engine answered with, while the panel is up.</summary>
    public void Heard(IReadOnlyList<Event> batch)
    {
        if (Flyout.IsOpen && batch.Any(e => e is Event.Settings))
            Build();
    }

    void Build()
    {
        quiet = true;
        var panel = new StackPanel { Spacing = 12, Width = 280 };
        if (ctx.Known.Settings?.SubtitleStyle is not { } style)
        {
            Flyout.Content = panel;
            quiet = false;
            return;
        }
        panel.Children.Add(new TextBlock { Text = T.PlayerSubsStyle, Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"] });

        panel.Children.Add(Stepper(T.SettingsSubsSize, T.PlayerSubsSmaller, T.PlayerSubsLarger,
            T.SettingsSubsSizeValue(style.Size), style.Size > 50, style.Size < 200,
            by => Send(SettingKey.SubtitleSize, new SettingValue.Int(Math.Clamp((long)style.Size + by * 10, 50, 200)))));
        panel.Children.Add(Stepper(T.SettingsSubsRaise, T.PlayerSubsLower, T.PlayerSubsRaise,
            T.SettingsSubsSizeValue(style.Raise), style.Raise > 0, style.Raise < 20,
            by => Send(SettingKey.SubtitleRaise, new SettingValue.Int(Math.Clamp((long)style.Raise + by * 2, 0, 20)))));

        // Outline and Shadow are edges with no box; Box is a plate behind the lines.
        var look = style.Background != SubtitleBackground.NoBackground ? 2 : style.Edge == SubtitleEdge.DropShadow ? 1 : 0;
        var backgrounds = new RadioButtons { MaxColumns = 3 };
        AutomationProperties.SetName(backgrounds, T.SettingsSubsBackground);
        foreach (var name in new[] { T.PlayerSubsBgOutline, T.PlayerSubsBgShadow, T.PlayerSubsBgBox })
            backgrounds.Items.Add(new RadioButton { Content = name, MinWidth = 0 });
        backgrounds.SelectedIndex = look;
        backgrounds.SelectionChanged += (_, _) =>
        {
            if (quiet || backgrounds.SelectedIndex < 0 || backgrounds.SelectedIndex == look)
                return;
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

        var choices = new[]
        {
            (StyledSubtitles.Keep, "keep", T.SettingsSubsStyledKeep),
            (StyledSubtitles.Restyle, "restyle", T.SettingsSubsStyledRestyle),
            (StyledSubtitles.Strip, "strip", T.SettingsSubsStyledStrip),
        };
        var styled = new ComboBox { Header = T.SettingsSubsStyled, HorizontalAlignment = HorizontalAlignment.Stretch, ItemsSource = choices.Select(c => c.Item3).ToList() };
        styled.SelectedIndex = Array.FindIndex(choices, c => c.Item1 == style.Styled);
        styled.SelectionChanged += (_, _) =>
        {
            if (!quiet && styled.SelectedIndex >= 0 && choices[styled.SelectedIndex].Item1 != style.Styled)
                Send(SettingKey.StyledSubtitles, new SettingValue.Str(choices[styled.SelectedIndex].Item2));
        };
        panel.Children.Add(styled);
        Flyout.Content = panel;
        quiet = false;
    }

    void Send(SettingKey key, SettingValue value) => ctx.Send(new Command.SetSetting(key, value));

    /// <summary>A label with a value between minus and plus buttons.</summary>
    static FrameworkElement Stepper(string label, string lessName, string moreName, string value, bool canLess, bool canMore, Action<int> step)
    {
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(64) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Button Make(string glyph, string name, bool enabled, int by)
        {
            var b = new Button { Content = new FontIcon { Glyph = glyph, FontSize = 12 }, IsEnabled = enabled, Width = 36, Height = 32, Padding = new Thickness(0) };
            AutomationProperties.SetName(b, name);
            ToolTipService.SetToolTip(b, name);
            b.Click += (_, _) => step(by);
            return b;
        }
        var text = new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center };
        var number = new TextBlock { Text = value, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        var less = Make("", lessName, canLess, -1);
        var more = Make("", moreName, canMore, 1);
        Grid.SetColumn(less, 1);
        Grid.SetColumn(number, 2);
        Grid.SetColumn(more, 3);
        row.Children.Add(text);
        row.Children.Add(less);
        row.Children.Add(number);
        row.Children.Add(more);
        return row;
    }
}
