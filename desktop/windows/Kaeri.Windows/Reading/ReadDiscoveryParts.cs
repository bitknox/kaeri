using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Placeholders at the final size of what they stand for: a cover card of Home's rows and
/// Search's source rows, 132 by 198 with the title's and the line's bars, and the cover grid's
/// cells. Hidden from screen readers; the heading or the place says it is loading.
/// </summary>
static class ReadPlates
{
    /// <summary>A cover card's height: the cover, the title's line and the caption's.</summary>
    public static double CardHeight(double width = HomeWording.PosterWidth) =>
        Math.Round(width * 1.5) + 8 + ReadingParts.TitleLine + 2 + ReadingParts.CaptionLine;

    /// <summary>One card's plate and bars, as tall as the card it stands for.</summary>
    public static StackPanel Card(double width = HomeWording.PosterWidth)
    {
        var card = new StackPanel { Width = width, Height = CardHeight(width) };
        card.Children.Add(States.Plate(width, Math.Round(width * 1.5), 4));
        var title = States.Plate(Math.Round(width * 0.8), 12, 3);
        title.VerticalAlignment = VerticalAlignment.Center;
        card.Children.Add(new Border { Height = 8 + ReadingParts.TitleLine, Padding = new Thickness(0, 8, 0, 0), Child = title });
        var line = States.Plate(Math.Round(width * 0.5), 10, 3);
        line.VerticalAlignment = VerticalAlignment.Center;
        card.Children.Add(new Border { Height = 2 + ReadingParts.CaptionLine, Padding = new Thickness(0, 2, 0, 0), Child = line });
        return card;
    }

    /// <summary>A row of <paramref name="count"/> card plates, spaced as the row's covers are.</summary>
    public static StackPanel Row(int count)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = HomeWording.Gap };
        for (var i = 0; i < count; i++)
            row.Children.Add(Card());
        AutomationProperties.SetAccessibilityView(row, AccessibilityView.Raw);
        return row;
    }

    /// <summary>The cover grid's cells as plates: the same layout as <see cref="ReadGrid"/>, so the covers take their room.</summary>
    public static FrameworkElement Grid(int count = 12)
    {
        var grid = new ItemsRepeater
        {
            Layout = new UniformGridLayout
            {
                MinItemWidth = ReadingParts.GridCell,
                MinColumnSpacing = ReadingParts.GridGap,
                MinRowSpacing = 24,
                ItemsStretch = UniformGridLayoutItemsStretch.Fill,
            },
            ItemsSource = Enumerable.Range(0, count).ToList(),
            ItemTemplate = new CellFactory(),
        };
        AutomationProperties.SetAccessibilityView(grid, AccessibilityView.Raw);
        return grid;
    }

    sealed partial class CellFactory : IElementFactory
    {
        public UIElement GetElement(ElementFactoryGetArgs args)
        {
            var cell = Card(ReadingParts.GridCell);
            cell.Width = double.NaN;
            cell.HorizontalAlignment = HorizontalAlignment.Stretch;
            cell.SizeChanged += (_, e) =>
            {
                if (cell.Children is [Border cover, ..] && e.NewSize.Width > 0 && Math.Abs(cover.Width - e.NewSize.Width) > 0.5)
                {
                    cover.Width = e.NewSize.Width;
                    cover.Height = Math.Round(e.NewSize.Width * 1.5);
                    cell.Height = CardHeight(e.NewSize.Width);
                }
            };
            return cell;
        }

        public void RecycleElement(ElementFactoryRecycleArgs args) { }
    }
}

