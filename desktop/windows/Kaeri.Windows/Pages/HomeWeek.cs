using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// This week: seven columns from today, each a day's heading and up to three titles (today's around now) with their
/// air time and episode, then "N more" which opens the calendar. A newer week changes it in place:
/// each entry is kept by its title and episode, so only what changed is drawn, and the air times
/// ("Out 2 h ago") keep to the clock while Home is up.
/// </summary>
sealed partial class HomeWeek : Grid
{
    const double ColumnMin = 150;

    readonly HomePage page;
    readonly TextBlock title;
    readonly HyperlinkButton link;
    readonly Grid strip;
    readonly StackPanel[] columns = new StackPanel[7];
    readonly TextBlock[] days = new TextBlock[7];
    readonly TextBlock[] nothing = new TextBlock[7];
    readonly HyperlinkButton[] mores = new HyperlinkButton[7];
    /// <summary>The entries drawn, by title and episode, each changed in place.</summary>
    Dictionary<string, WeekEntry> entries = [];
    HomeSection section;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer clock;

    public HomeWeek(HomePage page, HomeSection section)
    {
        this.page = page;
        this.section = section;
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var head = new Grid { Margin = new Thickness(0, 0, 0, 12) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        title = new TextBlock { Style = Ui.Res<Style>("SectionTitleStyle"), TextWrapping = TextWrapping.NoWrap };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
        head.Children.Add(title);
        link = new HyperlinkButton { Style = Ui.Res<Style>("SilkLinkStyle") };
        link.Click += (_, _) => page.OpenViewAll(SectionKind.ThisWeek, this.section.ViewAll);
        Grid.SetColumn(link, 1);
        head.Children.Add(link);
        Children.Add(head);

        strip = new Grid
        {
            ColumnSpacing = 16,
            BorderBrush = Ui.Res<Brush>("FaintRuleBrush"),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(0, 12, 0, 0),
            MinWidth = 7 * ColumnMin + 6 * 16,
        };
        for (var d = 0; d < 7; d++)
        {
            strip.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            columns[d] = new StackPanel { Spacing = 12 };
            days[d] = new TextBlock { FontSize = 12, LineHeight = 16, FontWeight = FontWeights.SemiBold };
            AutomationProperties.SetHeadingLevel(days[d], AutomationHeadingLevel.Level3);
            nothing[d] = Ui.Text(T.HomeNothingAiring, "HazeCaptionStyle");
            mores[d] = new HyperlinkButton { Style = Ui.Res<Style>("SilkLinkCaptionStyle") };
            mores[d].Click += (_, _) => page.OpenViewAll(SectionKind.ThisWeek, null);
            Grid.SetColumn(columns[d], d);
            strip.Children.Add(columns[d]);
        }
        var scroll = new ScrollViewer
        {
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollMode = ScrollMode.Enabled,
            IsHorizontalRailEnabled = true,
            VerticalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            IsTabStop = false,
            Content = strip,
        };
        // A horizontal ScrollViewer measures its content with no width limit, which turns the
        // equal columns into ones as wide as their longest title. The strip takes the width the
        // page gives it (at least its minimum), so the days share it and a long title wraps.
        scroll.SizeChanged += (_, e) => strip.Width = Math.Max(strip.MinWidth, e.NewSize.Width);
        HomeShelf.ForwardWheel(scroll);
        Grid.SetRow(scroll, 1);
        Children.Add(scroll);

        // The air times keep to the clock while the week is up: each half minute, the words
        // that changed change, and nothing else.
        clock = Timers.Weak(this, TimeSpan.FromSeconds(30), true, static week => week.Tick());
        Loaded += (_, _) =>
        {
            Tick();
            clock.Start();
        };
        Unloaded += (_, _) => clock.Stop();
        Update(section);
    }

    /// <summary>Takes the week as the engine now has it, changing only what differs.</summary>
    public void Update(HomeSection week)
    {
        section = week;
        var heading = HomeWording.Heading(week).ToString();
        if (title.Text != heading)
            title.Text = heading;
        var all = HomeWording.ViewAll(week);
        if (all is not null && link.Tag as string != all)
        {
            link.Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Children = { new TextBlock { Text = all, FontWeight = FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
            };
            AutomationProperties.SetName(link, all);
        }
        link.Tag = all;
        link.Visibility = all is null ? Visibility.Collapsed : Visibility.Visible;

        var byDay = HomeWording.ByDay(week);
        var now = DateTimeOffset.Now;
        var kept = new Dictionary<string, WeekEntry>();
        for (var d = 0; d < 7; d++)
        {
            var dayName = HomeWording.Day((uint)d, now);
            if (days[d].Text != dayName)
                days[d].Text = dayName;
            days[d].Foreground = Ui.Res<Brush>(d == 0 ? "SilkBrush" : "HazeBrush");
            var want = new List<UIElement> { days[d] };
            if (byDay[d].Count == 0)
                want.Add(nothing[d]);
            foreach (var item in d == 0 ? HomeWording.Window(byDay[d], now.ToUnixTimeSeconds(), 3) : byDay[d].Take(3))
            {
                var key = $"{item.Entry.Id}|{item.Episode}";
                if (!entries.TryGetValue(key, out var entry) && !kept.TryGetValue(key, out entry))
                    entry = new WeekEntry(page);
                entry.Take(item, now);
                kept[key] = entry;
                want.Add(entry.Button);
            }
            if (byDay[d].Count > 3)
            {
                var more = T.HomeDayMore(byDay[d].Count - 3);
                if (mores[d].Content as string != more)
                    mores[d].Content = more;
                want.Add(mores[d]);
            }
            // An entry moving to another day leaves its old column first.
            foreach (var element in want)
                if (element is FrameworkElement { Parent: Panel parent } && parent != columns[d])
                    parent.Children.Remove(element);
            Drawn.Arrange(columns[d].Children, want);
        }
        entries = kept;
    }

    void Tick()
    {
        var now = DateTimeOffset.Now;
        foreach (var entry in entries.Values)
            entry.Tick(now);
    }

    /// <summary>One title on a day: its art, its air time, its name and its episode, as a button with the Home card menu.</summary>
    sealed class WeekEntry
    {
        readonly HomePage page;
        readonly TextBlock time, name, episode;
        readonly FrameworkElement dot;
        readonly Grid row;
        HomeItem? item;
        HomeCard? card;

        public Button Button { get; }

        public WeekEntry(HomePage page)
        {
            this.page = page;
            row = new Grid { ColumnSpacing = 8, VerticalAlignment = VerticalAlignment.Top };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var text = new StackPanel { VerticalAlignment = VerticalAlignment.Top };
            var when = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            dot = Ui.Dot(Colours.Water, false);
            when.Children.Add(dot);
            time = new TextBlock { FontSize = 12, LineHeight = 16, FontWeight = FontWeights.SemiBold, TextTrimming = TextTrimming.CharacterEllipsis };
            when.Children.Add(time);
            text.Children.Add(when);
            name = new TextBlock { Style = Ui.Res<Style>("ShowTitleSmallStyle"), MaxLines = 2, TextWrapping = TextWrapping.Wrap, TextTrimming = TextTrimming.CharacterEllipsis };
            text.Children.Add(name);
            episode = new TextBlock { Style = Ui.Res<Style>("HazeCaptionStyle") };
            text.Children.Add(episode);
            Grid.SetColumn(text, 1);
            row.Children.Add(text);
            Button = new Button { Style = Ui.Res<Style>("HomeCardButtonStyle"), Content = row, HorizontalAlignment = HorizontalAlignment.Stretch };
            Button.Click += (_, _) =>
            {
                if (item is { } shown)
                    Ctx.Current.OpenTitle(shown.Entry.Id);
            };
            // The Home card menu, as on the Mac: play, Add to Planning, Details.
            Button.ContextFlyout = new MenuFlyout();
            ((MenuFlyout)Button.ContextFlyout).Opening += (sender, _) =>
            {
                var menu = (MenuFlyout)sender!;
                if (card is { } c)
                    page.Fill(menu, c, HomeShape.Week);
            };
        }

        /// <summary>Takes the engine's item, changing only the words that differ.</summary>
        public void Take(HomeItem now, DateTimeOffset at)
        {
            var was = item;
            item = now;
            if (was is null || was.Entry.Id != now.Entry.Id || was.Entry.CoverUrl != now.Entry.CoverUrl)
            {
                var cover = new CoverItem(now.Entry).Load(Ctx.Current.Art);
                if (row.Children.Count > 1)
                    row.Children.RemoveAt(0);
                row.Children.Insert(0, HomeArt.Make(cover, 44, 66, Art.Radius(44), false));
                card = new HomeCard(now, cover, "", "");
            }
            else if (card is { } c)
                card = new HomeCard(now, c.Cover, "", "");
            if (name.Text != now.Entry.Title)
                name.Text = now.Entry.Title;
            var words = now.Episode is { } ep ? T.EpisodeNumber(ep) : "";
            if (episode.Text != words)
                episode.Text = words;
            dot.Visibility = now.New ? Visibility.Visible : Visibility.Collapsed;
            Tick(at);
        }

        /// <summary>The air time to the clock, and the spoken name with it, with no empty parts.</summary>
        public void Tick(DateTimeOffset now)
        {
            if (item is not { } shown)
                return;
            var when = shown.AirsAt is { } at ? HomeWording.When(at, now, TimeZoneInfo.Local) : "";
            if (time.Text != when)
                time.Text = when;
            AutomationProperties.SetName(Button, string.Join(", ", new[] { shown.Entry.Title, episode.Text, when }.Where(s => s.Length > 0)));
        }
    }
}
