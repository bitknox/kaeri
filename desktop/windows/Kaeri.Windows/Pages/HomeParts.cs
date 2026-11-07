using System.ComponentModel;
using System.Text.RegularExpressions;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace Kaeri.Windows.Pages;

/// <summary>One card or poster on a shelf: the engine's item, its art as it arrives and the words drawn from it.</summary>
public sealed class HomeCard
{
    internal HomeCard(HomeItem item, CoverItem cover, string line, string name)
    {
        Item = item;
        Cover = cover;
        Line = line;
        Name = name;
    }

    public HomeItem Item { get; private set; }
    public CoverItem Cover { get; }
    public uint Id => Item.Entry.Id;
    public string Title => Item.Entry.Title;
    public string Line { get; private set; }
    public string Name { get; private set; }
    public double? Progress => Item.Progress;

    /// <summary>The card's words changed in place; the element showing it says them again.</summary>
    public event Action? Changed;

    /// <summary>Takes a newer reading of the same title: the card that stands changes its words, and keeps its element, focus and menu.</summary>
    internal void Take(HomeCard newer)
    {
        if (Look == newer.Look)
            return;
        Item = newer.Item;
        Line = newer.Line;
        Name = newer.Name;
        Changed?.Invoke();
    }

    /// <summary>Everything the card and its menu are drawn from: a card whose look is the same is kept, element and all.</summary>
    public (string, string, string, double?, bool, Playable, HomeAction, uint?, UpNextReason?, FuzzyDate?, bool, LibrarySegment?) Look =>
        (Title, Line, Name, Progress, Item.New, Item.Playable, Item.Action, Item.Episode, Item.Reason, Item.Starts, Item.Resume is not null, Item.Entry.ListStatus);
}

/// <summary>Art that fades over its placeholder colour when it arrives, as covers do everywhere.</summary>
static class HomeArt
{
    public static Border Make(CoverItem cover, double width, double height, double radius, bool wide)
    {
        var image = new Image { Stretch = Stretch.UniformToFill, Opacity = 0 };
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        var box = new Border { Width = width, Height = height, CornerRadius = new CornerRadius(radius), Background = cover.Placeholder, Child = image };
        void Fill(bool fade)
        {
            var source = wide ? cover.Banner ?? cover.Cover : cover.Cover;
            if (source is null || image.Source == source)
                return;
            image.Source = source;
            if (fade)
                Motion.Fade(image, 1);
            else
                image.Opacity = 1;
        }
        Fill(false);
        // Heard only while the art is up: the cover outlives a card that is taken down, and a
        // handler left on it would keep the card's element alive.
        void Arrived(object? sender, PropertyChangedEventArgs e) => Fill(true);
        var hearing = false;
        box.Loaded += (_, _) =>
        {
            Fill(false);
            if (!hearing)
                cover.PropertyChanged += Arrived;
            hearing = true;
        };
        box.Unloaded += (_, _) =>
        {
            cover.PropertyChanged -= Arrived;
            hearing = false;
        };
        return box;
    }
}

/// <summary>A spotlight, hover card or title synopsis with the words AniList marks as spoilers behind the Discussions bar.</summary>
static class SynopsisText
{
    public static void Fill(TextBlock block, Entry entry)
    {
        block.Inlines.Clear();
        var spans = entry.SynopsisSpans;
        if (spans is not { Length: > 0 } || !spans.Any(s => s.Spoiler))
        {
            block.Text = entry.Synopsis;
            return;
        }
        var bars = new List<(Border Bar, TextBlock Text)>();
        var open = false;
        void Set(bool on)
        {
            open = on;
            foreach (var (bar, text) in bars)
            {
                text.Opacity = on ? 1 : 0;
                bar.Opacity = on ? 0.55 : 1;
                AutomationProperties.SetName(bar, on ? text.Text : T.DiscussionSpoiler);
            }
        }
        foreach (var span in spans)
        {
            if (!span.Spoiler)
            {
                block.Inlines.Add(new Run { Text = span.Text });
                continue;
            }
            foreach (Match m in Regex.Matches(span.Text, @"\S+\s*|\s+"))
            {
                var text = new TextBlock { Text = m.Value.Replace('\n', ' '), FontSize = block.FontSize, Opacity = 0 };
                var bar = new Border
                {
                    Background = Ui.Res<Brush>("SpoilerBarBrush"),
                    CornerRadius = new CornerRadius(bars.Count == 0 ? 4 : 0, 0, 0, bars.Count == 0 ? 4 : 0),
                    Child = text,
                    IsTabStop = bars.Count == 0,
                    UseSystemFocusVisuals = true,
                };
                AutomationProperties.SetName(bar, T.DiscussionSpoiler);
                AutomationProperties.SetAutomationControlType(bar, AutomationControlType.Button);
                bar.Tapped += (_, e) =>
                {
                    e.Handled = true;
                    Set(!open);
                };
                bar.KeyDown += (_, e) =>
                {
                    if (e.Key is VirtualKey.Enter or VirtualKey.Space)
                    {
                        e.Handled = true;
                        Set(!open);
                    }
                };
                bars.Add((bar, text));
                block.Inlines.Add(new InlineUIContainer { Child = bar });
            }
        }
        Set(false);
    }
}