/// <summary>
/// A sideways strip of cover cards that pages by whole cards: the paging buttons show while the
/// pointer is over it, the wheel moves the page and Shift with the wheel the strip. Tab enters
/// it once and leaves it; the arrow keys move along it. Each card says its place, "1 of 20".
/// </summary>
sealed partial class CoverStrip : Grid
{
    readonly ScrollViewer scroll = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
        HorizontalScrollMode = ScrollMode.Enabled,
        IsHorizontalRailEnabled = true,
        VerticalScrollMode = ScrollMode.Disabled,
        VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
        IsTabStop = false,
    };
    readonly StackPanel strip = new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = HomeWording.Gap,
        TabFocusNavigation = KeyboardNavigationMode.Once,
        XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled,
        // The cards reach past the strip by their hover outset; the strip gives it back.
        Margin = new Thickness(HomeWording.HoverOutset),
    };
    readonly Button previous, next;
    readonly double cardWidth;
    bool inside;

    public CoverStrip(string name, double cardWidth = HomeWording.PosterWidth, double artHeight = HomeWording.PosterHeight)
    {
        this.cardWidth = cardWidth;
        AutomationProperties.SetName(strip, name);
        scroll.Margin = new Thickness(-HomeWording.HoverOutset);
        scroll.Content = strip;
        HomeShelf.ForwardWheel(scroll);
        scroll.ViewChanged += (_, _) => Arrows();
        scroll.SizeChanged += (_, _) => Arrows();
        Children.Add(scroll);
        previous = Arrow("", T.HomePrevious, false, artHeight);
        next = Arrow("", T.HomeNext, true, artHeight);
        Children.Add(previous);
        Children.Add(next);
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        PointerEntered += (_, _) => { inside = true; Arrows(); };
        PointerMoved += (_, _) =>
        {
            if (inside)
                return;
            inside = true;
            Arrows();
        };
        PointerExited += (_, _) => { inside = false; Arrows(); };
#if KAERI_TESTING
        inside = Ctx.Current.Options.Show == "arrows";
#endif
    }

    /// <summary>The name screen readers give the strip: the row's.</summary>
    public string Label
    {
        set => AutomationProperties.SetName(strip, value);
    }

    public int Count => strip.Children.Count;

    /// <summary>Puts <paramref name="cards"/> in the strip. The focus stays on the card it was on, where that card stays.</summary>
    public void Set(IReadOnlyList<FrameworkElement> cards)
    {
        var mark = Ui.MarkFocus(strip);
        strip.Children.Clear();
        for (var i = 0; i < cards.Count; i++)
        {
            AutomationProperties.SetPositionInSet(cards[i], i + 1);
            AutomationProperties.SetSizeOfSet(cards[i], cards.Count);
            strip.Children.Add(cards[i]);
        }
        if (mark is not null)
            Ui.RestoreFocus(strip, mark);
        Arrows();
    }

    Button Arrow(string glyph, string name, bool forward, double artHeight)
    {
        var button = new Button
        {
            Width = 36,
            Height = 36,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(18),
            HorizontalAlignment = forward ? HorizontalAlignment.Right : HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(forward ? 0 : -18, artHeight / 2 - 18, forward ? -18 : 0, 0),
            Background = Ui.Res<Brush>("RaisedBrush"),
            Content = new FontIcon { Glyph = glyph, FontSize = 14 },
            Opacity = 0,
            IsHitTestVisible = false,
            // The arrow keys page the strip from the keyboard; the buttons are for the pointer.
            IsTabStop = false,
        };
        Canvas.SetZIndex(button, 5);
        AutomationProperties.SetName(button, name);
        button.Click += (_, _) =>
        {
            var to = ShelfPaging.Target(scroll.HorizontalOffset, scroll.ViewportWidth - 2 * HomeWording.HoverOutset,
                scroll.ExtentWidth - 2 * HomeWording.HoverOutset, cardWidth, strip.Children.Count, forward);
            Motion.Ease(scroll, scroll.HorizontalOffset, to, v => scroll.ChangeView(v, null, null, true));
        };
        return button;
    }

    void Arrows()
    {
        Show(previous, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, false));
        Show(next, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, true));
    }

    static void Show(Button arrow, bool shown)
    {
        arrow.IsHitTestVisible = shown;
        if (shown == arrow.Opacity > 0.5)
            return;
        Motion.Fade(arrow, shown ? 1 : 0);
    }
}

