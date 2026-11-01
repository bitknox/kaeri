using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The subtitle style drawn over a 16:9 frame, bright sky on the left and night on the right, so
/// poor contrast shows on either. Plain draws two dialogue lines at the bottom; Styled adds a
/// typeset sign at the top, drawn as the Keep, Restyle or Strip choice would leave it. It
/// redraws at once, with no animation. It stands in for libass: the player draws the real thing.
/// </summary>
sealed class SubtitlePreview : Grid
{
    public const double FrameWidth = 320, FrameHeight = 180;

    readonly Grid layer = new();
    SubtitleStyle style = new(100, SubtitleFont.Default, SubtitleColour.White, SubtitleEdge.Outline, SubtitleThickness.Medium, SubtitleBackground.NoBackground, 100, 0, StyledSubtitles.Keep, true);
    bool styled;
    SystemCaptionStyle? system;

    public SubtitlePreview()
    {
        Width = FrameWidth;
        Height = FrameHeight;
        HorizontalAlignment = HorizontalAlignment.Left;
        CornerRadius = new CornerRadius(8);
        var frame = new Grid { CornerRadius = new CornerRadius(8) };
        frame.ColumnDefinitions.Add(new ColumnDefinition());
        frame.ColumnDefinitions.Add(new ColumnDefinition());
        var sky = new Border
        {
            Background = new LinearGradientBrush
            {
                StartPoint = new global::Windows.Foundation.Point(0, 0),
                EndPoint = new global::Windows.Foundation.Point(0, 1),
                GradientStops =
                {
                    new GradientStop { Color = ColorHelper.FromArgb(255, 0x9C, 0xD4, 0xF2), Offset = 0 },
                    new GradientStop { Color = ColorHelper.FromArgb(255, 0xE8, 0xF4, 0xFB), Offset = 1 },
                },
            },
        };
        var night = new Border
        {
            Background = new LinearGradientBrush
            {
                StartPoint = new global::Windows.Foundation.Point(0, 0),
                EndPoint = new global::Windows.Foundation.Point(0, 1),
                GradientStops =
                {
                    new GradientStop { Color = ColorHelper.FromArgb(255, 0x0B, 0x10, 0x2E), Offset = 0 },
                    new GradientStop { Color = ColorHelper.FromArgb(255, 0x1B, 0x23, 0x4D), Offset = 1 },
                },
            },
        };
        SetColumn(night, 1);
        frame.Children.Add(sky);
        frame.Children.Add(night);
        Children.Add(frame);
        Children.Add(layer);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(this, T.SettingsSubsPreview);
    }

