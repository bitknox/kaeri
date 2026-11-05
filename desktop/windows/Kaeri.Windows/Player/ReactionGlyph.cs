using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Player;

/// <summary>
/// The four reactions as one drawn set, the same on every platform: monochrome strokes on a
/// 24-unit grid, drawn in the text colour at the size asked for. Segoe Fluent Icons has no clap
/// or crying face to match the others, so the set is drawn rather than taken from the font.
/// </summary>
static class ReactionGlyph
{
    /// <summary>Windows' stroke weight on the 24-unit grid, as the board draws it.</summary>
    const double Weight = 1.5;

    public static FrameworkElement Make(Reaction reaction, double size, Brush? brush = null)
    {
        var canvas = new Canvas { Width = 24, Height = 24 };
        brush ??= (Brush)Application.Current.Resources["SilkBrush"];
        void Stroke(Shape shape)
        {
            shape.Stroke = brush;
            shape.StrokeThickness = Weight;
            shape.StrokeStartLineCap = shape.StrokeEndLineCap = PenLineCap.Round;
            shape.StrokeLineJoin = PenLineJoin.Round;
            canvas.Children.Add(shape);
        }
        void Circle(double x, double y, double r, bool filled = false)
        {
            var dot = new Ellipse { Width = 2 * r, Height = 2 * r };
            Canvas.SetLeft(dot, x - r);
            Canvas.SetTop(dot, y - r);
            if (filled)
            {
                dot.Fill = brush;
                canvas.Children.Add(dot);
            }
            else
                Stroke(dot);
        }
        void Line(string data, bool filled = false)
        {
            var path = new Microsoft.UI.Xaml.Shapes.Path { Data = (Geometry)XamlBindingHelper.ConvertValue(typeof(Geometry), data) };
            if (filled)
            {
                path.Fill = brush;
                canvas.Children.Add(path);
            }
            else
                Stroke(path);
        }
        void Hand(double x, double angle, double centreX)
        {
            var palm = new Rectangle { Width = 5.8, Height = 12, RadiusX = 2.9, RadiusY = 2.9 };
            Canvas.SetLeft(palm, x);
            Canvas.SetTop(palm, 8.6);
            palm.RenderTransform = new RotateTransform { Angle = angle, CenterX = centreX - x, CenterY = 14.6 - 8.6 };
            Stroke(palm);
        }
        switch (reaction)
        {
            case Reaction.Laugh:
                Circle(12, 12, 9);
                Line("M8,10.3 Q9.5,8.4 11,10.3 M13,10.3 Q14.5,8.4 16,10.3");
                Line("M7.6,13.4 H16.4 A4.4,4.4 0 0 1 7.6,13.4 Z");
                break;
            case Reaction.Gasp:
                Circle(12, 12, 9);
                Circle(9, 10, 1, filled: true);
                Circle(15, 10, 1, filled: true);
                Circle(12, 15.3, 2.2);
                break;
            case Reaction.Clap:
                Hand(5.6, -16, 8.5);
                Hand(12.6, 16, 15.5);
                Line("M12,2.6 V4.9 M7.4,3.8 L8.6,5.7 M16.6,3.8 L15.4,5.7");
                break;
            default:
                Circle(12, 12, 9);
                Line("M8,10 H10.5 M13.5,10 H16");
                Line("M9,16.6 Q12,14.3 15,16.6");
                Line("M16.2,11.7 C16.2,11.7 15,13.5 15,14.3 A1.2,1.2 0 0 0 17.4,14.3 C17.4,13.5 16.2,11.7 16.2,11.7 Z", filled: true);
                break;
        }
        var box = new Viewbox { Width = size, Height = size, Child = canvas, IsHitTestVisible = false };
        AutomationProperties.SetAccessibilityView(box, AccessibilityView.Raw);
        return box;
    }
}