/// <summary>Builds the element for one <see cref="HomeCard"/>: a wide Continue card or a poster, as a flat button.</summary>
sealed partial class HomeCardFactory(HomeShape shape, HomePage page) : IElementFactory
{
    public UIElement GetElement(ElementFactoryGetArgs args) => args.Data is HomeCard card ? Build(card) : new Grid();

    public void RecycleElement(ElementFactoryRecycleArgs args) { }

    Button Build(HomeCard card)
    {
        var wide = shape == HomeShape.Card;
        var width = wide ? HomeWording.CardWidth : HomeWording.PosterWidth;
        var height = wide ? HomeWording.CardHeight : HomeWording.PosterHeight;
        var stack = new StackPanel { Width = width };
        stack.Children.Add(HomeArt.Make(card.Cover, width, height, Art.Radius(width), wide));
        Border? track = null, fill = null;
        if (wide)
        {
            track = new Border
            {
                Height = 3,
                Margin = new Thickness(0, 6, 0, 0),
                CornerRadius = new CornerRadius(2),
                Background = Ui.Res<Brush>("RuleBrush"),
            };
            fill = new Border { Height = 3, HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(2), Background = Ui.Res<Brush>("SilkBrush") };
            track.Child = fill;
            stack.Children.Add(track);
        }
        var title = new TextBlock { Style = Ui.Res<Style>("ShowTitleStyle"), Margin = new Thickness(0, 8, 0, 0), IsTextScaleFactorEnabled = true };
        stack.Children.Add(title);
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        var dot = Ui.Dot(Colours.Water, false);
        line.Children.Add(dot);
        var words = new TextBlock { Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis };
        line.Children.Add(words);
        stack.Children.Add(line);
        var notFound = new TextBlock { Text = T.HomeNotFound, Style = Ui.Res<Style>("HazeCaptionStyle") };
        stack.Children.Add(notFound);

        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = stack,
            Tag = card,
            Width = width + 2 * HomeWording.HoverOutset,
        };
        // The words from the card as it is now: set once, then again only where a newer reading
        // of the title changed them, so a card is never made again for a new line or a new dot.
        void Say()
        {
            if (track is not null && fill is not null)
            {
                track.Opacity = card.Progress is null ? 0 : 1;
                fill.Width = width * Math.Clamp(card.Progress ?? 0, 0, 1);
            }
            if (title.Text != card.Title)
                title.Text = card.Title;
            dot.Visibility = card.Item.New ? Visibility.Visible : Visibility.Collapsed;
            if (words.Text != card.Line)
                words.Text = card.Line;
            notFound.Visibility = card.Item.Playable == Playable.NotFound ? Visibility.Visible : Visibility.Collapsed;
            AutomationProperties.SetName(button, card.Name);
        }
        Say();
        // Heard only while the card is up, so a card taken down is not kept alive by its record.
        var hearing = false;
        button.Loaded += (_, _) =>
        {
            Say();
            if (!hearing)
                card.Changed += Say;
            hearing = true;
        };
        button.Unloaded += (_, _) =>
        {
            card.Changed -= Say;
            hearing = false;
        };
        button.Click += (_, _) => page.OpenCard(card, shape);
        button.ContextFlyout = page.MenuFor(card, shape);
        button.KeyDown += (_, e) => page.CardKey(card, shape, e);
        HoverPreview.Attach(button, card.Id);
        return button;
    }
}

