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
/// cells. The bars' lines grow with the text size, as the cards' do. Hidden from screen readers;
/// the heading or the place says it is loading.
/// </summary>
static class ReadPlates
{
    /// <summary>A cover card's height: the cover, the title's line and the caption's, at the text size in force.</summary>
    public static double CardHeight(double width = HomeWording.PosterWidth) =>
        Math.Round(width * 1.5) + 8 + Lines(ReadingParts.TitleLine) + 2 + Lines(ReadingParts.CaptionLine);

    /// <summary>A line's height at the text size in force.</summary>
    public static double Lines(double line) => Math.Ceiling(line * ReadingParts.TextScale);

    /// <summary>One card's plate and bars, as tall as the card it stands for.</summary>
    public static StackPanel Card(double width = HomeWording.PosterWidth)
    {
        var card = new StackPanel { Width = width, Height = CardHeight(width) };
        card.Children.Add(States.Plate(width, Math.Round(width * 1.5), 4));
        var title = States.Plate(Math.Round(width * 0.8), 12, 3);
        title.VerticalAlignment = VerticalAlignment.Center;
        card.Children.Add(new Border { Height = 8 + Lines(ReadingParts.TitleLine), Padding = new Thickness(0, 8, 0, 0), Child = title });
        var line = States.Plate(Math.Round(width * 0.5), 10, 3);
        line.VerticalAlignment = VerticalAlignment.Center;
        card.Children.Add(new Border { Height = 2 + Lines(ReadingParts.CaptionLine), Padding = new Thickness(0, 2, 0, 0), Child = line });
        return card;
    }

    /// <summary>A result's plate as a list row for very large text: the cover's plate beside two bars.</summary>
    public static Microsoft.UI.Xaml.Controls.Grid ListRow()
    {
        var row = new Microsoft.UI.Xaml.Controls.Grid { ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.Children.Add(States.Plate(56, 84, 4));
        var bars = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        bars.Children.Add(States.Plate(220, 14, 3));
        bars.Children.Add(States.Plate(120, 12, 3));
        Microsoft.UI.Xaml.Controls.Grid.SetColumn(bars, 1);
        row.Children.Add(bars);
        return row;
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
        var list = ReadingParts.ListText;
        var grid = new ItemsRepeater
        {
            Layout = ReadGrid.MakeLayout(),
            ItemsSource = Enumerable.Range(0, list ? Math.Min(count, 6) : count).ToList(),
            ItemTemplate = new CellFactory(list),
        };
        AutomationProperties.SetAccessibilityView(grid, AccessibilityView.Raw);
        return grid;
    }

    sealed partial class CellFactory(bool list) : IElementFactory
    {
        public UIElement GetElement(ElementFactoryGetArgs args) => list ? ListRow() : Card(ReadingParts.GridCell);

        public void RecycleElement(ElementFactoryRecycleArgs args) { }
    }
}

/// <summary>
/// A panel of covers that screen readers meet as a list, with the row's name: Narrator says
/// "Trending manga, list" and each card's place in it.
/// </summary>
sealed partial class ListPanel : StackPanel
{
    protected override AutomationPeer OnCreateAutomationPeer() => new ListPeer(this);

    sealed partial class ListPeer(FrameworkElement owner) : FrameworkElementAutomationPeer(owner)
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.List;

        protected override string GetClassNameCore() => nameof(ListPanel);

        protected override bool IsControlElementCore() => true;

        protected override bool IsContentElementCore() => true;
    }
}

/// <summary>
/// How a part leaves its place: it fades out, then its gap closes on the standard spring (at
/// once with reduced motion), and only then is it gone.
/// </summary>
static class Leaving
{
    public static void Close(FrameworkElement view, Action gone)
    {
        view.IsHitTestVisible = false;
        var done = false;
        void Gone()
        {
            if (done)
                return;
            done = true;
            gone();
        }
        if (view.ActualHeight <= 0 || view.Visibility == Visibility.Collapsed)
        {
            Gone();
            return;
        }
        Motion.FadeOutThen(view, () =>
        {
            var top = view.Margin.Top;
            var from = view.ActualHeight + top;
            Motion.Ease(view, from, 0, v =>
            {
                var share = from > 0 ? v / from : 0;
                view.Margin = new Thickness(view.Margin.Left, top * share, view.Margin.Right, 0);
                view.Height = Math.Max(0, v - view.Margin.Top);
                if (v <= 0.5)
                    Gone();
            });
        });
    }
}

