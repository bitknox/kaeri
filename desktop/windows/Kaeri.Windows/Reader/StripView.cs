using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The strip: every image of the chapter in one vertical scroll, a centred column on a PC. Only
/// the images near the window are mounted. After the last image a divider names the next
/// chapter, and if the person stays at the foot it follows on its own; the end page is only for
/// the last chapter there is.
/// </summary>
sealed class StripView : Grid
{
    readonly ReaderOverlay owner;
    readonly ScrollViewer scroll;
    readonly Canvas column = new();
    readonly List<LeafPart> parts = [];
    readonly List<double> tops = [];
    readonly Dictionary<int, PartView> live = [];
    readonly DispatcherQueueTimer follow;
    FrameworkElement? foot;
    string chapter = "";
    double columnWidth = 720;
    bool restoring = true;
    bool atFoot;
    int image;

    public StripView(ReaderOverlay owner)
    {
        this.owner = owner;
        scroll = new ScrollViewer
        {
            VerticalScrollMode = ScrollMode.Enabled,
            HorizontalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Hidden,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            ZoomMode = ZoomMode.Disabled,
            IsTabStop = false,
            Content = column,
        };
        column.HorizontalAlignment = HorizontalAlignment.Center;
        Children.Add(scroll);
        scroll.ViewChanged += (_, e) => Track(e.IsIntermediate);
        scroll.AddHandler(PointerWheelChangedEvent, new PointerEventHandler((_, _) => Interacted()), true);
        scroll.AddHandler(PointerPressedEvent, new PointerEventHandler((_, _) => Interacted()), true);
        SizeChanged += (_, _) => Layout();
        follow = Timers.Weak(this, TimeSpan.FromMilliseconds(1400), false, s => s.FollowOn());
    }

    ReaderSession Reader => owner.Reader;

    double Gap => Reader.Choice(ReaderSettingKey.StripGap) == "small" ? 8 : 0;

    /// <summary>The image at the window's top, counted from 0, and how many there are.</summary>
    public (int Image, int Count) Position => (image, parts.Count);

    void Interacted()
    {
        restoring = false;
        owner.HideChrome();
    }

    // Building

    public void Rebuild()
    {
        follow.Stop();
        var newChapter = Reader.View?.Chapter.Id ?? "";
        if (newChapter != chapter)
        {
            restoring = true;
            atFoot = false;
        }
        chapter = newChapter;
        parts.Clear();
        foreach (var leaf in Reader.Leaves)
            if (!leaf.EndPage)
                parts.AddRange(leaf.Parts);
        foreach (var view in live.Values)
            column.Children.Remove(view);
        live.Clear();
        if (foot is not null)
        {
            column.Children.Remove(foot);
            foot = null;
        }
        Layout();
        if (restoring)
            Restore();
    }

    public void RefreshPages()
    {
        var before = tops.Count > 0 ? tops[^1] : 0;
        Layout();
        foreach (var view in live.Values)
            view.Refresh(owner.Scale);
        // An image that came in above the place being restored moves it: put it back.
        if (restoring && tops.Count > 0 && Math.Abs(tops[^1] - before) > 0.5)
            Restore();
    }

    /// <summary>The foot's words changed: the next chapter, or the end page.</summary>
    public void RefreshFoot()
    {
        if (foot is not null)
        {
            column.Children.Remove(foot);
            foot = null;
        }
        Layout();
    }

    double HeightOf(LeafPart part)
    {
        if (Reader.FileOf(part, chapter) is { } file)
        {
            double w = file.Crop?.Width ?? file.Width, h = file.Crop?.Height ?? file.Height;
            if (w > 0)
                return columnWidth * h / w;
        }
        if (part.Ratio is { } ratio && ratio > 0)
            return columnWidth / ratio;
        return columnWidth * 1.5;
    }

    void Layout()
    {
        if (ActualWidth <= 0)
            return;
        var factor = (Reader.Amount(ReaderSettingKey.StripWidth) ?? 0.6) / 0.6;
        columnWidth = Math.Min(ActualWidth, 720 * factor);
        tops.Clear();
        double y = 0;
        var gap = Gap;
        foreach (var part in parts)
        {
            tops.Add(y);
            y += HeightOf(part) + gap;
        }
        foot ??= BuildFoot();
        if (!column.Children.Contains(foot))
            column.Children.Add(foot);
        foot.Width = columnWidth;
        foot.Height = (Reader.End?.Next ?? Reader.View?.Next) is not null ? Math.Max(ActualHeight * 0.6, 320) : Math.Max(ActualHeight * 0.8, 420);
        Canvas.SetTop(foot, y);
        column.Width = columnWidth;
        column.Height = y + foot.Height;
        foreach (var (index, view) in live)
        {
            view.Width = columnWidth;
            view.Height = HeightOf(parts[index]);
            Canvas.SetTop(view, tops[index]);
        }
        Mount();
    }