/// <summary>
/// One slot that holds a row's covers, its placeholders or a failure, one over the other, so
/// whatever it changes to takes the same place and nothing above it moves. Before anything
/// shows it keeps the placeholders' room, unseen. Placeholders come 600 ms after the asking
/// began and stay at least 0.3 s. What comes next fades in over what shows in 200 ms (ease out),
/// and what showed fades out over 300 ms once the new is opaque; the slot then takes its new
/// height on the standard spring (at once with reduced motion). Fades stay with reduced motion.
/// </summary>
sealed partial class CoverSlot : Grid
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(HomeWording.PlaceholderAfterMs), Least = TimeSpan.FromMilliseconds(HomeWording.PlaceholderLeastMs);

    enum Layer { None, Plates, Covers, Failure }

    readonly FrameworkElement plates, content;
    readonly CoverStrip? strip;
    readonly DispatcherQueueTimer due;
    FrameworkElement? failure;
    Layer shown, wanted;
    DateTimeOffset platesSince;
    int top = 1;

    /// <summary>A row's slot: a strip of covers, with a row of plates for placeholders.</summary>
    public CoverSlot(string name, int plateCount = 7) : this(ReadPlates.Row(plateCount), new CoverStrip(name)) { }

    /// <summary>A slot for any <paramref name="content"/>, with <paramref name="plates"/> standing for it.</summary>
    public CoverSlot(FrameworkElement plates, FrameworkElement content)
    {
        this.plates = plates;
        this.content = content;
        strip = content as CoverStrip;
        // Until something shows, the plates keep the room unseen, so nothing moves when it does.
        plates.Opacity = 0;
        plates.VerticalAlignment = VerticalAlignment.Top;
        content.Visibility = Visibility.Collapsed;
        content.VerticalAlignment = VerticalAlignment.Top;
        Children.Add(plates);
        Children.Add(content);
        due = Timers.Weak(this, Silence, false, static slot => slot.Due());
    }

    /// <summary>The content shows now, not the placeholders or a failure.</summary>
    public bool Showing => shown == Layer.Covers;

    /// <summary>Something shows in the slot now: placeholders, covers or a failure.</summary>
    public bool Seen => shown != Layer.None;

    /// <summary>The placeholders show: the heading says it is loading.</summary>
    public bool Waiting => shown == Layer.Plates;

    /// <summary>The first time anything shows in the slot, and when the placeholders come or go.</summary>
    public event Action? Changed;

    public string Label
    {
        set
        {
            if (strip is not null)
                strip.Label = value;
        }
    }

    /// <summary>
    /// Asking since <paramref name="since"/>: what shows stays until 600 ms have passed, then the
    /// placeholders take its place.
    /// </summary>
    public void Ask(DateTimeOffset since)
    {
        wanted = Layer.Plates;
        if (shown == Layer.Plates)
            return;
        var left = since + Silence - DateTimeOffset.Now;
        due.Stop();
        if (left <= TimeSpan.Zero)
        {
            Plates();
            return;
        }
        due.Interval = left;
        due.Start();
    }

    /// <summary>The covers: in place when they already show, else over what shows.</summary>
    public void Covers(IReadOnlyList<FrameworkElement> cards)
    {
        strip?.Set(cards);
        Content();
    }

    /// <summary>The content, as it now is: in place when it already shows, else over what shows.</summary>
    public void Content()
    {
        wanted = Layer.Covers;
        Land();
    }

    /// <summary>A failure in the slot's place: the reason and Try again.</summary>
    public void Fail(FrameworkElement block)
    {
        wanted = Layer.Failure;
        if (failure is not null && failure != block)
        {
            if (shown == Layer.Failure)
            {
                // A new reason for a failure already showing: in place.
                var at = Children.IndexOf(failure);
                Children[at] = block;
                Canvas.SetZIndex(block, Canvas.GetZIndex(failure));
                failure = block;
                return;
            }
            Children.Remove(failure);
        }
        if (failure != block)
        {
            failure = block;
            block.Visibility = Visibility.Collapsed;
            block.VerticalAlignment = VerticalAlignment.Top;
            Children.Add(block);
        }
        Land();
    }

    void Due()
    {
        due.Stop();
        if (wanted == Layer.Plates)
            Plates();
        else
            Land();
    }

    /// <summary>The placeholders, at once and with no fade of their own, as the places' states have them.</summary>
    void Plates()
    {
        if (shown == Layer.Plates)
            return;
        var from = ActualHeight;
        var before = shown;
        platesSince = DateTimeOffset.Now;
        shown = Layer.Plates;
        if (before == Layer.None)
        {
            plates.Opacity = 1;
            plates.Visibility = Visibility.Visible;
            Changed?.Invoke();
            return;
        }
        Over(plates, LayerOf(before), from);
        Changed?.Invoke();
    }

    /// <summary>Shows what is wanted, once the placeholders have stood their 0.3 s.</summary>
    void Land()
    {
        if (wanted == shown)
            return;
        if (shown == Layer.Plates)
        {
            var left = platesSince + Least - DateTimeOffset.Now;
            if (left > TimeSpan.Zero)
            {
                due.Stop();
                due.Interval = left;
                due.Start();
                return;
            }
        }
        due.Stop();
        var from = ActualHeight;
        var before = shown;
        shown = wanted;
        var incoming = LayerOf(wanted);
        if (incoming is null)
            return;
        if (before == Layer.None)
        {
            // Nothing showed yet: the new part fades in where the placeholders kept its room.
            Canvas.SetZIndex(incoming, ++top);
            Motion.FadeInThen(incoming, () =>
            {
                if (shown != Layer.Plates)
                    plates.Visibility = Visibility.Collapsed;
                Settle(from);
            });
            Changed?.Invoke();
            return;
        }
        Over(incoming, LayerOf(before), from);
        Changed?.Invoke();
    }

    /// <summary>Fades <paramref name="incoming"/> in over <paramref name="outgoing"/>, then the old one out, then the height.</summary>
    void Over(UIElement incoming, UIElement? outgoing, double from)
    {
        Canvas.SetZIndex(incoming, ++top);
        Motion.FadeInThen(incoming, () =>
        {
            if (outgoing is null || LayerOf(shown) == outgoing)
            {
                Settle(from);
                return;
            }
            Motion.FadeOutThen(outgoing, () =>
            {
                if (LayerOf(shown) == outgoing)
                    return;
                outgoing.Visibility = Visibility.Collapsed;
                outgoing.Opacity = 1;
                Settle(from);
            });
        });
    }

    /// <summary>The slot takes its new height on the standard spring; only what is below it moves.</summary>
    void Settle(double from)
    {
        UpdateLayout();
        Measure(new global::Windows.Foundation.Size(ActualWidth > 0 ? ActualWidth : double.PositiveInfinity, double.PositiveInfinity));
        var to = DesiredSize.Height;
        if (from > 0 && Math.Abs(from - to) > 1)
            Motion.Height(this, from, to);
    }

    UIElement? LayerOf(Layer layer) => layer switch
    {
        Layer.Plates => plates,
        Layer.Covers => content,
        Layer.Failure => failure,
        _ => null,
    };

    /// <summary>
    /// A failure in a slot: the error glyph, the line, the reason under it and Try again, which
    /// asks again for this slot alone. The glyph is hidden from screen readers.
    /// </summary>
    public static StackPanel FailureBlock(string line, string? reason, Action retry)
    {
        var block = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Padding = new Thickness(0, 4, 0, 8) };
        var glyph = States.ErrorGlyph(17);
        glyph.VerticalAlignment = VerticalAlignment.Top;
        glyph.Margin = new Thickness(0, 1, 0, 0);
        AutomationProperties.SetAccessibilityView(glyph, AccessibilityView.Raw);
        block.Children.Add(glyph);
        var words = new StackPanel();
        words.Children.Add(new TextBlock { Text = line, FontSize = 14, LineHeight = 19, TextWrapping = TextWrapping.Wrap });
        if (reason is { Length: > 0 })
            words.Children.Add(new TextBlock { Text = reason, Style = Ui.Res<Style>("HazeCaptionStyle"), FontSize = 13, LineHeight = 18, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 2, 0, 0) });
        var again = new Button { Content = T.ActionTryAgain, MinHeight = 32, Margin = new Thickness(0, 10, 0, 0) };
        AutomationProperties.SetName(again, ReadingParts.Joined(line, reason, T.ActionTryAgain));
        again.Click += (_, _) => retry();
        words.Children.Add(again);
        block.Children.Add(words);
        AutomationProperties.SetLiveSetting(block, AutomationLiveSetting.Polite);
        return block;
    }
}

