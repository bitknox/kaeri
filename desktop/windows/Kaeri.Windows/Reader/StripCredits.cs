using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A strip's credit pages: at a chapter's start a 44 px row, "1 credit page skipped" with an
/// underlined Show, which grows into the pages on the standard token as they fade in on the
/// fade-in token (with reduced motion they are there at once and only fade); at its end the
/// pages once the end page's line showed them. Above each page, "Skipped as a credit page" and
/// Not a credit page. A page whose file can't be read is left out.
/// </summary>
sealed class StripCredits : Grid
{
    public const double RowHeight = 44;
    const double CaptionHeight = 44;

    readonly ReaderOverlay owner;
    readonly bool start;
    readonly StackPanel content = new() { VerticalAlignment = VerticalAlignment.Top };
    readonly StackPanel pictures = new();
    readonly Grid row = new() { Height = RowHeight };
    readonly Button? line;
    readonly List<(CreditPage Page, FrameworkElement Frame, PageImageView Image)> pages = [];
    readonly List<(ulong Fingerprint, TextBlock Words, Button Not)> captions = [];
    double reveal;
    bool opening;
    double width = 720;

    public ulong[] Prints { get; }
    public bool Revealed => opening || reveal >= 1;

    StripCredits(ReaderOverlay owner, IReadOnlyList<CreditPage> shown, bool start)
    {
        this.owner = owner;
        this.start = start;
        Prints = [.. shown.Select(p => p.Fingerprint)];
        var palette = owner.PagePalette;
        reveal = start ? 0 : 1;
        if (start)
        {
            line = Line(palette, shown.Count);
            row.Children.Add(line);
            var caption = Caption(palette, shown[0].Fingerprint);
            caption.Opacity = 0;
            caption.Visibility = Visibility.Collapsed;
            row.Children.Add(caption);
            content.Children.Add(row);
        }
        for (var i = 0; i < shown.Count; i++)
        {
            if (i > 0 || !start)
                pictures.Children.Add(Caption(palette, shown[i].Fingerprint));
            var image = new PageImageView { FitWidth = true, Dim = palette.Paper, Cut = true };
            var frame = new Border { Child = image };
            AutomationProperties.SetName(frame, T.ReaderCreditPageLabel);
            image.Show(shown[i].File, owner.Scale);
            pictures.Children.Add(frame);
            pages.Add((shown[i], frame, image));
        }
        if (start)
            pictures.Opacity = 0;
        content.Children.Add(pictures);
        Children.Add(content);
        RefreshHeld();
    }

    /// <summary>The block for <paramref name="credits"/> on one side, or null when no page's file is there.</summary>
    public static StripCredits? Build(ReaderOverlay owner, IEnumerable<CreditPage> credits, bool start)
    {
        var shown = credits.Where(c => System.IO.File.Exists(c.File.Path)).ToList();
        return shown.Count == 0 ? null : new StripCredits(owner, shown, start);
    }

