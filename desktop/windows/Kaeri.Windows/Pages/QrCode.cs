using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A QR code from the engine's <c>qr_matrix</c>, drawn as ground modules on a silk plate with the
/// quiet zone all round, in both appearances and never amber, so a phone's camera reads it from
/// either. One path of squares, scaled to <c>Side</c>.
/// </summary>
static class QrCode
{
    public static Border Make(string text, double side = 160)
    {
        var box = new Border();
        var matrix = KaeriFfiMethods.QrMatrix(text);
        box.Width = box.Height = side;
        box.CornerRadius = new CornerRadius(8);
        box.Background = new SolidColorBrush(ColorHelper.FromArgb(255, 0xF4, 0xF0, 0xE6));
        box.HorizontalAlignment = HorizontalAlignment.Left;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, T.StoresQrLabel);
        if (matrix.Size == 0)
            return box;
        var span = matrix.Size + 2 * matrix.QuietZone;
        var group = new GeometryGroup { FillRule = FillRule.Nonzero };
        for (uint y = 0; y < matrix.Size; y++)
            for (uint x = 0; x < matrix.Size; x++)
                if (matrix.Modules[y * matrix.Size + x])
                    group.Children.Add(new RectangleGeometry { Rect = new global::Windows.Foundation.Rect(x + matrix.QuietZone, y + matrix.QuietZone, 1, 1) });
        var path = new Microsoft.UI.Xaml.Shapes.Path
        {
            Data = group,
            // Ground: the dark of the app, in either appearance.
            Fill = new SolidColorBrush(ColorHelper.FromArgb(255, 0x14, 0x14, 0x18)),
            Stretch = Stretch.Fill,
            Width = span,
            Height = span,
        };
        // The path's box starts at the first dark module; a canvas keeps the quiet zone round it.
        var canvas = new Canvas { Width = span, Height = span };
        canvas.Children.Add(path);
        Canvas.SetLeft(path, 0);
        Canvas.SetTop(path, 0);
        path.Stretch = Stretch.None;
        box.Child = new Viewbox { Child = canvas, Stretch = Stretch.Uniform };
        return box;
    }
}