/// <summary>
/// A line of words that changes with one crossfade: the new words fade in as the old fade out,
/// in the same place. Its room stays while it is empty, so nothing under it moves.
/// </summary>
sealed partial class FadeLine : Grid
{
    readonly Func<TextBlock> make;
    TextBlock front;

    public FadeLine(Func<TextBlock> make)
    {
        this.make = make;
        front = make();
        Children.Add(front);
    }

    /// <summary>No words, at once, the room kept.</summary>
    public void Clear()
    {
        Children.Clear();
        front = make();
        Children.Add(front);
    }

    public string Text
    {
        get => front.Text;
        set
        {
            if (value == front.Text)
                return;
            var next = make();
            next.Text = value;
            next.Opacity = 0;
            foreach (var old in Children.OfType<TextBlock>().ToList())
            {
                if (old.Text.Length == 0)
                    Children.Remove(old);
                else
                    Motion.FadeOutThen(old, () => Children.Remove(old));
            }
            Children.Add(next);
            front = next;
            if (value.Length > 0)
                Motion.FadeIn(next);
        }
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
    readonly ListPanel strip = new()
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

    /// <summary>How far the strip is scrolled: a new drawing of the same row starts where the old one stood.</summary>
    public double Offset
    {
        get => scroll.HorizontalOffset;
        set
        {
            if (value <= 0)
                return;
            void Go(object sender, object e)
            {
                scroll.LayoutUpdated -= Go;
                scroll.ChangeView(value, null, null, true);
            }
            scroll.LayoutUpdated += Go;
        }
    }

    /// <summary>The card at <paramref name="index"/>, to take the focus.</summary>
    public Control? CardAt(int index) => index >= 0 && index < strip.Children.Count ? strip.Children[index] as Control : null;

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
/// shows it keeps the placeholders' room, unseen. Placeholders fade in 600 ms after the asking
/// began and stay at least 0.3 s. What comes next fades in over what shows in 200 ms (ease out),
/// and what showed fades out over 300 ms once the new is opaque; the slot then takes its new
/// height on the standard spring (at once with reduced motion). New covers in place of covers
/// that show come the same way, as a fresh layer. Fades stay with reduced motion. When the part
/// that leaves held the focus, the focus moves to the slot's placeholders or its first cover.
/// </summary>
sealed partial class CoverSlot : Grid
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(HomeWording.PlaceholderAfterMs), Least = TimeSpan.FromMilliseconds(HomeWording.PlaceholderLeastMs);

    enum Layer { None, Plates, Covers, Failure }

    // The placeholders sit in a holder that can take the focus while they show, when the part
    // that held it left: Narrator then says the row is loading.
    readonly ContentControl plates;
    readonly DispatcherQueueTimer due;
    readonly List<FrameworkElement> stale = [];
    FrameworkElement content;
    CoverStrip? strip;
    FrameworkElement? failure;
    Layer shown, wanted;
    DateTimeOffset platesSince;
    string label = "";
    int top = 1;

    /// <summary>A row's slot: a strip of covers, with a row of plates for placeholders.</summary>
    public CoverSlot(string name, int plateCount = 7) : this(ReadPlates.Row(plateCount), new CoverStrip(name)) => Label = name;

    /// <summary>A slot for any <paramref name="content"/>, with <paramref name="placeholders"/> standing for it.</summary>
    public CoverSlot(FrameworkElement placeholders, FrameworkElement content)
    {
        plates = new ContentControl
        {
            Content = placeholders,
            IsTabStop = false,
            UseSystemFocusVisuals = true,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            // Until something shows, the plates keep the room unseen, so nothing moves when it does.
            Opacity = 0,
        };
        plates.LostFocus += (_, _) => plates.IsTabStop = false;
        this.content = content;
        strip = content as CoverStrip;
        content.Visibility = Visibility.Collapsed;
        content.VerticalAlignment = VerticalAlignment.Top;
        Children.Add(plates);
        Children.Add(content);
        due = Timers.Weak(this, Silence, false, static slot => slot.Due());
        // What changes in the slot stays inside it while the slot eases to its new height; the
        // margin leaves room for a card's hover and focus outset and the paging buttons.
        SizeChanged += (_, e) => Clip = new RectangleGeometry
        {
            Rect = new global::Windows.Foundation.Rect(-40, -HomeWording.HoverOutset, e.NewSize.Width + 80, e.NewSize.Height + 2 * HomeWording.HoverOutset),
        };
    }

    /// <summary>The content shows now, not the placeholders or a failure.</summary>
    public bool Showing => shown == Layer.Covers;

    /// <summary>Something shows in the slot now: placeholders, covers or a failure.</summary>
    public bool Seen => shown != Layer.None;

    /// <summary>The placeholders show: the heading says it is loading.</summary>
    public bool Waiting => shown == Layer.Plates;

    /// <summary>The content as it now stands, the newest layer.</summary>
    public FrameworkElement Current => content;

    /// <summary>The first time anything shows in the slot, and when the placeholders come or go.</summary>
    public event Action? Changed;

    /// <summary>The row's name, which its strip and its placeholders say.</summary>
    public string Label
    {
        set
        {
            label = value;
            if (strip is not null)
                strip.Label = value;
            AutomationProperties.SetName(plates, $"{value}, {T.StateLoading}");
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

    /// <summary>
    /// Nothing shows, at once, and the placeholders keep the room unseen: for a page that comes
    /// in with a crossfade of its own, so it never shows what an earlier search left.
    /// </summary>
    public void Clear()
    {
        due.Stop();
        shown = wanted = Layer.None;
        foreach (var layer in stale)
            Children.Remove(layer);
        stale.Clear();
        content.Visibility = Visibility.Collapsed;
        content.Opacity = 1;
        if (failure is not null)
            failure.Visibility = Visibility.Collapsed;
        plates.Opacity = 0;
        plates.Visibility = Visibility.Visible;
        Height = double.NaN;
    }

    /// <summary>
    /// The covers. When covers show already they come as a fresh strip over the old one, which
    /// keeps its sideways place and the focus; else over what shows.
    /// </summary>
    public void Covers(IReadOnlyList<FrameworkElement> cards)
    {
        if (strip is null)
            return;
        if (shown == Layer.Covers)
        {
            var fresh = new CoverStrip(label);
            fresh.Set(cards);
            Replace(fresh);
            return;
        }
        strip.Set(cards);
        Content();
    }

    /// <summary>The content, as it now is: in place when it already shows, else over what shows.</summary>
    public void Content()
    {
        wanted = Layer.Covers;
        Land();
    }

    /// <summary>
    /// <paramref name="fresh"/> in place of the content. While the content shows, the fresh layer
    /// fades in over it, the old one fades out, then the slot eases to its new height; the focus
    /// goes to the same card in the fresh layer. Otherwise it simply takes the old one's place.
    /// </summary>
    public void Replace(FrameworkElement fresh)
    {
        var old = content;
        if (fresh == old)
        {
            Content();
            return;
        }
        fresh.VerticalAlignment = VerticalAlignment.Top;
        content = fresh;
        if (fresh is CoverStrip next)
        {
            next.Label = label;
            if (strip is not null)
                next.Offset = strip.Offset;
            strip = next;
        }
        wanted = Layer.Covers;
        due.Stop();
        if (shown != Layer.Covers)
        {
            fresh.Visibility = Visibility.Collapsed;
            var at = Children.IndexOf(old);
            if (at >= 0)
                Children[at] = fresh;
            else
                Children.Add(fresh);
            Land();
            return;
        }
        var from = ActualHeight;
        var mark = Ui.MarkFocus(old);
        stale.Add(old);
        // The slot keeps its height while the layers cross; the spring takes it after.
        if (from > 0)
            Height = from;
        fresh.Opacity = 0;
        Children.Add(fresh);
        Canvas.SetZIndex(fresh, ++top);
        Motion.FadeInThen(fresh, () =>
        {
            if (content != fresh)
                return;
            var going = stale.ToList();
            stale.Clear();
            var left = going.Count;
            foreach (var layer in going)
                Motion.FadeOutThen(layer, () =>
                {
                    Children.Remove(layer);
                    if (--left > 0)
                        return;
                    if (mark is not null)
                        Ui.RestoreFocus(fresh, mark);
                    Settle(from);
                });
            if (going.Count == 0)
                Settle(from);
        });
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

    /// <summary>The placeholders: they fade in over 200 ms where nothing showed, else over what shows.</summary>
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
            plates.Visibility = Visibility.Visible;
            Motion.FadeTo(plates, 1, Motion.FadeInTime);
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
        if (from > 0)
            Height = from;
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
                var held = outgoing is FrameworkElement part && Ui.FocusWithin(part) is not null;
                outgoing.Visibility = Visibility.Collapsed;
                outgoing.Opacity = 1;
                if (held)
                    Refocus(incoming);
                Settle(from);
            });
        });
    }

    /// <summary>The focus, from a part that left, to the placeholders that say the row is loading or to the first cover.</summary>
    void Refocus(UIElement incoming)
    {
        if (incoming == plates)
        {
            plates.IsTabStop = true;
            plates.Focus(FocusState.Programmatic);
            return;
        }
        if (FocusManager.FindFirstFocusableElement(incoming) is Control first)
            first.Focus(FocusState.Programmatic);
    }

    /// <summary>The slot takes its new height on the standard spring; only what is below it moves.</summary>
    void Settle(double from)
    {
        Height = double.NaN;
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
    /// asks again for this slot alone. The glyph is hidden from screen readers; Narrator hears the
    /// line and the reason once the block shows, with <paramref name="said"/> for the reason when
    /// it reads better spoken, as "No answer in 15 seconds".
    /// </summary>
    public static StackPanel FailureBlock(string line, string? reason, Action retry, string? said = null)
    {
        var block = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Padding = new Thickness(0, 4, 0, 8) };
        var glyph = States.ErrorGlyph(17);
        glyph.VerticalAlignment = VerticalAlignment.Top;
        glyph.Margin = new Thickness(0, 1, 0, 0);
        AutomationProperties.SetAccessibilityView(glyph, AccessibilityView.Raw);
        block.Children.Add(glyph);
        var words = new StackPanel();
        var lead = new TextBlock { Text = line, FontSize = 14, LineHeight = 19, TextWrapping = TextWrapping.Wrap };
        words.Children.Add(lead);
        if (reason is { Length: > 0 })
        {
            var why = new TextBlock { Text = reason, Style = Ui.Res<Style>("HazeCaptionStyle"), FontSize = 13, LineHeight = 18, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 2, 0, 0) };
            if (said is { Length: > 0 })
                AutomationProperties.SetName(why, said);
            words.Children.Add(why);
        }
        var again = new Button { Content = T.ActionTryAgain, MinHeight = 32, Margin = new Thickness(0, 10, 0, 0) };
        AutomationProperties.SetName(again, T.ActionTryAgain);
        AutomationProperties.SetHelpText(again, ReadingParts.Joined(line, said ?? reason));
        again.Click += (_, _) => retry();
        words.Children.Add(again);
        block.Children.Add(words);
        AutomationProperties.SetName(block, ReadingParts.Joined(line, said ?? reason));
        AutomationProperties.SetLiveSetting(block, AutomationLiveSetting.Polite);
        // The live region speaks when it shows, not when it is built.
        block.RegisterPropertyChangedCallback(VisibilityProperty, (_, _) =>
        {
            if (block.Visibility == Visibility.Visible)
                ReaderSpeech.Raise(block);
        });
        return block;
    }
}

