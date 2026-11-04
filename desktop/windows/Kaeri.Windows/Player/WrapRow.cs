using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Player;

/// <summary>A row that wraps onto the next line when its children do not fit, with one gap between them both ways.</summary>
public sealed class WrapRow : Panel
{
    public double Gap { get; set; } = 8;

    protected override global::Windows.Foundation.Size MeasureOverride(global::Windows.Foundation.Size available)
    {
        double x = 0, y = 0, line = 0, widest = 0;
        foreach (var child in Children)
        {
            child.Measure(new global::Windows.Foundation.Size(available.Width, available.Height));
            if (child.Visibility == Visibility.Collapsed)
                continue;
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > available.Width)
            {
                y += line + Gap;
                x = 0;
                line = 0;
            }
            x += size.Width + Gap;
            line = Math.Max(line, size.Height);
            widest = Math.Max(widest, x - Gap);
        }
        return new global::Windows.Foundation.Size(widest, y + line);
    }

    protected override global::Windows.Foundation.Size ArrangeOverride(global::Windows.Foundation.Size final)
    {
        double x = 0, y = 0, line = 0;
        foreach (var child in Children)
        {
            if (child.Visibility == Visibility.Collapsed)
                continue;
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > final.Width)
            {
                y += line + Gap;
                x = 0;
                line = 0;
            }
            child.Arrange(new global::Windows.Foundation.Rect(x, y, size.Width, size.Height));
            x += size.Width + Gap;
            line = Math.Max(line, size.Height);
        }
        return final;
    }
}
