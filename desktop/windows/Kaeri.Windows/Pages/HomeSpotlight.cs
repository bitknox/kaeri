using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Size = Windows.Foundation.Size;
using Rect = Windows.Foundation.Rect;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The spotlight: one title, its text beside the art (56% of the width, 16:9, radius 16) with
/// no scrim and no gradient, or under the art when the window is narrow. A change of title or
/// case crossfades in place between two faces.
/// </summary>
public sealed partial class HomeSpotlight : Grid
{
    /// <summary>Narrower than this the art sits above the text.</summary>
    const double SideBySide = 860;

    HomePage page = null!;
    SpotFace? shown;

    public HomeSpotlight()
    {
        Visibility = Visibility.Collapsed;
    }

    /// <summary>The page whose actions the buttons run.</summary>
    internal void Use(HomePage owner) => page = owner;

    public uint? Id => shown?.Spot.Entry.Id;

    /// <summary>The main action's button, for the first focus and for a test.</summary>
    public Button? Main => shown?.Main;

    /// <summary>Draws <paramref name="spot"/>; with <paramref name="crossfade"/> the old face fades out as the new one fades in.</summary>
    public void Show(Spotlight? spot, bool crossfade)
    {
        if (spot is null)
        {
            if (shown is { } gone)
            {
                shown = null;
                Motion.Hide(gone);
                Visibility = Visibility.Collapsed;
            }
            return;
        }
        var face = new SpotFace(page, spot);
        var old = shown;
        shown = face;
        Children.Add(face);
        if (old is not null && crossfade && Motion.Enabled)
        {
            // The spotlight changing case: a crossfade in place on standard.
            face.Opacity = 0;
            Motion.Crossfade(old, face);
            _ = Task.Delay(Motion.Standard).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                // Both faces share the cell while they cross, so it is as tall as the taller.
                // With the old one gone the cell eases to the new face's height, and the
                // shelves under it glide rather than jump.
                var before = ActualHeight;
                Children.Remove(old);
                if (shown == face && face.ActualHeight > 0)
                    Motion.Height(this, before, face.ActualHeight);
            }));
        }
        else
        {
            if (old is not null)
                Children.Remove(old);
            if (Visibility != Visibility.Visible)
                Motion.FadeIn(this);
        }
        Visibility = Visibility.Visible;
    }

    /// <summary>The planned state of the shown title changed (Adding, then On your Planning list).</summary>
    public void Refresh() => shown?.Words();

    /// <summary>
    /// The same title in the same case, as the engine now has it: an episode watched, a sources
    /// check answered. The face's words, buttons and menu take it in place, with no crossfade,
    /// and the art and focus stay.
    /// </summary>
    public void Update(Spotlight spot)
    {
        if (shown is { } face && face.Spot.Entry.Id == spot.Entry.Id)
            face.Take(spot);
        else
            Show(spot, crossfade: true);
    }

    sealed partial class SpotFace : Grid
    {
        readonly HomePage page;
        readonly ColumnDefinition textColumn = new() { Width = new GridLength(1, GridUnitType.Star) };
        readonly ColumnDefinition gap = new() { Width = new GridLength(40) };
        readonly ColumnDefinition artColumn = new() { Width = GridLength.Auto };
        readonly RowDefinition artRow = new() { Height = GridLength.Auto };
        readonly RowDefinition textRow = new() { Height = GridLength.Auto };
        readonly Border art;
        readonly StackPanel text = new() { VerticalAlignment = VerticalAlignment.Center };

        // The words, made once and changed in place when a newer record says something else.
        readonly TextBlock label = new() { Style = Ui.Res<Style>("HazeBodyStyle"), FontSize = 14, LineHeight = 19, Margin = new Thickness(0, 0, 0, 8) };
        readonly TextBlock title = new()
        {
            Style = Ui.Res<Style>("ShowTitleDisplayStyle"),
            MaxLines = 2,
            TextWrapping = TextWrapping.Wrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        readonly TextBlock secondary = new() { Style = Ui.Res<Style>("HazeCaptionStyle"), Margin = new Thickness(0, 4, 0, 0) };
        readonly TextBlock facts = new()
        {
            Style = Ui.Res<Style>("HazeCaptionStyle"),
            CharacterSpacing = 80,
            FontWeight = FontWeights.Medium,
            Margin = new Thickness(0, 10, 0, 0),
        };
        readonly TextBlock synopsis = new()
        {
            Style = Ui.Res<Style>("HazeBodyStyle"),
            FontSize = 14,
            LineHeight = 20,
            MaxLines = 3,
            MaxWidth = 440,
            HorizontalAlignment = HorizontalAlignment.Left,
            TextWrapping = TextWrapping.Wrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Margin = new Thickness(0, 12, 0, 0),
        };
        /// <summary>What the synopsis was filled from, so it is filled again only when that changes.</summary>
        (string?, SynopsisSpan[]?)? synopsisFrom;
        readonly StackPanel lineRow = new() { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 16, 0, 0) };
        readonly Border track = new() { Width = 200, Height = 3, CornerRadius = new CornerRadius(2), VerticalAlignment = VerticalAlignment.Center };
        readonly Border fill = new() { Height = 3, HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(2) };
        readonly FrameworkElement dot = Ui.Dot(Colours.Water, false);
        readonly TextBlock lineText = new() { Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.Wrap, MaxLines = 2, VerticalAlignment = VerticalAlignment.Center };

        /// <summary>
        /// The actions: main, Details or Add to Planning, Trailer when there is one, and More. One
        /// row while it fits; else the main action over the others, never squeezed beside them.
        /// </summary>
        readonly ActionRow buttons = new() { Spacing = 10, Margin = new Thickness(0, 18, 0, 0), HorizontalAlignment = HorizontalAlignment.Left };
        readonly TextBlock notFound = new() { Text = T.HomeNotFound, Style = Ui.Res<Style>("HazeCaptionStyle"), Margin = new Thickness(0, 12, 0, 0) };
        // 40 high, as the title page's and the hover card's actions are: the main one amber and semibold.
        readonly Button main = new() { Style = Ui.Res<Style>("PrimaryActionStyle") };
        readonly Button second = new() { MinHeight = 40 };
        readonly Button trailer = new() { MinHeight = 40, Content = T.TitleTrailer };
        readonly Button more = new() { Width = 40, Height = 40, Padding = new Thickness(0), Content = new FontIcon { Glyph = "", FontSize = 16 } };
        bool wide = true;

        public Spotlight Spot { get; private set; }
        public Button Main => main;

        public SpotFace(HomePage page, Spotlight spot)
        {
            this.page = page;
            Spot = spot;
            ColumnDefinitions.Add(textColumn);
            ColumnDefinitions.Add(gap);
            ColumnDefinitions.Add(artColumn);
            RowDefinitions.Add(textRow);
            RowDefinitions.Add(artRow);

            var cover = new CoverItem(spot.Entry).Load(Ctx.Current.Art, withBanner: true);
            art = HomeArt.Make(cover, HomeWording.ArtMaxWidth, HomeWording.ArtMaxWidth * 9 / 16, 16, wide: true);
            art.Tapped += (_, _) => page.OpenTitle(Spot.Entry.Id);
            AutomationProperties.SetAccessibilityView(art, AccessibilityView.Raw);

            AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
            track.Background = Ui.Res<Brush>("RuleBrush");
            fill.Background = Ui.Res<Brush>("SilkBrush");
            track.Child = fill;
            dot.Width = dot.Height = 6;
            dot.VerticalAlignment = VerticalAlignment.Center;
            lineRow.Children.Add(track);
            lineRow.Children.Add(dot);
            lineRow.Children.Add(lineText);
            text.Children.Add(label);
            text.Children.Add(title);
            text.Children.Add(secondary);
            text.Children.Add(facts);
            text.Children.Add(synopsis);
            text.Children.Add(lineRow);
            buttons.Children.Add(main);
            buttons.Children.Add(second);
            buttons.Children.Add(trailer);
            buttons.Children.Add(more);
            text.Children.Add(buttons);
            text.Children.Add(notFound);
            AutomationProperties.SetName(more, T.HomeMore);
            ToolTipService.SetToolTip(more, T.HomeMore);
            // More is made as it opens, from the record shown then.
            var menu = new MenuFlyout();
            menu.Opening += (_, _) => FillMore(menu);
            more.Flyout = menu;
            main.Click += (_, _) => page.MainAction(Spot, Effective());
            second.Click += (_, _) => page.SecondAction(Spot, Secondary());
            trailer.Click += (_, _) => TrailerWindow.Play(Ctx.Current, Spot.Entry);
            Say();
            Children.Add(text);
            Children.Add(art);
            Arrange(true);
            SizeChanged += (_, e) =>
            {
                var side = e.NewSize.Width >= SideBySide;
                if (side != wide)
                    Arrange(side);
                SizeArt(e.NewSize.Width);
            };
            AutomationProperties.SetLocalizedLandmarkType(text, T.HomeSpotlightContinuing);
        }

        void Arrange(bool side)
        {
            wide = side;
            textColumn.Width = new GridLength(1, GridUnitType.Star);
            gap.Width = side ? new GridLength(40) : new GridLength(0);
            artColumn.Width = side ? GridLength.Auto : new GridLength(0);
            Grid.SetColumn(text, 0);
            Grid.SetRow(text, side ? 0 : 1);
            Grid.SetColumn(art, side ? 2 : 0);
            Grid.SetColumnSpan(art, side ? 1 : 3);
            Grid.SetRow(art, 0);
            text.Margin = new Thickness(0, side ? 0 : 16, 0, 0);
            if (!side)
                art.HorizontalAlignment = HorizontalAlignment.Left;
            // Stacked, as on phones: the main action full width, the others in a row under it.
            buttons.Full = !side;
            buttons.HorizontalAlignment = side ? HorizontalAlignment.Left : HorizontalAlignment.Stretch;
            // A button keeps to its own width unless told to fill what it is given.
            main.HorizontalAlignment = side ? HorizontalAlignment.Left : HorizontalAlignment.Stretch;
        }

        void SizeArt(double width)
        {
            // Stacked, the art takes the column's whole width, with nothing beside it.
            var w = wide ? Math.Min(HomeWording.ArtMaxWidth, Math.Round(width * HomeWording.ArtShare)) : Math.Round(width);
            if (w <= 0 || Math.Abs(art.Width - w) < 0.5)
                return;
            art.Width = w;
            art.Height = Math.Round(w * 9 / 16);
        }

        /// <summary>Takes a newer record of the same title: only what it says differently changes.</summary>
        public void Take(Spotlight spot)
        {
            if (spot == Spot)
                return;
            Spot = spot;
            Say();
        }

        static void Set(TextBlock block, string? words)
        {
            var text = words ?? "";
            if (block.Text != text)
                block.Text = text;
            var visibility = text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            if (block.Visibility != visibility)
                block.Visibility = visibility;
        }

        /// <summary>The words from the record, each set only where it changed.</summary>
        void Say()
        {
            var s = Spot;
            var entry = s.Entry;
            AutomationProperties.SetName(text, HomeWording.Name(s));
            Set(label, s.Label.Text());
            Set(title, entry.Title);
            Set(secondary, entry.Secondary is { Length: > 0 } other && other != entry.Title ? other : null);
            var line = HomeWording.Facts(entry);
            if (s.Starts is { } starts)
                line = string.Join(" · ", new[] { line, T.UpNextStarts(UpNextWording.Date(starts, DateTimeOffset.Now)) }.Where(x => x.Length > 0));
            Set(facts, line.ToUpper(System.Globalization.CultureInfo.CurrentCulture));
            if (synopsisFrom is not { } from || from.Item1 != entry.Synopsis || !ReferenceEquals(from.Item2, entry.SynopsisSpans))
            {
                synopsisFrom = (entry.Synopsis, entry.SynopsisSpans);
                SynopsisText.Fill(synopsis, entry);
                synopsis.Visibility = synopsis.Inlines.Count > 0 || synopsis.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            }

            // Under the synopsis: the track and the time left, where another device stopped, or
            // the new episode with the water dot.
            if (HomeWording.Line(s) is { } words)
            {
                var resume = s.Resume?.Fraction is not null;
                track.Visibility = resume ? Visibility.Visible : Visibility.Collapsed;
                fill.Width = 200 * Math.Clamp(s.Resume?.Fraction ?? 0, 0, 1);
                dot.Visibility = resume ? Visibility.Collapsed : Visibility.Visible;
                lineRow.Spacing = resume ? 12 : 6;
                lineText.Foreground = Ui.Res<Brush>(resume ? "HazeBrush" : "SilkBrush");
                if (lineText.Text != words)
                    lineText.Text = words;
                lineRow.Visibility = Visibility.Visible;
            }
            else
                lineRow.Visibility = Visibility.Collapsed;

            trailer.Visibility = entry.TrailerUrl is not null ? Visibility.Visible : Visibility.Collapsed;
            notFound.Visibility = s.Playable == Playable.NotFound ? Visibility.Visible : Visibility.Collapsed;
            Words();
        }

        bool Listed => Spot.Entry.ListStatus is not null || Spot.Case == SpotlightCase.Continuing;

        HomeAction Effective() => HomeWording.Effective(Spot.Action, Spot.Playable, Listed);

        /// <summary>What the second button is: Details, Add to Planning, or the state it is in once added.</summary>
        SpotSecond Secondary()
        {
            var planned = page.PlannedHere.Contains(Spot.Entry.Id);
            if (Effective() == HomeAction.AddToPlanning && planned)
                return SpotSecond.Planned;
            if (Spot.Case == SpotlightCase.Continuing || Effective() != HomeAction.Play)
                return SpotSecond.Details;
            if (planned)
                return SpotSecond.Planned;
            return Spot.Reason == UpNextReason.Planning ? SpotSecond.Details : SpotSecond.Add;
        }

        /// <summary>The words on the buttons, from the engine's action and what was done here.</summary>
        public void Words()
        {
            var action = Effective();
            var planned = page.PlannedHere.Contains(Spot.Entry.Id);
            var label = HomeWording.SpotlightAction(Spot.Action, Spot.Episode, Spot.Playable, Listed);
            if (action == HomeAction.AddToPlanning && planned)
                label = null;
            main.Visibility = label is null ? Visibility.Collapsed : Visibility.Visible;
            if (label is not null && main.Tag as string != $"{action}|{label}")
            {
                main.Tag = $"{action}|{label}";
                var icon = action == HomeAction.AddToPlanning ? "" : "";
                main.Content = action == HomeAction.Details ? label : Button(icon, label);
                AutomationProperties.SetName(main, label);
            }
            var s = Secondary();
            var (glyph, words) = s switch
            {
                SpotSecond.Add => ("", T.UpNextAddPlanning),
                SpotSecond.Planned => ("", page.AddingHere.Contains(Spot.Entry.Id) ? T.HomeAdding : T.UpNextReasonPlanning),
                _ => ("", T.HomeDetails),
            };
            if (second.Tag as string != $"{glyph}|{words}")
            {
                second.Tag = $"{glyph}|{words}";
                second.Content = glyph.Length > 0 ? Button(glyph, words) : words;
                AutomationProperties.SetName(second, words);
            }
            second.IsEnabled = s != SpotSecond.Planned;
            // When Details is the main action, it is not said twice.
            second.Visibility = action == HomeAction.Details ? Visibility.Collapsed : Visibility.Visible;
        }

        static StackPanel Button(string glyph, string words) => new()
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { new FontIcon { Glyph = glyph, FontSize = 12 }, new TextBlock { Text = words } },
        };

        void FillMore(MenuFlyout menu)
        {
            menu.Items.Clear();
            var s = Spot;
            if (s.Case == SpotlightCase.Continuing)
            {
                // Only an episode there is to play is offered as watched, as on the Mac.
                if (s.Action is HomeAction.Resume or HomeAction.Play)
                {
                    var watched = new MenuFlyoutItem { Text = T.HomeMarkWatched(s.Episode), Icon = new FontIcon { Glyph = "" } };
                    watched.Click += (_, _) => page.MarkWatched(s.Entry.Id, s.Episode);
                    menu.Items.Add(watched);
                }
                var remove = new MenuFlyoutItem { Text = T.HomeRemoveContinue, Icon = new FontIcon { Glyph = "" }, KeyboardAcceleratorTextOverride = "Delete" };
                remove.Click += (_, _) => page.RemoveFromContinue(s.Entry.Id);
                menu.Items.Add(remove);
            }
            else
            {
                var no = new MenuFlyoutItem { Text = T.UpNextNotInterested, Icon = new FontIcon { Glyph = "" } };
                no.Click += (_, _) => page.NotInterested(s.Entry, s.Starts is not null);
                menu.Items.Add(no);
            }
            menu.Items.Add(new MenuFlyoutSeparator());
            var details = new MenuFlyoutItem { Text = T.HomeDetails, Icon = new FontIcon { Glyph = "" } };
            details.Click += (_, _) => page.OpenTitle(s.Entry.Id);
            menu.Items.Add(details);
        }
    }
}