/// <summary>
/// A shelf: the heading with its count and link, and the cards in an <see cref="ItemsRepeater"/>
/// that scrolls sideways by whole cards. The arrows show while the pointer is over the shelf.
/// </summary>
sealed partial class HomeShelf : Grid
{
    readonly HomePage page;
    readonly ScrollViewer scroll = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
        // Touch and a touchpad swipe the strip sideways, and pass an upright swipe to the page.
        HorizontalScrollMode = ScrollMode.Enabled,
        IsHorizontalRailEnabled = true,
        VerticalScrollMode = ScrollMode.Disabled,
        VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
        IsTabStop = false,
    };
    /// <summary>
    /// The wheel moves the page, not a sideways strip; Shift and a sideways wheel move the strip.
    /// The strip's content hides an upright wheel from the strip, then the strip hands it on
    /// untouched, so the page's own scrolling takes it just as it does between the shelves, with
    /// its glide, and a touchpad's precise scrolling and inertia.
    /// </summary>
    internal static void ForwardWheel(ScrollViewer inner)
    {
        var content = (UIElement)inner.Content;
        var passing = false;
        double? heading = null;
        inner.ViewChanged += (_, e) =>
        {
            if (!e.IsIntermediate)
                heading = null;
        };
        content.AddHandler(PointerWheelChangedEvent, new PointerEventHandler((_, e) =>
        {
            var point = e.GetCurrentPoint(inner).Properties;
            // Handled here, the strip's own wheel handling, which would turn an upright wheel
            // sideways, leaves it alone.
            e.Handled = true;
            if (point.IsHorizontalMouseWheel || e.KeyModifiers.HasFlag(VirtualKeyModifiers.Shift))
            {
                passing = false;
                // Shift with the wheel down goes on along the strip, as everywhere in Windows.
                var by = point.IsHorizontalMouseWheel ? point.MouseWheelDelta : -point.MouseWheelDelta;
                // A mouse's notches glide along the strip as the page's do (Smooth scrolling).
                if (SmoothWheel.Sideways(inner, e, by))
                    return;
                heading = Math.Clamp((heading ?? inner.HorizontalOffset) + by, 0, inner.ScrollableWidth);
                inner.ChangeView(heading, null, null, false);
                return;
            }
            passing = true;
        }), true);
        inner.AddHandler(PointerWheelChangedEvent, new PointerEventHandler((_, e) =>
        {
            if (!passing)
                return;
            passing = false;
            e.Handled = false;
        }), true);
    }

    readonly ItemsRepeater repeater;
    readonly Button previous, next;
    /// <summary>
    /// One line: the lead, the title in the show font, what follows it, then the small aside. As
    /// one block its spaces keep their width, and it is cut short before the link.
    /// </summary>
    readonly TextBlock heading;
    readonly Run lead = new(), title = new(), tail = new(), aside = new();
    readonly HyperlinkButton viewAll;
    readonly double cardWidth;
    bool inside;

    public SectionKind Kind { get; }
    public HomeShape Shape { get; }
    /// <summary>The shelf's cards, changed in place: a card that stays keeps its element, its focus and its menu.</summary>
    readonly System.Collections.ObjectModel.ObservableCollection<HomeCard> cards = [];
    public IReadOnlyList<HomeCard> Cards => cards;

    public HomeShelf(HomePage page, HomeSection section, SeasonYear? season)
    {
        this.page = page;
        Kind = section.Kind;
        Shape = HomeWording.Shape(section.Kind);
        cardWidth = Shape == HomeShape.Card ? HomeWording.CardWidth : HomeWording.PosterWidth;
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);

        var head = new Grid { Margin = new Thickness(0, 0, 0, 12) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        heading = new TextBlock
        {
            Style = Ui.Res<Style>("SectionTitleStyle"),
            VerticalAlignment = VerticalAlignment.Bottom,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxLines = 1,
            Margin = new Thickness(0, 0, 12, 0),
        };
        title.FontFamily = Ui.Res<FontFamily>("ShowFont");
        title.FontWeight = FontWeights.Bold;
        aside.FontSize = 14;
        aside.FontWeight = FontWeights.Normal;
        aside.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        heading.Inlines.Add(lead);
        heading.Inlines.Add(title);
        heading.Inlines.Add(tail);
        heading.Inlines.Add(aside);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        head.Children.Add(heading);
        viewAll = new HyperlinkButton { Style = Ui.Res<Style>("SilkLinkStyle") };
        viewAll.Click += (_, _) => page.OpenViewAll(Kind, section.ViewAll);
        Grid.SetColumn(viewAll, 1);
        head.Children.Add(viewAll);
        Children.Add(head);

        repeater = new ItemsRepeater
        {
            Layout = new StackLayout { Orientation = Orientation.Horizontal, Spacing = HomeWording.Gap },
            ItemTemplate = new HomeCardFactory(Shape, page),
            ItemsSource = cards,
            // Tab enters a shelf once; the arrow keys move along it, and up and down between shelves.
            TabFocusNavigation = KeyboardNavigationMode.Once,
            XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled,
        };
        // The strip reaches past the shelf by a card's hover outset, which the repeater gives back,
        // so the fill of a card at either end or edge is not cut off.
        scroll.Margin = new Thickness(-HomeWording.HoverOutset);
        repeater.Margin = new Thickness(HomeWording.HoverOutset);
        scroll.Content = repeater;
        // The wheel moves the page, not the shelf; Shift and a sideways wheel move the shelf.
        ForwardWheel(scroll);
        scroll.ViewChanged += (_, _) => Arrows();
        scroll.SizeChanged += (_, _) => Arrows();
        Grid.SetRow(scroll, 1);
        Children.Add(scroll);

        var artHeight = Shape == HomeShape.Card ? HomeWording.CardHeight : HomeWording.PosterHeight;
        previous = Arrow("", T.HomePrevious, false, artHeight);
        next = Arrow("", T.HomeNext, true, artHeight);
        Children.Add(previous);
        Children.Add(next);

        // A transparent ground makes the whole shelf, gaps included, part of the hover.
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        PointerEntered += (_, _) =>
        {
            inside = true;
            Arrows();
        };
        PointerMoved += (_, _) =>
        {
            if (inside)
                return;
            inside = true;
            Arrows();
        };
        PointerExited += (_, _) =>
        {
            inside = false;
            Arrows();
        };
#if KAERI_TESTING
        // --show=arrows: the pointer is taken to be over every shelf, for a screenshot; the VM's
        // synthetic pointer does not raise PointerEntered.
        inside = Ctx.Current.Options.Show == "arrows";
#endif
        Update(section, season, []);
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
            Margin = new Thickness(forward ? 0 : -18, 40 + artHeight / 2 - 18, forward ? -18 : 0, 0),
            Background = Ui.Res<Brush>("RaisedBrush"),
            Content = new FontIcon { Glyph = glyph, FontSize = 14 },
            Opacity = 0,
            IsHitTestVisible = false,
        };
        Canvas.SetZIndex(button, 5);
        AutomationProperties.SetName(button, name);
        Grid.SetRow(button, 1);
        button.Click += (_, _) => Page(forward);
        button.GotFocus += (_, _) => Motion.Fade(button, 1);
        button.LostFocus += (_, _) => Arrows();
        return button;
    }

    /// <summary>Shows the arrows that have somewhere to go while the pointer is over the shelf, and hides the rest.</summary>
    void Arrows()
    {
        Set(previous, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, false));
        Set(next, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, true));
    }

    static void Set(Button arrow, bool shown)
    {
        arrow.IsHitTestVisible = shown;
        if (shown == arrow.Opacity > 0.5)
            return;
        Motion.Fade(arrow, shown ? 1 : 0);
    }

    /// <summary>A page: by the whole cards that fit, landing on a card edge, on the standard token.</summary>
    void Page(bool forward)
    {
        // The cards show across the shelf's own width; the strip's outset either side is for hover fills.
        var outset = 2 * HomeWording.HoverOutset;
        var to = ShelfPaging.Target(scroll.HorizontalOffset, scroll.ViewportWidth - outset, scroll.ExtentWidth - outset, cardWidth, Cards.Count, forward);
        Motion.Ease(scroll, scroll.HorizontalOffset, to, v => scroll.ChangeView(v, null, null, true));
    }

    /// <summary>Takes the section as the engine now has it. Cards that left fade out first when <paramref name="leaving"/> names them.</summary>
    public void Update(HomeSection section, SeasonYear? season, IReadOnlyCollection<uint> leaving)
    {
        var words = HomeWording.Heading(section);
        var small = HomeWording.Aside(section, season);
        // Each run and the link change only where their words did.
        static void Set(Run run, string text)
        {
            if (run.Text != text)
                run.Text = text;
        }
        Set(lead, words.Lead);
        Set(title, words.Title);
        Set(tail, words.Tail);
        // An em space keeps the aside apart from the heading, as the gap between two blocks did.
        Set(aside, small is null ? "" : " " + small);
        AutomationProperties.SetName(heading, small is null ? words.ToString() : $"{words}, {small}");
        var all = HomeWording.ViewAll(section);
        if (all is not null && viewAll.Tag as string != all)
        {
            viewAll.Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Children = { new TextBlock { Text = all, FontWeight = FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
            };
            AutomationProperties.SetName(viewAll, all);
        }
        viewAll.Tag = all;
        viewAll.Visibility = all is null ? Visibility.Collapsed : Visibility.Visible;
        var now = DateTimeOffset.Now;
        var was = cards.ToDictionary(c => c.Id);
        var next = section.Items.Select(item =>
        {
            was.TryGetValue(item.Entry.Id, out var old);
            var cover = old?.Cover ?? new CoverItem(item.Entry).Load(Ctx.Current.Art, withBanner: Shape == HomeShape.Card);
            var card = new HomeCard(item, cover, HomeWording.Line(item, now, TimeZoneInfo.Local), HomeWording.Name(item, now, TimeZoneInfo.Local));
            if (old is null)
                return card;
            // The card that stands takes the newer words in place.
            old.Take(card);
            return old;
        }).ToList();
        Apply(next);
    }

    /// <summary>
    /// Makes the shelf's cards <paramref name="next"/> with the fewest changes: cards that left
    /// are taken out, moved ones moved, changed ones replaced, and every other element stays.
    /// </summary>
    void Apply(List<HomeCard> next)
    {
        var ids = next.Select(c => c.Id).ToHashSet();
        for (var k = cards.Count - 1; k >= 0; k--)
        {
            if (!ids.Contains(cards[k].Id))
                cards.RemoveAt(k);
        }
        for (var i = 0; i < next.Count; i++)
        {
            var want = next[i];
            if (i < cards.Count && cards[i].Id == want.Id)
            {
                if (!ReferenceEquals(cards[i], want))
                    cards[i] = want;
                continue;
            }
            var at = -1;
            for (var j = i + 1; j < cards.Count; j++)
            {
                if (cards[j].Id == want.Id)
                {
                    at = j;
                    break;
                }
            }
            if (at < 0)
            {
                cards.Insert(i, want);
                continue;
            }
            cards.Move(at, i);
            if (!ReferenceEquals(cards[i], want))
                cards[i] = want;
        }
    }

    /// <summary>The element showing title <paramref name="id"/> now, if it is realised.</summary>
    public UIElement? ElementOf(uint id)
    {
        for (var i = 0; i < cards.Count; i++)
            if (cards[i].Id == id)
                return repeater.TryGetElement(i);
        return null;
    }

    /// <summary>A card leaves: it fades out (fade-out), then the cards after it close the gap.</summary>
    public void Leave(uint id, Action done)
    {
        if (ElementOf(id) is not { } element || !Motion.Enabled)
        {
            done();
            return;
        }
        Motion.Fade(element, 0);
        // Faded, the card narrows to nothing with the gap after it, so the cards after it slide
        // up to close it on the standard token instead of jumping when it is taken out.
        _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
        {
            if (element is not FrameworkElement card || card.ActualWidth <= 0)
            {
                done();
                return;
            }
            var width = card.ActualWidth;
            Motion.Ease(card, width, 0, v =>
            {
                card.Width = Math.Max(0, v);
                card.Margin = new Thickness(0, 0, -HomeWording.Gap * (1 - v / width), 0);
            });
            _ = Task.Delay(Motion.Standard).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => done()));
        }));
    }

    public void Focus() => repeater.TryGetElement(0)?.Focus(FocusState.Programmatic);
}
