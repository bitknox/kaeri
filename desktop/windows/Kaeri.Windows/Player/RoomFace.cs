using Kaeri.App;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;

namespace Kaeri.Windows.Player;

/// <summary>
/// A person in the room: their initial on a disc, and one of three rings around it (the board's
/// faces). Solid water is with the room, solid haze is paused, an arc filling toward the 20 s
/// cap is the room waiting for them, and no ring with the disc dimmed is not with the room. The
/// one who paused it carries a pause badge; no source carries the vermilion dot. The word always
/// stands beside or under the face, so colour never says anything alone.
/// </summary>
sealed class RoomFace : Grid
{
    readonly double disc, stroke, gap;
    readonly Ellipse ring = new();
    readonly Ellipse track = new();
    readonly Microsoft.UI.Xaml.Shapes.Path arc = new() { StrokeStartLineCap = PenLineCap.Round, StrokeEndLineCap = PenLineCap.Round };
    readonly Border inner;
    readonly TextBlock letter = new() { HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold };
    readonly Border badge;
    readonly Ellipse error;
    RoomFaceLook? look;

    /// <summary>A face whose disc is <paramref name="d"/> epx across; the ring and its gap add to it.</summary>
    public RoomFace(double d)
    {
        disc = d;
        stroke = d >= 48 ? 3 : d >= 30 ? 2 : 1.75;
        gap = d >= 48 ? 3 : 2;
        var size = d + 2 * (stroke + gap);
        Width = Height = size;
        IsHitTestVisible = false;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);

        foreach (var e in new[] { ring, track })
        {
            e.StrokeThickness = stroke;
            e.Width = e.Height = size;
        }
        track.Stroke = (Brush)Application.Current.Resources["RuleBrush"];
        arc.StrokeThickness = stroke;
        arc.Stroke = (Brush)Application.Current.Resources["SilkBrush"];
        Children.Add(ring);
        Children.Add(track);
        Children.Add(arc);

        letter.FontSize = Math.Round(d * 0.42);
        letter.Foreground = (Brush)Application.Current.Resources["SilkBrush"];
        inner = new Border
        {
            Width = d,
            Height = d,
            CornerRadius = new CornerRadius(d / 2),
            Background = (Brush)Application.Current.Resources["SelectedFillBrush"],
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Child = letter,
        };
        Children.Add(inner);

        var bz = Math.Max(12, Math.Round(d * 0.42));
        badge = new Border
        {
            Width = bz,
            Height = bz,
            CornerRadius = new CornerRadius(bz / 2),
            Background = (Brush)Application.Current.Resources["GroundBrush"],
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, -1, -1),
            Child = new FontIcon { Glyph = "", FontSize = Math.Max(7, bz - 6), Foreground = (Brush)Application.Current.Resources["SilkBrush"] },
            Visibility = Visibility.Collapsed,
        };
        Children.Add(badge);

        var ez = Math.Max(8, Math.Round(d * 0.28));
        error = new Ellipse
        {
            Width = ez,
            Height = ez,
            Fill = new SolidColorBrush(Pages.Ui.Vermilion),
            Stroke = (Brush)Application.Current.Resources["GroundBrush"],
            StrokeThickness = 2,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, stroke - 1, stroke - 1),
            Visibility = Visibility.Collapsed,
        };
        Children.Add(error);
    }

    /// <summary>The "+N" face that stands for everyone past the first three.</summary>
    public static RoomFace More(double d, string label)
    {
        var face = new RoomFace(d);
        face.ring.Visibility = face.track.Visibility = face.arc.Visibility = Visibility.Collapsed;
        face.letter.Text = label;
        face.letter.FontSize = Math.Round(d * 0.36);
        face.letter.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
        face.inner.Background = (Brush)Application.Current.Resources["RaisedBrush"];
        face.inner.BorderBrush = (Brush)Application.Current.Resources["RuleBrush"];
        face.inner.BorderThickness = new Thickness(1);
        return face;
    }

    /// <summary>Shows a person as they are now. The dim changes on fade-in; the arc steps once a second.</summary>
    public void Show(string name, RoomFaceLook now)
    {
        var initial = Rooms.Initial(name);
        if (letter.Text != initial)
            letter.Text = initial;
        if (look == now)
            return;
        var dimChanged = look is null || look.Dim != now.Dim;
        look = now;
        ring.Visibility = now.Ring is RingKind.Water or RingKind.Haze ? Visibility.Visible : Visibility.Collapsed;
        ring.Stroke = (Brush)Application.Current.Resources[now.Ring == RingKind.Haze ? "HazeBrush" : "WaterBrush"];
        var progress = now.Ring == RingKind.Progress;
        track.Visibility = arc.Visibility = progress ? Visibility.Visible : Visibility.Collapsed;
        if (progress)
            arc.Data = Arc(now.Fraction);
        badge.Visibility = now.PauseBadge ? Visibility.Visible : Visibility.Collapsed;
        error.Visibility = now.ErrorDot ? Visibility.Visible : Visibility.Collapsed;
        if (dimChanged)
        {
            if (inner.Opacity != now.Dim && IsLoaded)
                Motion.FadeTo(inner, now.Dim, Motion.FadeInTime);
            else
                inner.Opacity = now.Dim;
        }
    }

    /// <summary>The arc from twelve o'clock, clockwise, <paramref name="fraction"/> of the way round.</summary>
    Geometry Arc(double fraction)
    {
        var size = Width;
        var r = size / 2 - stroke / 2;
        var c = size / 2;
        var f = Math.Clamp(fraction, 0.04, 1);
        if (f >= 0.999)
            return new EllipseGeometry { Center = new Point(c, c), RadiusX = r, RadiusY = r };
        var angle = f * 2 * Math.PI;
        var end = new Point(c + r * Math.Sin(angle), c - r * Math.Cos(angle));
        var figure = new PathFigure { StartPoint = new Point(c, c - r), IsClosed = false };
        figure.Segments.Add(new ArcSegment
        {
            Point = end,
            Size = new Size(r, r),
            SweepDirection = SweepDirection.Clockwise,
            IsLargeArc = f > 0.5,
        });
        var geometry = new PathGeometry();
        geometry.Figures.Add(figure);
        return geometry;
    }
}
