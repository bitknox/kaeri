using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The strip: every image of the chapter in one vertical scroll, a centred column on a PC. Only
/// the images near the window are mounted. After the last image a marker names the next chapter,
/// with its first image under it; once the marker reaches the window's middle the next chapter
/// opens under it in the same scroll, so reading runs on with no jump and no blank. Chapters far
/// above are let go. Vertical pages and the last chapter there is end on the end page.
/// </summary>
sealed class StripView : Grid
{
    /// <summary>The marker's words between two chapters: Next, the title, its facts.</summary>
    const double MarkerHeight = 200;

    /// <summary>One chapter in the scroll, with the files of a chapter the reader has moved past.</summary>
    sealed class Section(string chapter, string label)
    {
        public string Chapter { get; } = chapter;
        public string Label { get; } = label;
        public List<LeafPart> Parts { get; } = [];
        public Dictionary<ReaderPageKey, PageFile> Kept { get; } = [];
        /// <summary>The marker into this chapter: the foot it flowed from, its first image under the words.</summary>
        public FrameworkElement? Marker { get; set; }
        public double Top { get; set; }
        public List<double> Tops { get; } = [];
        public double Bottom { get; set; }
    }

    readonly ReaderOverlay owner;
    readonly ScrollViewer scroll;
    readonly Canvas column = new();
    readonly List<Section> sections = [];
    readonly Dictionary<(Section, int), PartView> live = [];
    FrameworkElement? foot;
    PageFile? footPreview;
    double footLead;
    double columnWidth = 720;
    bool restoring = true;
    bool carry;
    Section? shown;
    int image;
    int spoken = -1;
    Section? spokenIn;

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
        SizeChanged += (_, _) =>
        {
            Layout();
            if (restoring)
                Restore();
        };
    }

    ReaderSession Reader => owner.Reader;

    double Gap => Reader.Choice(ReaderSettingKey.StripGap) == "small" ? 8 : 0;

    /// <summary>Strip mode runs on into the next chapter; vertical pages end on the end page.</summary>
    bool Flows => Reader.View?.Mode == ReaderMode.Strip;

    Section? Current => sections.Count > 0 ? sections[^1] : null;

    /// <summary>The image at the window's top in the chapter under its middle, counted from 0, and how many that chapter has.</summary>
    public (int Image, int Count) Position => (image, (shown ?? Current)?.Parts.Count ?? 0);

    /// <summary>The chapter under the window's middle, when the strip has run on from another and it is that one.</summary>
    public string? ChapterLabel => shown is { } s && s != Current ? s.Label : null;

    void Interacted()
    {
        restoring = false;
        owner.HideChrome();
    }

    // Building

    public void Rebuild()
    {
        var view = Reader.View;
        var newChapter = view?.Chapter.Id ?? "";
        var parts = Reader.Leaves.Where(l => !l.EndPage).SelectMany(l => l.Parts).ToList();
        if (Reader.Flowing)
        {
            // The next chapter is opening under the marker: wait for its images, then add them.
            if (parts.Count == 0 || Current is null || Current.Chapter == newChapter)
                return;
            Flow(newChapter, view?.Chapter.Label ?? "", parts);
            Reader.Flowed();
            return;
        }
        if (Current is null || Current.Chapter != newChapter)
        {
            // Another chapter opened (the list, the end page, a restore): the scroll starts over.
            foreach (var key in live.Keys.ToList())
                Unmount(key);
            foreach (var section in sections)
                if (section.Marker is not null)
                    column.Children.Remove(section.Marker);
            sections.Clear();
            sections.Add(new Section(newChapter, view?.Chapter.Label ?? ""));
            shown = null;
            restoring = true;
            carry = owner.TakeCarry();
            spoken = -1;
        }
        Current!.Parts.Clear();
        Current.Parts.AddRange(parts);
        // The look or the layout changed: every image is made again on the new ground.
        foreach (var key in live.Keys.ToList())
            Unmount(key);
        DropFoot();
        Layout();
        if (restoring)
            Restore();
    }

    /// <summary>The chapter ran on: what was the foot is now its marker, and its images follow it.</summary>
    void Flow(string chapter, string label, List<LeafPart> parts)
    {
        var from = Current!;
        // The chapter before keeps the files it showed: the session lets them go.
        foreach (var part in from.Parts)
            if (Reader.FileOf(part, from.Chapter) is { } file)
                from.Kept[new ReaderPageKey(from.Chapter, part.Page, part.Half)] = file;
        var next = new Section(chapter, label) { Marker = foot };
        next.Parts.AddRange(parts);
        // The first image shows the file the marker already showed, so it never blanks.
        if (footPreview is { } lead && parts.Count > 0)
            next.Kept[new ReaderPageKey(chapter, parts[0].Page, parts[0].Half)] = lead;
        foot = null;
        footPreview = null;
        sections.Add(next);
        Layout();
    }

    public void RefreshPages()
    {
        var before = Current is { } s && s.Tops.Count > 0 ? s.Tops[^1] : 0;
        Layout();
        foreach (var view in live.Values)
            view.Refresh(owner.Scale);
        // An image that came in above the place being restored moves it: put it back.
        if (restoring && Current is { } now && now.Tops.Count > 0 && Math.Abs(now.Tops[^1] - before) > 0.5)
            Restore();
    }

    /// <summary>The foot's words changed: the next chapter, or the end page.</summary>
    public void RefreshFoot()
    {
        // A flowing chapter's marker stays as it is: it becomes the chapter's top.
        if (Reader.Flowing)
            return;
        DropFoot();
        Layout();
    }

    void DropFoot()
    {
        if (foot is null)
            return;
        column.Children.Remove(foot);
        foot = null;
        footPreview = null;
    }

    PageFile? FileFor(Section section, LeafPart part) =>
        Reader.FileOf(part, section.Chapter)
        ?? (section.Kept.TryGetValue(new ReaderPageKey(section.Chapter, part.Page, part.Half), out var half) ? half
            : section.Kept.TryGetValue(new ReaderPageKey(section.Chapter, part.Page), out var whole) ? whole : null);

    double HeightOf(Section section, LeafPart part)
    {
        if (FileFor(section, part) is { } file)
            return HeightOf(file);
        if (part.Ratio is { } ratio && ratio > 0)
            return columnWidth / ratio;
        return columnWidth * 1.5;
    }

    double HeightOf(PageFile file)
    {
        double w = file.Crop?.Width ?? file.Width, h = file.Crop?.Height ?? file.Height;
        return w > 0 ? columnWidth * h / w : columnWidth * 1.5;
    }

    void Layout()
    {
        if (ActualWidth <= 0 || Current is null)
            return;
        // What is at the window's top now, so a new width or new image heights keep it there.
        var anchor = restoring ? null : Anchor();
        var factor = (Reader.Amount(ReaderSettingKey.StripWidth) ?? 0.6) / 0.6;
        columnWidth = Math.Min(ActualWidth, 720 * factor);
        double y = 0;
        var gap = Gap;
        foreach (var section in sections)
        {
            section.Top = y;
            if (section.Marker is { } marker)
            {
                if (!column.Children.Contains(marker))
                    column.Children.Add(marker);
                marker.Width = columnWidth;
                Canvas.SetTop(marker, y);
                Canvas.SetZIndex(marker, 0);
                y += MarkerHeight;
            }
            section.Tops.Clear();
            foreach (var part in section.Parts)
            {
                section.Tops.Add(y);
                y += HeightOf(section, part) + gap;
            }
            section.Bottom = y;
        }
        foot ??= BuildFoot();
        if (!column.Children.Contains(foot))
            column.Children.Add(foot);
        foot.Width = columnWidth;
        foot.Height = foot is EndPageView ? Math.Max(ActualHeight * 0.8, 420) : Math.Max(MarkerHeight + footLead, ActualHeight * 0.6);
        Canvas.SetTop(foot, y);
        column.Width = columnWidth;
        column.Height = y + foot.Height;
        foreach (var ((section, index), view) in live)
        {
            view.Width = columnWidth;
            view.Height = HeightOf(section, section.Parts[index]);
            Canvas.SetTop(view, section.Tops[index]);
        }
        if (anchor is { } kept)
            Keep(kept, column.Height);
        Mount();
    }

    /// <summary>A place in the strip that survives a relayout: a chapter, an image in it (-1 for its marker, Count for the foot) and how far into that.</summary>
    readonly record struct Place(Section Section, int Index, double Into);

    /// <summary>Where the scroll is about to be: a move asked of it lands a moment later.</summary>
    double? going;

    double Offset => going ?? scroll.VerticalOffset;

    /// <summary>The place at the window's top, from the layout as it stands.</summary>
    Place? Anchor()
    {
        var y = Offset;
        foreach (var section in sections)
        {
            if (section.Tops.Count != section.Parts.Count || section.Parts.Count == 0)
                return null;
            var last = section == sections[^1];
            if (y >= section.Bottom && !last)
                continue;
            if (y < section.Tops[0])
                return new Place(section, -1, y - section.Top);
            if (y >= section.Bottom)
                return new Place(section, section.Parts.Count, y - section.Bottom);
            var i = section.Tops.FindLastIndex(t => t <= y);
            var span = (i + 1 < section.Tops.Count ? section.Tops[i + 1] : section.Bottom) - section.Tops[i];
            return new Place(section, i, span > 0 ? (y - section.Tops[i]) / span : 0);
        }
        return null;
    }

    /// <summary>Puts the place back at the window's top after a relayout moved it.</summary>
    void Keep(Place place, double height)
    {
        var section = place.Section;
        if (!sections.Contains(section) || section.Tops.Count != section.Parts.Count || section.Parts.Count == 0)
            return;
        double y;
        if (place.Index < 0)
            y = section.Top + place.Into;
        else if (place.Index >= section.Parts.Count)
            y = section.Bottom + place.Into;
        else
        {
            var i = place.Index;
            var span = (i + 1 < section.Tops.Count ? section.Tops[i + 1] : section.Bottom) - section.Tops[i];
            y = section.Tops[i] + place.Into * span;
        }
        y = Math.Clamp(y, 0, Math.Max(0, height - scroll.ViewportHeight));
        if (Math.Abs(y - Offset) <= 0.5)
            return;
        going = y;
        scroll.ChangeView(null, y, null, true);
    }

    /// <summary>
    /// The marker into the next chapter, with its first image (or a plate naming it) under the
    /// words; the end page when there is no next chapter or the strip is vertical pages.
    /// </summary>
    FrameworkElement BuildFoot()
    {
        var next = Reader.EndNext;
        footPreview = null;
        footLead = 0;
        if (next is null || !Flows)
            return owner.BuildEndPage();
        var palette = owner.PagePalette;
        var marker = new StackPanel();
        var words = new StackPanel { Spacing = 4, Padding = new Thickness(28, 0, 28, 0), Height = MarkerHeight };
        words.Children.Add(new Border { Height = 1, Background = new SolidColorBrush(palette.Rule), Margin = new Thickness(0, 40, 0, 28) });
        words.Children.Add(new TextBlock { Text = T.ReaderEndNext, FontSize = 13, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Haze) });
        var title = next.Number is { Length: > 0 } number
            ? next.Title is { Length: > 0 } named ? T.ReaderChapterLineTitled(number, named) : T.ReaderChapterLine(number)
            : next.Title ?? next.Label;
        words.Children.Add(new TextBlock { Text = title, FontSize = 20, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Silk), TextWrapping = TextWrapping.WrapWholeWords, MaxLines = 2 });
        var images = next.Pages is { } count && count > 0 ? T.ReaderDividerImages(count) : null;
        var source = Reader.End?.NextSource is { Length: > 0 } from ? from : null;
        var facts = (images, source) switch
        {
            ({ } i, { } s) => T.ReaderDividerFactsSource(i, s),
            (null, { } s) => T.ReaderEndFrom(s),
            (var i, null) => i,
        };
        if (facts is not null)
            words.Children.Add(new TextBlock { Text = facts, FontSize = 15, Foreground = new SolidColorBrush(palette.Haze) });
        marker.Children.Add(words);
        AutomationProperties.SetName(words, facts is null ? title : title + ", " + facts);
        AutomationProperties.SetHeadingLevel(words, AutomationHeadingLevel.Level2);

        // Its first image, where the chapter's first image will be once it opens.
        if (Reader.NextPreview is { } preview)
        {
            footPreview = preview;
            footLead = HeightOf(preview);
            var lead = new PageImageView { Width = columnWidth, Height = footLead, Dim = palette.Paper, Cut = palette.Paper };
            lead.Show(preview, owner.Scale);
            AutomationProperties.SetAccessibilityView(lead, AccessibilityView.Raw);
            marker.Children.Add(lead);
        }
        else
        {
            footLead = columnWidth * 1.42;
            var plate = new Border
            {
                Height = footLead,
                Background = new SolidColorBrush(palette.Raised),
                Child = new TextBlock
                {
                    Text = next.Number is { Length: > 0 } n ? T.ReaderChapterLine(n) : next.Label,
                    FontSize = 15,
                    Foreground = new SolidColorBrush(palette.Haze),
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                },
            };
            AutomationProperties.SetAccessibilityView(plate, AccessibilityView.Raw);
            marker.Children.Add(plate);
        }
        return marker;
    }

    /// <summary>Only the images within a couple of windows of the view are mounted.</summary>
    void Mount()
    {
        if (Current is null)
            return;
        var top = scroll.VerticalOffset - ActualHeight * 1.5;
        var bottom = scroll.VerticalOffset + ActualHeight * 2.5;
        var want = new HashSet<(Section, int)>();
        foreach (var section in sections)
        {
            if (section.Bottom < top || section.Top > bottom || section.Tops.Count != section.Parts.Count)
                continue;
            for (var i = 0; i < section.Parts.Count; i++)
            {
                var h = HeightOf(section, section.Parts[i]);
                if (section.Tops[i] + h >= top && section.Tops[i] <= bottom)
                    want.Add((section, i));
            }
        }
        foreach (var gone in live.Keys.Where(k => !want.Contains(k)).ToList())
            Unmount(gone);
        var atOnce = carry;
        if (want.Count > 0)
            carry = false;
        foreach (var key in want)
        {
            if (live.ContainsKey(key))
                continue;
            var (section, index) = key;
            var part = section.Parts[index];
            var view = new PartView(Reader, part, section.Chapter, owner.PagePalette, HorizontalAlignment.Center, true, false,
                T.WindowsReaderImageLabel(index + 1, section.Parts.Count), atOnce,
                section.Kept.Count > 0 ? p => FileFor(section, p) : null, strip: true)
            {
                Width = columnWidth,
                Height = HeightOf(section, part),
            };
            Canvas.SetTop(view, section.Tops[index]);
            // Over the marker's first image, which it takes over from.
            Canvas.SetZIndex(view, 1);
            column.Children.Add(view);
            live[key] = view;
            view.Refresh(owner.Scale);
            if (section == Current && Reader.FileOf(part, section.Chapter) is null && Reader.FailureOf(part, section.Chapter) is null)
                Reader.NeedPage(part.Page);
        }
    }

    void Unmount((Section, int) key)
    {
        if (live.Remove(key, out var view))
            column.Children.Remove(view);
    }

    void Restore()
    {
        if (Current is not { } section || section.Parts.Count == 0 || section.Tops.Count != section.Parts.Count)
            return;
        var at = Reader.StripRestore ?? Reader.View?.Start.Strip;
        if (at is null)
        {
            going = null;
            scroll.ChangeView(null, section.Top, null, true);
            return;
        }
        var index = Math.Min((int)at.Image, section.Parts.Count - 1);
        going = null;
        scroll.ChangeView(null, section.Tops[index] + at.Offset * HeightOf(section, section.Parts[index]), null, true);
    }

    void Track(bool moving)
    {
        if (!moving)
            going = null;
        Mount();
        if (Current is not { } current || current.Tops.Count != current.Parts.Count)
            return;
        var y = scroll.VerticalOffset;
        var middle = y + scroll.ViewportHeight / 2;
        var footNow = y + scroll.ViewportHeight >= scroll.ExtentHeight - 40;

        // The chapter under the window's middle names the group's line and the scrubber.
        var under = sections.LastOrDefault(s => s.Top <= middle) ?? sections[0];
        var found = 0;
        for (var i = 0; i < under.Parts.Count && i < under.Tops.Count; i++)
        {
            found = i;
            if (y < under.Tops[i] + HeightOf(under, under.Parts[i]) + Gap)
                break;
        }
        if (found != image || under != shown)
        {
            image = found;
            shown = under;
            owner.PositionChanged();
        }
        // Narrator hears the image once the scroll comes to rest on a new one.
        if (!moving && (found != spoken || under != spokenIn))
        {
            spoken = found;
            spokenIn = under;
            if (!restoring)
                owner.StripSettled();
        }
        // The person's jump reports once the scroll lands, so the engine can offer Back to page.
        if (!moving && jumped is { } leaf)
        {
            jumped = null;
            if (!restoring && under == current)
                Reader.Shown(leaf, TurnHow.Jump);
        }
        if (!restoring && !moving && under == current && current.Parts.Count > 0)
        {
            var h = HeightOf(current, current.Parts[found]);
            var into = h > 0 ? Math.Clamp((y - current.Tops[found]) / h, 0, 1) : 0;
            Reader.StripAt(new StripPos((uint)found, (float)into), footNow);
        }

        // The marker into the next chapter reached the middle: the chapter opens under it.
        if (!restoring && Flows && foot is not null and not EndPageView && !Reader.Flowing
            && Canvas.GetTop(foot) <= middle && Reader.EndNext is not null)
            Reader.FlowOn(under == current ? (uint)found : 0);

        if (!moving)
            Trim();
    }

    /// <summary>A chapter far above the window is let go, and the scroll keeps its place.</summary>
    void Trim()
    {
        var y = scroll.VerticalOffset;
        if (sections.Count < 3 || sections[0].Bottom >= y - ActualHeight * 3)
            return;
        var gone = sections[0];
        foreach (var key in live.Keys.Where(k => k.Item1 == gone).ToList())
            Unmount(key);
        if (gone.Marker is not null)
            column.Children.Remove(gone.Marker);
        sections.RemoveAt(0);
        var shift = sections[0].Top;
        Layout();
        scroll.ChangeView(null, Math.Max(0, y - shift), null, true);
    }

    // Moving

    /// <summary>A key or a click zone: most of a window down or up, or a line with the arrows.</summary>
    public void Step(bool forward, bool line)
    {
        restoring = false;
        var step = line ? 120 : scroll.ViewportHeight * 0.9;
        var from = Offset;
        going = null;
        scroll.ChangeView(null, Math.Max(0, from + (forward ? step : -step)), null, !Motion.Enabled || owner.PaperMode);
    }

    /// <summary>
    /// Home and End: the top and the foot of the chapter under the window's middle. After the
    /// strip ran on, the chapter before stays above in the scroll, and Home doesn't climb into it.
    /// </summary>
    public void Ends(bool last)
    {
        restoring = false;
        var section = shown ?? Current;
        double to;
        if (section is null || section.Parts.Count == 0 || section.Tops.Count != section.Parts.Count)
            to = last ? scroll.ScrollableHeight : 0;
        else if (!last)
            to = section.Tops[0];
        else if (section == Current)
            to = scroll.ScrollableHeight;
        else
            to = Math.Max(section.Tops[0], section.Bottom - scroll.ViewportHeight);
        going = null;
        scroll.ChangeView(null, Math.Clamp(to, 0, scroll.ScrollableHeight), null, !Motion.Enabled || owner.PaperMode);
    }

    /// <summary>To the top of an image of the chapter under the middle, counted from 0: the scrubber and the engine's moves.</summary>
    public void JumpTo(int index)
    {
        var section = shown ?? Current;
        if (section is null || section.Parts.Count == 0 || section.Tops.Count != section.Parts.Count)
            return;
        restoring = false;
        index = Math.Clamp(index, 0, section.Parts.Count - 1);
        jumped = section == Current && Math.Abs(Offset - section.Tops[index]) > 1 ? index : null;
        going = null;
        scroll.ChangeView(null, section.Tops[index], null, !Motion.Enabled || owner.PaperMode);
    }

    /// <summary>The image the person jumped to, until the scroll lands on it.</summary>
    int? jumped;

    /// <summary>The engine moved the place: a restore, as on opening.</summary>
    public void Restored()
    {
        restoring = true;
        Restore();
    }

    public void Tap(double x) => owner.ToggleChrome();
}