    Button Line(ReaderPalette palette, int count)
    {
        var skipped = T.ReaderCreditPagesSkipped(count);
        var words = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center };
        words.Children.Add(new TextBlock { Text = skipped, FontSize = 13, Foreground = new SolidColorBrush(palette.Haze), VerticalAlignment = VerticalAlignment.Center });
        var show = new TextBlock { FontSize = 13, Foreground = new SolidColorBrush(palette.Silk), VerticalAlignment = VerticalAlignment.Center };
        var underline = new Microsoft.UI.Xaml.Documents.Underline();
        underline.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = T.ReaderCreditShow });
        show.Inlines.Add(underline);
        words.Children.Add(show);
        var button = new Button
        {
            Content = words,
            Height = RowHeight,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
        };
        button.Resources["ButtonBackgroundPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.06));
        button.Resources["ButtonBackgroundPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.04));
        button.Resources["ButtonBorderBrushPointerOver"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        AutomationProperties.SetName(button, skipped);
        AutomationProperties.SetHelpText(button, T.ReaderCreditShowHint);
        button.Click += (_, _) => owner.ShowCredits(false);
        return button;
    }

    /// <summary>"Skipped as a credit page · Not a credit page" over a page, 44 high.</summary>
    FrameworkElement Caption(ReaderPalette palette, ulong fingerprint)
    {
        var words = new TextBlock { Text = T.ReaderCreditShown, FontSize = 13, Foreground = new SolidColorBrush(palette.Haze), VerticalAlignment = VerticalAlignment.Center };
        var not = new Button
        {
            Content = T.ReaderCreditNot,
            FontSize = 13,
            MinHeight = 32,
            Padding = new Thickness(12, 0, 12, 0),
            CornerRadius = new CornerRadius(4),
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(palette.High),
            Foreground = new SolidColorBrush(palette.Silk),
            BorderThickness = new Thickness(0),
        };
        not.Click += (_, _) => owner.Reader.Keep(fingerprint);
        var caption = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Height = CaptionHeight, HorizontalAlignment = HorizontalAlignment.Center };
        caption.Children.Add(words);
        caption.Children.Add(not);
        captions.Add((fingerprint, words, not));
        return caption;
    }

    /// <summary>A page kept with Undo still offered says so, and offers no second keep.</summary>
    public void RefreshHeld()
    {
        foreach (var (print, words, not) in captions)
        {
            var held = owner.Reader.Held(print);
            words.Text = held ? T.ReaderCreditKept : T.ReaderCreditShown;
            not.Visibility = held ? Visibility.Collapsed : Visibility.Visible;
        }
    }

    /// <summary>The strip ran on past this chapter: its pages stay where they are, with nothing more to keep.</summary>
    public void Settle()
    {
        foreach (var (_, _, not) in captions)
            not.Visibility = Visibility.Collapsed;
        if (line is not null)
            line.IsEnabled = false;
    }

    /// <summary>The pages' full height at the column's width.</summary>
    double Full
    {
        get
        {
            double h = start ? RowHeight : 0;
            for (var i = 0; i < pages.Count; i++)
            {
                if (i > 0 || !start)
                    h += CaptionHeight;
                var file = pages[i].Page.File;
                double w = file.Crop?.Width ?? file.Width, fh = file.Crop?.Height ?? file.Height;
                h += w > 0 ? width * fh / w : width * 1.42;
            }
            return h;
        }
    }

    /// <summary>The height the block takes in the strip now: the row, the pages, or the way between.</summary>
    public double Shown => start ? RowHeight + reveal * (Full - RowHeight) : Full;

    /// <summary>The pages at the column's width.</summary>
    public void Size(double columnWidth)
    {
        width = columnWidth;
        Width = columnWidth;
        foreach (var (page, frame, image) in pages)
        {
            var file = page.File;
            double w = file.Crop?.Width ?? file.Width, h = file.Crop?.Height ?? file.Height;
            image.Width = columnWidth;
            image.Height = w > 0 ? columnWidth * h / w : columnWidth * 1.42;
            frame.Height = image.Height;
        }
        Fit();
    }

    void Fit()
    {
        Height = Shown;
        Clip = new RectangleGeometry { Rect = new global::Windows.Foundation.Rect(0, 0, width, Height) };
    }

    /// <summary>
    /// The row gives way to the first page's caption by a crossfade, and the block grows to the
    /// pages on the standard token, the strip moving with it; the pages fade in as it grows.
    /// </summary>
    public void Reveal(bool animate, Action moved)
    {
        if (Revealed)
            return;
        opening = true;
        if (line is not null && row.Children.Count > 1 && row.Children[1] is FrameworkElement caption)
        {
            caption.Visibility = Visibility.Visible;
            line.IsHitTestVisible = false;
            AutomationProperties.SetAccessibilityView(line, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
            if (animate)
                Motion.Crossfade(line, caption);
            else
            {
                line.Opacity = 0;
                caption.Opacity = 1;
            }
        }
        if (!animate || owner.PaperMode)
        {
            reveal = 1;
            pictures.Opacity = 1;
            Fit();
            moved();
            return;
        }
        Motion.FadeIn(pictures, Motion.FadeInTime);
        Motion.Ease(this, reveal, 1, v =>
        {
            reveal = v;
            Fit();
            moved();
        });
    }
}
