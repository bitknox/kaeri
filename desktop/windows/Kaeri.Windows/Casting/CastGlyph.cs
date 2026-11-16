using Kaeri.App;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Shapes;
using Path = Microsoft.UI.Xaml.Shapes.Path;

namespace Kaeri.Windows.Casting;

/// <summary>
/// The cast glyph as the board draws it on a 24-unit grid: the screen's corner and two waves
/// from its dot, stroked at 1.5. Outline while TVs are there, the screen filled inside its frame
/// while one is held, and while it connects the dot and the waves light in turn, 1.2 s round,
/// each 0.2 s after the one before. With animation effects off the connecting glyph holds one
/// frame: the dot lit, the waves faint.
/// </summary>
sealed class CastGlyph : Grid
{
    const double Weight = 1.5;
    readonly Path filled, wide, near;
    readonly Ellipse dot;
    readonly Storyboard waves = new();
    CastButtonState? drawn;

    public CastGlyph(double size, Brush? brush = null)
    {
        Width = Height = size;
        brush ??= (Brush)Application.Current.Resources["TextFillColorPrimaryBrush"];
        var canvas = new Canvas { Width = 24, Height = 24 };
        Path Stroked(string data)
        {
            var path = new Path
            {
                Data = (Geometry)XamlBindingHelper.ConvertValue(typeof(Geometry), data),
                Stroke = brush,
                StrokeThickness = Weight,
                StrokeStartLineCap = PenLineCap.Round,
                StrokeEndLineCap = PenLineCap.Round,
                StrokeLineJoin = PenLineJoin.Round,
            };
            canvas.Children.Add(path);
            return path;
        }
        Stroked("M3 7.4 V3.6 H21 V19.2 H13.6");
        // Inside the frame with a clear gap all round, its corner cut on a circle about the waves'
        // centre that keeps the same gap from the outer wave: the shape Android draws.
        filled = new Path
        {
            Data = (Geometry)XamlBindingHelper.ConvertValue(typeof(Geometry), "M5.7 6.3 H18.3 V16.5 H13.73 A11.5 11.5 0 0 0 5.7 8.33 Z"),
            Fill = brush,
            Visibility = Visibility.Collapsed,
        };
        canvas.Children.Add(filled);
        wide = Stroked("M3 10.4 A9 9 0 0 1 11.6 19.2");
        near = Stroked("M3 14 A5.4 5.4 0 0 1 8 19.2");
        dot = new Ellipse { Width = 3.2, Height = 3.2, Fill = brush };
        Canvas.SetLeft(dot, 3.9 - 1.6);
        Canvas.SetTop(dot, 18.3 - 1.6);
        canvas.Children.Add(dot);
        Children.Add(new Viewbox { Child = canvas });
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        Wave(dot, 0);
        Wave(near, 0.2);
        Wave(wide, 0.4);
        Unloaded += (_, _) => waves.Stop();
        Loaded += (_, _) =>
        {
            if (drawn == CastButtonState.Connecting && Motion.Enabled)
                waves.Begin();
        };
    }

    void Wave(UIElement part, double delay)
    {
        var animation = new DoubleAnimationUsingKeyFrames { BeginTime = TimeSpan.FromSeconds(delay), RepeatBehavior = RepeatBehavior.Forever };
        animation.KeyFrames.Add(new LinearDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = 0.3 });
        animation.KeyFrames.Add(new LinearDoubleKeyFrame { KeyTime = TimeSpan.FromSeconds(0.36), Value = 1 });
        animation.KeyFrames.Add(new LinearDoubleKeyFrame { KeyTime = TimeSpan.FromSeconds(1.2), Value = 0.3 });
        Storyboard.SetTarget(animation, part);
        Storyboard.SetTargetProperty(animation, "Opacity");
        waves.Children.Add(animation);
    }

    /// <summary>Draws the glyph for the button's state; the same state again changes nothing.</summary>
    public void Show(CastButtonState state)
    {
        if (drawn == state)
            return;
        drawn = state;
        // Held, the frame stays and the screen inside it fills, as the system's cast icon does.
        filled.Visibility = state == CastButtonState.Connected ? Visibility.Visible : Visibility.Collapsed;
        waves.Stop();
        if (state == CastButtonState.Connecting)
        {
            if (Motion.Enabled)
            {
                if (IsLoaded)
                    waves.Begin();
                return;
            }
            dot.Opacity = 1;
            near.Opacity = wide.Opacity = 0.35;
            return;
        }
        dot.Opacity = near.Opacity = wide.Opacity = 1;
    }
}