/// <summary>
/// A discovery row of Read's Home: the heading with its side words and View all, then the covers
/// in a slot. A row on its way holds placeholders at final size after 600 ms; its heading fades
/// in with them and says it is loading. A row that leaves fades out, then its gap closes. Each
/// cover's line is the row's own: where the series is from, then how it stands or how long it
/// is, two facts at most.
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
            if (this.row.ViewAll is not { } query)
                return;
            // Back comes to this View all again, the way it was reached.
            ReadHomePage.Returning = (this.row.Id, viewAll.FocusState == FocusState.Unfocused ? FocusState.Programmatic : viewAll.FocusState);
            ReadingParts.OpenSearch(ctx, query);
        };
        Grid.SetColumn(viewAll, 1);
        head.Children.Add(viewAll);
        Children.Add(head);

        slot = new CoverSlot(row.Title.Text());
        slot.Changed += () =>
        {
            // The heading fades in with the placeholders or the covers, whichever comes first.
            if (head.Opacity < 1 && slot.Seen)
                Motion.FadeTo(head, 1, Motion.FadeInTime);
            Say();
        };
        SetRow(slot, 1);
        Children.Add(slot);
    }

    public string Id => row.Id;

    /// <summary>View all takes the focus, as it is when Back comes to Home from its Search.</summary>
    public void FocusViewAll(FocusState how)
    {
        if (viewAll.Visibility == Visibility.Visible)
            viewAll.Focus(how);
    }

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
        var key = string.Join("\n", tiles.Select(t => $"{t.Work}\t{t.Title}\t{t.Cover}\t{ReadDiscovery.RowCaption(now.Kind, t)}"));
        if (key != drawn)
        {
            drawn = key;
            slot.Covers([.. tiles.Select(t => (FrameworkElement)ReadingParts.Card(ctx, t, caption: ReadDiscovery.RowCaption(now.Kind, t)))]);
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
        if (!slot.Seen)
        {
            IsHitTestVisible = false;
            gone();
            return;
        }
        Leaving.Close(this, gone);
    }
}