    /// <summary>The divider and the next chapter, or the end page when there is no next.</summary>
    FrameworkElement BuildFoot()
    {
        var next = Reader.End?.Next ?? Reader.View?.Next;
        if (next is null)
            return owner.BuildEndPage();
        var palette = owner.Palette;
        var stack = new StackPanel { Spacing = 4, Padding = new Thickness(28, 0, 28, 0) };
        stack.Children.Add(new Border { Height = 1, Background = new SolidColorBrush(palette.Rule), Margin = new Thickness(0, 0, 0, 28) });
        stack.Children.Add(new TextBlock { Text = T.ReaderEndNext, FontSize = 13, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Haze) });
        stack.Children.Add(new TextBlock { Text = next.Title ?? next.Label, FontSize = 20, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Silk), TextWrapping = TextWrapping.WrapWholeWords });
        stack.Children.Add(new TextBlock { Text = next.Pages is { } pages ? T.WindowsReaderEndNextDetail(next.Label, (int)pages) : next.Label, FontSize = 15, Foreground = new SolidColorBrush(palette.Haze) });
        if (Reader.NextPreview is { } preview)
        {
            var image = new PageImageView { Height = 200, Width = 140, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 16, 0, 0), Dim = palette.Paper };
            image.Show(preview, owner.Scale);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(image, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
            stack.Children.Add(image);
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(stack, T.ReaderEndNext + ", " + (next.Title ?? next.Label));
        return stack;
    }

    /// <summary>Only the images within a couple of windows of the view are mounted.</summary>
    void Mount()
    {
        if (parts.Count == 0)
            return;
        var top = scroll.VerticalOffset - ActualHeight * 1.5;
        var bottom = scroll.VerticalOffset + ActualHeight * 2.5;
        var want = new HashSet<int>();
        for (var i = 0; i < parts.Count; i++)
        {
            var h = HeightOf(parts[i]);
            if (tops[i] + h >= top && tops[i] <= bottom)
                want.Add(i);
        }
        foreach (var gone in live.Keys.Where(k => !want.Contains(k)).ToList())
        {
            column.Children.Remove(live[gone]);
            live.Remove(gone);
        }
        var total = parts.Count;
        foreach (var index in want)
        {
            if (live.ContainsKey(index))
                continue;
            var part = parts[index];
            var view = new PartView(Reader, part, chapter, owner.Palette, HorizontalAlignment.Center, true, false,
                T.WindowsReaderPageLabel(index + 1, total))
            {
                Width = columnWidth,
                Height = HeightOf(part),
            };
            Canvas.SetTop(view, tops[index]);
            column.Children.Add(view);
            live[index] = view;
            view.Refresh(owner.Scale);
            if (Reader.FileOf(part, chapter) is null && Reader.FailureOf(part, chapter) is null)
                Reader.NeedPage(part.Page);
        }
    }

    void Restore()
    {
        if (parts.Count == 0 || tops.Count != parts.Count)
            return;
        var at = Reader.StripRestore ?? Reader.View?.Start.Strip;
        if (at is null)
        {
            scroll.ChangeView(null, 0, null, true);
            return;
        }
        var index = Math.Min((int)at.Image, parts.Count - 1);
        scroll.ChangeView(null, tops[index] + at.Offset * HeightOf(parts[index]), null, true);
    }

    void Track(bool moving)
    {
        Mount();
        if (parts.Count == 0 || tops.Count != parts.Count)
            return;
        var y = scroll.VerticalOffset;
        var footNow = y + scroll.ViewportHeight >= scroll.ExtentHeight - 40;
        var found = parts.Count - 1;
        for (var i = 0; i < parts.Count; i++)
        {
            if (y < tops[i] + HeightOf(parts[i]) + Gap)
            {
                found = i;
                break;
            }
        }
        if (found != image)
        {
            image = found;
            owner.PositionChanged();
        }
        if (!restoring && !moving)
        {
            var h = HeightOf(parts[found]);
            var into = h > 0 ? Math.Clamp((y - tops[found]) / h, 0, 1) : 0;
            Reader.StripAt(new StripPos((uint)found, (float)into), footNow);
        }
        if (footNow && !atFoot && !restoring)
            FollowSoon();
        if (!footNow)
            follow.Stop();
        atFoot = footNow;
    }

    /// <summary>The person reached the foot: a moment later, if they are still there, the next chapter follows.</summary>
    void FollowSoon()
    {
        if ((Reader.View?.Next) is null)
            return;
        follow.Stop();
        follow.Start();
    }

    void FollowOn()
    {
        if (!atFoot || scroll.VerticalOffset <= 0)
            return;
        Reader.FlowOn();
    }

    // Moving

    /// <summary>A key or a click zone: most of a window down or up, or a line with the arrows.</summary>
    public void Step(bool forward, bool line)
    {
        restoring = false;
        var step = line ? 120 : scroll.ViewportHeight * 0.9;
        scroll.ChangeView(null, Math.Max(0, scroll.VerticalOffset + (forward ? step : -step)), null, !Motion.Enabled || owner.PaperMode);
    }

    public void Ends(bool last)
    {
        restoring = false;
        scroll.ChangeView(null, last ? scroll.ScrollableHeight : 0, null, !Motion.Enabled || owner.PaperMode);
    }

    /// <summary>To the top of an image, counted from 0: the scrubber and the engine's moves.</summary>
    public void JumpTo(int index)
    {
        if (parts.Count == 0 || tops.Count != parts.Count)
            return;
        restoring = false;
        index = Math.Clamp(index, 0, parts.Count - 1);
        scroll.ChangeView(null, tops[index], null, !Motion.Enabled || owner.PaperMode);
    }

    /// <summary>The engine moved the place: a restore, as on opening.</summary>
    public void Restored()
    {
        restoring = true;
        Restore();
    }

    public void Tap(double x) => owner.ToggleChrome();
}