/// <summary>
/// A discovery row of Read's Home: the heading with its side words and View all, then the covers
/// in a slot. A row on its way holds placeholders at final size after 600 ms; its heading shows
/// with them and says it is loading. A row that leaves fades out, then its gap closes.
/// </summary>
sealed partial class ReadHomeRowView : Grid
{
    readonly Ctx ctx;
    readonly TextBlock heading;
    readonly Run lead = new(), title = new(), tail = new(), side = new();
    readonly HyperlinkButton viewAll;
    readonly CoverSlot slot;
    ReadHomeRow row;
    string drawn = "";
    bool leaving;

    public ReadHomeRowView(Ctx ctx, ReadHomeRow row)
    {
        this.ctx = ctx;
        this.row = row;
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var head = new Grid { Margin = new Thickness(0, 0, 0, 12), Opacity = 0 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        heading = new TextBlock
        {
            FontSize = 20,
            LineHeight = 28,
            FontWeight = FontWeights.SemiBold,
            VerticalAlignment = VerticalAlignment.Bottom,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxLines = 1,
            Margin = new Thickness(0, 0, 12, 0),
        };
        title.FontFamily = Ui.Res<FontFamily>("ShowFont");
        title.FontWeight = FontWeights.Bold;
        side.FontSize = 12;
        side.FontWeight = FontWeights.Normal;
        side.Foreground = Ui.Res<Brush>("HazeBrush");
        heading.Inlines.Add(lead);
        heading.Inlines.Add(title);
        heading.Inlines.Add(tail);
        heading.Inlines.Add(side);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        head.Children.Add(heading);
        viewAll = new HyperlinkButton
        {
            Style = Ui.Res<Style>("SilkLinkStyle"),
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Children = { new TextBlock { Text = T.HomeViewAll, FontSize = 14, FontWeight = FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
            },
        };
        viewAll.Click += (_, _) =>
        {
            if (this.row.ViewAll is { } query)
                ReadingParts.OpenSearch(ctx, query);
        };
        Grid.SetColumn(viewAll, 1);
        head.Children.Add(viewAll);
        Children.Add(head);

        slot = new CoverSlot(row.Title.Text());
        slot.Changed += () =>
        {
            if (head.Opacity < 1 && slot.Seen)
            {
                // With the placeholders the heading simply shows; with the covers it fades in with them.
                if (slot.Waiting)
                    head.Opacity = 1;
                else
                    Motion.FadeTo(head, 1, Motion.FadeInTime);
            }
            Say();
        };
        SetRow(slot, 1);
        Children.Add(slot);
    }

    public string Id => row.Id;

    /// <summary>Takes the row as the engine now has it; <paramref name="since"/> is when Home began asking.</summary>
    public void Update(ReadHomeRow now, DateTimeOffset since)
    {
        row = now;
        var words = ReadDiscovery.Heading(now.Title);
        Set(lead, words.Lead);
        Set(title, words.Title);
        Set(tail, words.Tail);
        var aside = now.Side?.Text();
        // An em space keeps the side words apart from the heading.
        Set(side, aside is null ? "" : " " + aside);
        viewAll.Visibility = now.ViewAll is null ? Visibility.Collapsed : Visibility.Visible;
        AutomationProperties.SetName(viewAll, $"{T.HomeViewAll}, {words}");
        slot.Label = words.ToString();
        var tiles = now.Tiles ?? [];
        if (tiles.Length == 0)
        {
            if (now.Pending)
                slot.Ask(since);
            Say();
            return;
        }
        var key = string.Join("\n", tiles.Select(t => $"{t.Work}\t{t.Title}\t{t.Cover}\t{ReadingParts.Line(t)}"));
        if (key != drawn)
        {
            drawn = key;
            slot.Covers([.. tiles.Select(t => (FrameworkElement)ReadingParts.Card(ctx, t))]);
        }
        Say();
    }

    static void Set(Run run, string text)
    {
        if (run.Text != text)
            run.Text = text;
    }

    /// <summary>The heading as Narrator reads it: "New manga, Last 12 months", and "loading" while the placeholders show.</summary>
    void Say()
    {
        var words = ReadDiscovery.Heading(row.Title).ToString();
        var parts = new List<string> { words };
        if (row.Side?.Text() is { Length: > 0 } aside)
            parts.Add(aside);
        if ((row.Tiles?.Length ?? 0) == 0 && row.Pending)
            parts.Add(T.StateLoading);
        AutomationProperties.SetName(heading, string.Join(", ", parts));
    }

    /// <summary>
    /// The row left Home, as when AniList failed with nothing kept: its heading and covers fade
    /// out, then the gap closes on the standard spring (at once with reduced motion).
    /// </summary>
    public void Leave(Action gone)
    {
        if (leaving)
            return;
        leaving = true;
        IsHitTestVisible = false;
        if (!slot.Seen || ActualHeight <= 0)
        {
            gone();
            return;
        }
        Motion.FadeOutThen(this, () =>
        {
            var from = ActualHeight + Margin.Top;
            var top = Margin.Top;
            Motion.Ease(this, from, 0, v =>
            {
                var share = from > 0 ? v / from : 0;
                Margin = new Thickness(0, top * share, 0, 0);
                Height = Math.Max(0, v - Margin.Top);
                if (v <= 0.5)
                    gone();
            });
        });
    }
}

/// <summary>
/// One source's row in Search's From your sources: its name and kind, a small ring while it is
/// asked, then its covers sideways, or why it didn't answer with Try again for it alone. Each
/// state takes the slot of the one before; Narrator reads "Home library, Server" as the row's
/// heading and each cover's place in the row.
/// </summary>
sealed partial class SourceRowView : StackPanel
{
    readonly Ctx ctx;
    readonly TextBlock heading;
    readonly Run name = new(), kind = new();
    readonly ProgressRing ring = new() { Width = 12, Height = 12, MinWidth = 12, MinHeight = 12, IsActive = false, Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center };
    readonly CoverSlot slot;
    string drawn = "", failed = "";

    public SourceRowView(Ctx ctx, string id)
    {
        this.ctx = ctx;
        Id = id;
        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 26, 0, 10) };
        heading = new TextBlock { FontSize = 14, LineHeight = 19, TextWrapping = TextWrapping.NoWrap, TextTrimming = TextTrimming.CharacterEllipsis };
        name.FontWeight = FontWeights.SemiBold;
        kind.Foreground = Ui.Res<Brush>("HazeBrush");
        heading.Inlines.Add(name);
        heading.Inlines.Add(kind);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level3);
        head.Children.Add(heading);
        AutomationProperties.SetAccessibilityView(ring, AccessibilityView.Raw);
        head.Children.Add(ring);
        Children.Add(head);
        slot = new CoverSlot(id);
        Children.Add(slot);
    }

    public string Id { get; }

    /// <summary>The first row comes closer to the section's heading.</summary>
    public bool First
    {
        set => ((FrameworkElement)Children[0]).Margin = new Thickness(0, value ? 12 : 26, 0, 10);
    }

    /// <summary>Takes the group as the engine now has it; <paramref name="since"/> is when the words were asked.</summary>
    public void Update(ReadSourceGroup group, ReadSourceKind sourceKind, DateTimeOffset since)
    {
        var kindWords = ReadDiscovery.KindWords(sourceKind);
        if (name.Text != group.SourceName)
            name.Text = group.SourceName;
        // An en space between the name and its kind, as the board's gap.
        var aside = kindWords is null ? "" : " " + kindWords;
        if (kind.Text != aside)
            kind.Text = aside;
        var said = kindWords is null ? group.SourceName : $"{group.SourceName}, {kindWords}";
        slot.Label = said;
        var asking = group.State is ReadSourceState.Asking;
        ring.IsActive = asking;
        ring.Visibility = asking ? Visibility.Visible : Visibility.Collapsed;
        AutomationProperties.SetName(heading, asking ? $"{said}, {T.ReadSearchAsking(group.SourceName)}" : said);
        switch (group.State)
        {
            case ReadSourceState.Asking:
                // Asked again after it failed: the 600 ms count from Try again.
                slot.Ask(failed.Length > 0 ? DateTimeOffset.Now : since);
                failed = "";
                break;
            case ReadSourceState.Hits:
                var tiles = group.Tiles ?? [];
                var key = string.Join("\n", tiles.Select(t => $"{t.Work}\t{t.Title}\t{t.Cover}\t{ReadingParts.Line(t)}"));
                if (key != drawn)
                {
                    drawn = key;
                    slot.Covers([.. tiles.Select(t => (FrameworkElement)ReadingParts.Card(ctx, t))]);
                }
                failed = "";
                break;
            case ReadSourceState.Failed or ReadSourceState.TimedOut:
                var line = T.ReadSearchSourceFailed(group.SourceName);
                var reason = ReadDiscovery.Reason(group.State);
                if (failed != line + reason)
                {
                    failed = line + reason;
                    drawn = "";
                    var id = group.SourceId;
                    slot.Fail(CoverSlot.FailureBlock(line, reason, () => ctx.Known.Reading.RetrySource(id)));
                }
                break;
        }
    }
}