/// <summary>
/// One source's row in Search's From your sources: its name and kind, a small ring while its
/// placeholders show, then its covers sideways, or why it didn't answer with Try again for it
/// alone. Five hits show at first, with Show N more under them for the rest. Each state takes
/// the slot of the one before; Narrator reads "Home library, Server" as the row's heading and
/// each cover's place in the row. A row that leaves fades out, then its gap closes.
/// </summary>
sealed partial class SourceRowView : StackPanel
{
    /// <summary>The hits a row shows before Show N more.</summary>
    const int FirstHits = 5;

    readonly Ctx ctx;
    readonly Grid head;
    readonly TextBlock heading;
    readonly Run name = new(), kind = new();
    readonly ProgressRing ring = new() { Width = 12, Height = 12, MinWidth = 12, MinHeight = 12, IsActive = false, Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(8, 0, 0, 0) };
    readonly CoverSlot slot;
    readonly HyperlinkButton more = new() { Visibility = Visibility.Collapsed, Margin = new Thickness(0, 4, 0, 0), Padding = new Thickness(0, 4, 0, 4) };
    string drawn = "", failed = "";
    DateTimeOffset asked;
    bool all, leaving;
    WorkTile[] hits = [];

    public SourceRowView(Ctx ctx, string id)
    {
        this.ctx = ctx;
        Id = id;
        // The name trims before the kind and the ring are pushed off the row.
        head = new Grid { Margin = new Thickness(0, 26, 0, 10) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        heading = new TextBlock { FontSize = 14, LineHeight = 19, TextWrapping = TextWrapping.NoWrap, TextTrimming = TextTrimming.CharacterEllipsis, HorizontalAlignment = HorizontalAlignment.Left };
        name.FontWeight = FontWeights.SemiBold;
        kind.Foreground = Ui.Res<Brush>("HazeBrush");
        heading.Inlines.Add(name);
        heading.Inlines.Add(kind);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level3);
        head.Children.Add(heading);
        AutomationProperties.SetAccessibilityView(ring, AccessibilityView.Raw);
        Grid.SetColumn(ring, 1);
        head.Children.Add(ring);
        Children.Add(head);
        slot = new CoverSlot(ReadPlates.Row(FirstHits), new CoverStrip(id));
        slot.Label = id;
        // The ring comes and goes with the placeholders, not with every ask.
        slot.Changed += () =>
        {
            var waiting = slot.Waiting;
            ring.IsActive = waiting;
            ring.Visibility = waiting ? Visibility.Visible : Visibility.Collapsed;
            Changed?.Invoke();
        };
        Children.Add(slot);
        more.Style = Ui.Res<Style>("SilkLinkStyle");
        more.Click += (_, _) => ShowAll();
        Children.Add(more);
    }