    /// <summary>Draws <paramref name="now"/>; <paramref name="plain"/> picks the Plain tab over Styled.</summary>
    public void Draw(SubtitleStyle now, bool plain, SystemCaptionStyle? systemStyle)
    {
        style = now;
        styled = !plain;
        system = style.FollowSystem ? systemStyle : null;
        layer.Children.Clear();
        var scale = style.Size / 100.0 * (system?.RelativeSize ?? 1.0);
        var size = Math.Clamp(15.0 * scale, 6, 40);
        if (styled)
            AddSign(size);
        var restyle = !styled || style.Styled != StyledSubtitles.Keep;
        var lines = new StackPanel
        {
            VerticalAlignment = VerticalAlignment.Bottom,
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, 0, 0, 10 + style.Raise / 100.0 * FrameHeight),
            Spacing = 2,
        };
        // Keep leaves a styled line to its author: a plain white face with its own edge.
        var look = restyle ? Look.Of(style, system) : Look.Authors;
        if (look.Plate == PlateKind.All)
        {
            var plate = new Border
            {
                Background = new SolidColorBrush(look.PlateColour),
                Padding = new Thickness(6, 2, 6, 2),
                CornerRadius = new CornerRadius(2),
                Child = Lines(size, look, false),
            };
            lines.Children.Add(plate);
        }
        else
            lines.Children.Add(Lines(size, look, look.Plate == PlateKind.Each));
        layer.Children.Add(lines);
    }

    StackPanel Lines(double size, Look look, bool eachPlate)
    {
        var stack = new StackPanel { Spacing = 2, HorizontalAlignment = HorizontalAlignment.Center };
        foreach (var text in new[] { T.SettingsSubsPreviewLine, T.SettingsSubsPreviewLine2 })
        {
            FrameworkElement line = Line(text, size, look);
            if (eachPlate)
                line = new Border
                {
                    Background = new SolidColorBrush(look.PlateColour),
                    Padding = new Thickness(4, 0, 4, 0),
                    CornerRadius = new CornerRadius(2),
                    HorizontalAlignment = HorizontalAlignment.Center,
                    Child = line,
                };
            stack.Children.Add(line);
        }
        return stack;
    }

    void AddSign(double size)
    {
        // A typeset sign at the top: Keep leaves it as its author drew it, Restyle gives it the
        // viewer's face and colour, and Strip takes it out of the frame and into the dialogue.
        if (style.Styled == StyledSubtitles.Strip)
            return;
        var keep = style.Styled == StyledSubtitles.Keep;
        var look = keep ? Look.Sign : Look.Of(style, system);
        var sign = Line(T.SettingsSubsPreviewSign, keep ? 17 : size, look);
        sign.HorizontalAlignment = HorizontalAlignment.Center;
        sign.VerticalAlignment = VerticalAlignment.Top;
        sign.Margin = new Thickness(0, keep ? 14 : 10, 0, 0);
        layer.Children.Add(sign);
    }

    /// <summary>One line with its edge: the text over copies of itself in the edge colour.</summary>
    static FrameworkElement Line(string text, double size, Look look)
    {
        var grid = new Grid { HorizontalAlignment = HorizontalAlignment.Center };
        TextBlock Block(global::Windows.UI.Color colour, double dx, double dy) => new()
        {
            Text = text,
            FontSize = size,
            FontWeight = look.Bold ? FontWeights.SemiBold : FontWeights.Normal,
            FontFamily = look.Family is { } f ? new FontFamily(f) : new FontFamily("Segoe UI"),
            Foreground = new SolidColorBrush(colour),
            RenderTransform = new TranslateTransform { X = dx, Y = dy },
            TextWrapping = TextWrapping.NoWrap,
            IsHitTestVisible = false,
        };
        if (look.Outline > 0)
        {
            var o = look.Outline;
            foreach (var (dx, dy) in new[] { (-o, 0.0), (o, 0), (0, -o), (0, o), (-o, -o), (o, o), (-o, o), (o, -o) })
                grid.Children.Add(Block(look.EdgeColour, dx, dy));
        }
        if (look.Shadow > 0)
            grid.Children.Add(Block(look.EdgeColour, look.Shadow, look.Shadow));
        grid.Children.Add(Block(look.Text, 0, 0));
        return grid;
    }

    enum PlateKind { None, Each, All }

    /// <summary>What the preview draws a line with.</summary>
    sealed record Look(
        string? Family,
        global::Windows.UI.Color Text,
        double Outline,
        double Shadow,
        global::Windows.UI.Color EdgeColour,
        PlateKind Plate,
        global::Windows.UI.Color PlateColour,
        bool Bold)
    {
        public static readonly Look Authors = new(null, Colors.White, 1.2, 0, Colors.Black, PlateKind.None, Colors.Black, false);
        public static readonly Look Sign = new("Georgia", ColorHelper.FromArgb(255, 0xFF, 0xD7, 0x7A), 1.2, 0, ColorHelper.FromArgb(255, 0x5A, 0x1E, 0x2E), PlateKind.None, Colors.Black, true);

        public static Look Of(SubtitleStyle style, SystemCaptionStyle? system)
        {
            var thin = style.Thickness switch { SubtitleThickness.Thin => 1.0, SubtitleThickness.Medium => 1.4, _ => 2.0 };
            var colour = style.Colour switch
            {
                SubtitleColour.Yellow => Colors.Yellow,
                SubtitleColour.Cyan => Colors.Cyan,
                SubtitleColour.Green => Colors.Lime,
                _ => Colors.White,
            };
            var plateAlpha = (byte)(style.Opacity switch { 50 => 0x80, 75 => 0xBF, _ => 0xFF });
            var family = style.Font switch
            {
                SubtitleFont.ZenKakuGothicNew => "ms-appx:///Assets/Fonts/ZenKakuGothicNew-Bold.ttf#Zen Kaku Gothic New",
                SubtitleFont.Serif => "Georgia",
                SubtitleFont.Monospace => "Consolas",
                _ => null,
            };
            var outline = style.Edge == SubtitleEdge.Outline ? thin : 0;
            var shadow = style.Edge == SubtitleEdge.DropShadow ? thin + 0.6 : 0;
            var plate = style.Background switch
            {
                SubtitleBackground.EachLine => PlateKind.Each,
                SubtitleBackground.AllLines => PlateKind.All,
                _ => PlateKind.None,
            };
            if (plate == PlateKind.Each)
                outline = shadow = 0;
            var plateColour = ColorHelper.FromArgb(plateAlpha, 0, 0, 0);
            var edge = ColorHelper.FromArgb(0xFF, 0, 0, 0);
            if (system is not null)
            {
                family = system.FontFamily ?? family;
                colour = system.TextRgba is { } t ? Rgba(t) : colour;
                if (system.Edge is { } e)
                {
                    outline = e == CaptionEdge.Uniform ? 1.4 : 0;
                    shadow = e is CaptionEdge.DropShadow or CaptionEdge.Raised or CaptionEdge.Depressed ? 1.6 : 0;
                }
                if (system.BackgroundRgba is { } b && (b & 0xFF) > 0)
                {
                    plate = PlateKind.Each;
                    plateColour = Rgba(b);
                    outline = shadow = 0;
                }
                else if (system.WindowRgba is { } w && (w & 0xFF) > 0)
                {
                    plate = PlateKind.All;
                    plateColour = Rgba(w);
                    outline = shadow = 0;
                }
            }
            return new Look(family, colour, outline, shadow, edge, plate, plateColour, false);
        }

        static global::Windows.UI.Color Rgba(uint c) => ColorHelper.FromArgb((byte)c, (byte)(c >> 24), (byte)(c >> 16), (byte)(c >> 8));
    }
}
