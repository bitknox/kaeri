using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Size = global::Windows.Foundation.Size;
using Rect = global::Windows.Foundation.Rect;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Reading Search's bar: the scope, the filters, then the rule and Your sources only. Parts that do
/// not fit move to the next line whole, centred on their line, so no filter is ever cut off at the
/// window's edge. The rule only shows between two parts on one line.
/// </summary>
sealed partial class FilterBar : Panel
{
    public double Gap { get; set; } = 8;
    public double LineGap { get; set; } = 8;

    /// <summary>The thin rule before Your sources only, left out where a line begins or ends.</summary>
    public UIElement? Rule { get; set; }

    protected override Size MeasureOverride(Size available)
    {
        foreach (var child in Children)
            child.Measure(new Size(available.Width, available.Height));
        var (places, size) = Lay(available.Width);
        _ = places;
        return size;
    }

    protected override Size ArrangeOverride(Size final)
    {
        var (places, _) = Lay(final.Width);
        for (var i = 0; i < Children.Count; i++)
            Children[i].Arrange(places[i]);
        return final;
    }

    (Rect[] Places, Size Size) Lay(double width)
    {
        var shown = Children.Where(c => c.Visibility == Visibility.Visible).ToList();
        var places = new Rect[Children.Count];
        // Which line each part sits on, and where along it.
        var lines = new List<List<(int Index, double X)>> { new() };
        double x = 0, widest = 0;
        for (var n = 0; n < shown.Count; n++)
        {
            var child = shown[n];
            var size = child.DesiredSize;
            if (child == Rule)
            {
                // The rule joins its line only when the part after it fits there too.
                var next = n + 1 < shown.Count ? shown[n + 1].DesiredSize.Width : 0;
                if (x == 0 || x + size.Width + Gap + next > width)
                    continue;
            }
            else if (x > 0 && x + size.Width > width)
            {
                lines.Add([]);
                x = 0;
            }
            lines[^1].Add((Children.IndexOf(child), x));
            x += size.Width + Gap;
            widest = Math.Max(widest, x - Gap);
        }
        double y = 0;
        foreach (var line in lines.Where(l => l.Count > 0))
        {
            var height = line.Max(p => Children[p.Index].DesiredSize.Height);
            foreach (var (index, at) in line)
            {
                var size = Children[index].DesiredSize;
                places[index] = new Rect(at, y + (height - size.Height) / 2, size.Width, size.Height);
            }
            y += height + LineGap;
        }
        return (places, new Size(Math.Min(widest, width), Math.Max(0, y - LineGap)));
    }
}