    public string Id { get; }

    /// <summary>Something shows in the row's slot: placeholders, covers or a failure.</summary>
    public bool Seen => slot.Seen;

    /// <summary>The row's slot changed what it shows.</summary>
    public event Action? Changed;

    public bool IsLeaving => leaving;

    /// <summary>The first row comes closer to the section's heading.</summary>
    public bool First
    {
        set => head.Margin = new Thickness(0, value ? 12 : 26, 0, 10);
    }

    /// <summary>New words are on their way: what shows stays 600 ms, then the placeholders.</summary>
    public void Ask(DateTimeOffset since) => slot.Ask(since);

    /// <summary>Takes the group as the engine now has it; <paramref name="since"/> is when the words were asked.</summary>
    public void Update(ReadSourceGroup group, DateTimeOffset since)
    {
        if (since != asked)
        {
            // New words: the row starts again at its first five.
            asked = since;
            all = false;
        }
        var kindWords = ReadDiscovery.KindWords(group.Kind);
        if (name.Text != group.SourceName)
            name.Text = group.SourceName;
        // An en space between the name and its kind, as the board's gap.
        var aside = kindWords is null ? "" : " " + kindWords;
        if (kind.Text != aside)
            kind.Text = aside;
        var said = kindWords is null ? group.SourceName : $"{group.SourceName}, {kindWords}";
        slot.Label = said;
        // The source is named once; the placeholders add that it is loading.
        AutomationProperties.SetName(heading, said);
        switch (group.State)
        {
            case ReadSourceState.Asking:
                // Asked again after it failed: the 600 ms count from Try again.
                slot.Ask(failed.Length > 0 ? DateTimeOffset.Now : since);
                failed = "";
                More(0);
                break;
            case ReadSourceState.Hits:
                hits = group.Tiles ?? [];
                Draw();
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
                    var spoken = group.State is ReadSourceState.TimedOut timedOut ? T.WindowsReadSearchSourceTimeoutSaid(timedOut.Seconds) : null;
                    slot.Fail(CoverSlot.FailureBlock(line, reason, () => ctx.Known.Reading.RetrySource(id), spoken));
                }
                More(0);
                break;
        }
    }

    /// <summary>The hits as they stand: the first five, or all of them after Show N more.</summary>
    void Draw()
    {
        WorkTile[] shown = all ? hits : [.. hits.Take(FirstHits)];
        var key = string.Join("\n", shown.Select(t => $"{t.Work}\t{t.Title}\t{t.Cover}\t{ReadDiscovery.Amount(t)}"));
        if (key != drawn)
        {
            drawn = key;
            // Under a hit's title, how much of it the source holds; nothing when it does not say.
            // A hit's menu is Details alone.
            slot.Covers([.. shown.Select(t => (FrameworkElement)ReadingParts.Card(ctx, t, caption: ReadDiscovery.Amount(t), lists: false))]);
        }
        More(hits.Length - shown.Length);
    }

    void More(int left)
    {
        if (left > 0)
        {
            more.Content = T.ReadSearchSourceMore(left);
            AutomationProperties.SetName(more, ReadingParts.Joined(T.ReadSearchSourceMore(left), name.Text));
            more.Visibility = Visibility.Visible;
            more.Opacity = 1;
        }
        else
            more.Visibility = Visibility.Collapsed;
    }

    /// <summary>Show N more: the rest of the hits come in the row, and the focus goes to the first of them.</summary>
    void ShowAll()
    {
        all = true;
        var next = FirstHits;
        Draw();
        if (slot.Current is CoverStrip strip && strip.CardAt(next) is { } card)
            card.Focus(FocusState.Keyboard);
    }

    /// <summary>The source left the rows, as when it found nothing or its hits joined AniList's: it fades out, then its gap closes.</summary>
    public void Leave(Action gone)
    {
        if (leaving)
            return;
        leaving = true;
        Leaving.Close(this, gone);
    }
}
