using Kaeri.App;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Casting;

/// <summary>The pieces the mini, the controller and the menus share, as the Windows board draws them.</summary>
static class CastParts
{
    /// <summary>An icon button with no chrome until hovered: 40 by 36, corner 4 (the board's wib2).</summary>
    public static Button Icon(FrameworkElement glyph, string name, double width = 40, double height = 36)
    {
        var button = new Button
        {
            Content = glyph,
            Width = width,
            Height = height,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        Name(button, name);
        return button;
    }

    public static FontIcon Glyph(string glyph, double size) => new() { Glyph = glyph, FontSize = size };

    /// <summary>The play or pause button in its round: 36 in the mini, 48 in the controller.</summary>
    public static Button Round(FontIcon glyph, double size)
    {
        var button = new Button
        {
            Content = glyph,
            Width = size,
            Height = size,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(size / 2),
            Background = Ui.Res<Brush>("SelectedFillBrush"),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        return button;
    }

    /// <summary>Sets the play or pause glyph and its name, only when it changes, so focus and a press stay put.</summary>
    public static void PlayGlyph(Button button, FontIcon glyph, bool paused)
    {
        var want = paused ? "" : "";
        if (glyph.Glyph == want)
            return;
        glyph.Glyph = want;
        Name(button, paused ? T.ActionPlay : T.ActionPause);
    }

    /// <summary>Back or forward 10 seconds: the turning arrow with its digits, mirrored for back.</summary>
    public static FrameworkElement Ten(bool back, double size)
    {
        var icon = new FontIcon { Glyph = "", FontSize = size, RenderTransformOrigin = new global::Windows.Foundation.Point(0.5, 0.5) };
        if (back)
            icon.RenderTransform = new ScaleTransform { ScaleX = -1 };
        var grid = new Grid();
        grid.Children.Add(icon);
        grid.Children.Add(new TextBlock { Text = "10", Style = Ui.Res<Style>("SkipDigitsStyle") });
        return grid;
    }

    /// <summary>A text button 32 high, corner 4, 14 px (the board's wbtn); the accent one in amber.</summary>
    public static Button Text(string text, bool accent = false)
    {
        var button = new Button
        {
            Content = text,
            MinHeight = 32,
            Padding = new Thickness(12, 0, 12, 0),
            CornerRadius = new CornerRadius(4),
            FontSize = 14,
            VerticalAlignment = VerticalAlignment.Center,
        };
        if (accent)
        {
            button.Style = Ui.Res<Style>("AccentButtonStyle");
            button.FontWeight = Microsoft.UI.Text.FontWeights.SemiBold;
        }
        return button;
    }

    public static void Name(FrameworkElement element, string name)
    {
        ToolTipService.SetToolTip(element, name);
        AutomationProperties.SetName(element, name);
    }

    /// <summary>
    /// The show's wide art, 16:9, from the cast's own picture or the catalogue's banner, in its
    /// placeholder until it arrives. One element kept per place, its picture changed in place, so
    /// the connected animation always has the same element to fly from and to.
    /// </summary>
    public sealed class Art
    {
        readonly Image image = new() { Stretch = Stretch.UniformToFill };
        string? bound;

        public Art(double width, double height, double radius)
        {
            AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
            Root = new Border
            {
                Width = width,
                Height = height,
                CornerRadius = new CornerRadius(radius),
                Background = Ui.Res<Brush>("RaisedBrush"),
                Child = image,
            };
            ArtFade.Watch(image);
        }

        public Border Root { get; }

        public void Show(Ctx ctx, uint? id, string? picture)
        {
            var entry = id is { } known ? ctx.Known.Entry(known) : null;
            var url = picture is { Length: > 0 } ? picture : entry?.BannerUrl ?? entry?.CoverUrl;
            if (url == bound)
                return;
            bound = url;
            image.Source = null;
            if (url is not null)
                ctx.Art.Bind(url, art =>
                {
                    if (bound == url)
                        image.Source = art;
                });
        }
    }

    /// <summary>
    /// The status line: the water dot while it plays or is paused, vermilion once it failed, a
    /// small ring while it starts or joins again. The words change in place, said once each time
    /// the state changes, not as a count moves.
    /// </summary>
    public sealed class StatusLine
    {
        readonly Ellipse dot = new() { Width = 7, Height = 7, VerticalAlignment = VerticalAlignment.Center };
        readonly ProgressRing ring = new() { Width = 12, Height = 12, MinWidth = 12, MinHeight = 12, IsActive = false, Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center };
        readonly TextBlock text = new() { VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
        CastLine? said;

        public StatusLine(double size, bool haze)
        {
            text.FontSize = size;
            if (haze)
                text.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
            Root.Children.Add(dot);
            Root.Children.Add(ring);
            Root.Children.Add(text);
            AutomationProperties.SetAccessibilityView(dot, AccessibilityView.Raw);
            AutomationProperties.SetLiveSetting(text, AutomationLiveSetting.Polite);
        }

        public StackPanel Root { get; } = new() { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };

        public void Show(CastLine line, string words)
        {
            var spinning = line is CastLine.Starting or CastLine.Rejoining;
            if (ring.IsActive != spinning)
            {
                ring.IsActive = spinning;
                ring.Visibility = spinning ? Visibility.Visible : Visibility.Collapsed;
                dot.Visibility = spinning ? Visibility.Collapsed : Visibility.Visible;
            }
            if (!spinning)
            {
                var colour = line == CastLine.Failed ? Ui.Vermilion : Colours.Water;
                if (dot.Fill is not SolidColorBrush { } brush || brush.Color != colour)
                    dot.Fill = new SolidColorBrush(colour);
            }
            if (text.Text != words)
                text.Text = words;
            if (said != line)
            {
                if (said is not null)
                    FrameworkElementAutomationPeer.FromElement(text)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
                said = line;
            }
        }
    }
}