/// <summary>What the spotlight's second button is.</summary>
enum SpotSecond
{
    Details,
    Add,
    Planned,
}

/// <summary>
/// A row of buttons side by side while they fit; when they do not, the first goes on a line of
/// its own over the rest, as the Mac's spotlight does, so a long label is never clipped.
/// </summary>
sealed partial class ActionRow : Panel
{
    public double Spacing { get; set; } = 10;

    /// <summary>The stacked spotlight's layout: the first button the row's whole width, the rest under it.</summary>
    public bool Full
    {
        get => full;
        set
        {
            if (full == value)
                return;
            full = value;
            InvalidateMeasure();
        }
    }

    bool stacked, full;

    protected override Size MeasureOverride(Size availableSize)
    {
        double firstWidth = 0, firstHeight = 0, restWidth = 0, restHeight = 0;
        var shown = 0;
        foreach (var child in Children)
        {
            child.Measure(new Size(double.PositiveInfinity, availableSize.Height));
            if (child.Visibility != Visibility.Visible)
                continue;
            var size = child.DesiredSize;
            if (shown == 0)
                (firstWidth, firstHeight) = (size.Width, size.Height);
            else
            {
                restWidth += (shown > 1 ? Spacing : 0) + size.Width;
                restHeight = Math.Max(restHeight, size.Height);
            }
            shown++;
        }
        var row = firstWidth + (shown > 1 ? Spacing + restWidth : 0);
        if (full && !double.IsInfinity(availableSize.Width))
        {
            stacked = shown > 1;
            var width = Math.Max(availableSize.Width, Math.Max(firstWidth, restWidth));
            return new Size(width, firstHeight + (stacked ? Spacing + restHeight : 0));
        }
        stacked = shown > 1 && row > availableSize.Width;
        return stacked
            ? new Size(Math.Max(firstWidth, restWidth), firstHeight + Spacing + restHeight)
            : new Size(row, Math.Max(firstHeight, restHeight));
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        double x = 0, y = 0;
        var index = 0;
        foreach (var child in Children)
        {
            if (child.Visibility != Visibility.Visible)
            {
                child.Arrange(new Rect(0, 0, 0, 0));
                continue;
            }
            var size = child.DesiredSize;
            var width = full && index == 0 ? finalSize.Width : size.Width;
            child.Arrange(new Rect(x, y, width, size.Height));
            if (stacked && index == 0)
            {
                x = 0;
                y += size.Height + Spacing;
            }
            else
                x += size.Width + Spacing;
            index++;
        }
        return finalSize;
    }
}
