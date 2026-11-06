using System.Numerics;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Calendar: the week, Monday first, All airing or My list. Seven columns with today's raised from
/// heading to foot while the content is wide enough; an agenda of days below that. Each
/// broadcast to watch shows its banner at 16:9, its time first ("~16:30" for an estimate, "Moved
/// from 23:00" beside it when it moved), the show, and "E9 in 1 h 30 min" or "E8 out 1 d ago" with
/// " · Dubbed" for a dub and a water dot when it has aired and is not watched. A watched one is a
/// single quiet line, as on the Mac. The engine's window sets how far
/// the weeks reach; weeks beyond what is loaded page in as they are approached, and a silk hairline
/// marks the time now in today's column.
/// </summary>
public sealed partial class CalendarPage : Page
{
    const double WeekWidth = 760;

    readonly Ctx ctx = Ctx.Current;
    readonly TimeZoneInfo zone = TimeZoneInfo.Local;
    readonly CalendarReach reach = new();
    int week;
    bool mine, picked, settling;
    bool? wide;
    Entry[] rows = [];
    /// <summary>Whether the engine has answered once, and the 600 ms of silence before placeholders passed.</summary>
    bool loaded, placeholders, slow, waitingNow;
    DateTimeOffset placeholdersSince;
    string? failed;
    TextBlock? nowTime;
    long nowAt;
    /// <summary>The shown broadcasts' "in 1 h 30 min" lines and their buttons, kept to the clock by <see cref="Tick"/>.</summary>
    readonly List<(TextBlock Status, Button Button, Broadcast Broadcast)> live = [];

    /// <summary>
    /// The cells drawn last, by what they show. When the week is laid out again (a broadcast
    /// watched, the now line passing one, a page arriving) every cell that shows the same keeps
    /// its element, art and all, so nothing that did not change is drawn again or flashes.
    /// </summary>
    Dictionary<string, (FrameworkElement Cell, TextBlock? Status, Button Button, Broadcast Broadcast)> cells = [], lastCells = [];
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer silence, activity, minute;

    public CalendarPage()
    {
        mine = SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.CalendarMine));
        InitializeComponent();
        settling = true;
        Whose.SelectedItem = mine ? MineItem : AllItem;
        settling = false;
        silence = Timers.Weak(this, TimeSpan.FromMilliseconds(600), false, static page =>
        {
            page.placeholders = true;
            page.placeholdersSince = DateTimeOffset.UtcNow;
            page.Show();
        });
        activity = Timers.Weak(this, TimeSpan.FromSeconds(4), false, static page =>
        {
            page.slow = true;
            page.Show();
        });
        minute = Timers.Weak(this, TimeSpan.FromSeconds(20), true, static page => page.Tick());
        // The frame keeps the page for Back and the next visit: the week, the filter and the scroll stay.
        NavigationCacheMode = NavigationCacheMode.Enabled;
    }

    string TodayTip => T.CalendarTodayTooltip("Ctrl+T");

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        // Heard from each visit's start, before anything is asked, until the frame moves on.
        ctx.Hear(this, Heard, () =>
        {
            silence.Stop();
            activity.Stop();
            minute.Stop();
        });
        rows = ctx.Known.ListRows(new ContentFor.Calendar());
        loaded = ctx.Known.Lists.TryGetValue(new ContentFor.Calendar(), out var list) && list is ContentList.Entries or ContentList.Empty || rows.Length > 0;
        if (!loaded)
            FirstWait();
        minute.Start();
        ctx.Send(new Command.LoadCalendar());
        Show();
    }

    /// <summary>Asks again, past anything remembered: Try again on a failed first load, Ctrl+R.</summary>
    void Refresh()
    {
        failed = null;
        if (!loaded)
            FirstWait();
        ctx.Send(new Command.RefreshCalendar());
        Show();
    }

    /// <summary>The first week on its way: nothing for 600 ms, then plates, and the activity line past 4 s.</summary>
    void FirstWait()
    {
        placeholders = slow = false;
        silence.Stop();
        silence.Start();
        activity.Stop();
        activity.Start();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        var again = false;
        var arrived = false;
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Content { View: ContentFor.Calendar } c when c.ContentValue is not ContentList.Loading:
                    rows = ctx.Known.ListRows(new ContentFor.Calendar());
                    failed = c.ContentValue is ContentList.Failed f && rows.Length == 0 ? f.V1 : null;
                    var first = !loaded;
                    loaded = failed is null;
                    silence.Stop();
                    if (first)
                    {
                        // No page asks are out before the first week: the activity line was its own.
                        activity.Stop();
                        slow = false;
                        // Plates that showed stay 0.3 s and crossfade to the week.
                        arrived |= loaded && placeholders;
                    }
                    again = true;
                    break;
                case Event.CalendarWindow w:
                    reach.Window(w);
                    again = true;
                    break;
                case Event.CalendarBackfill b:
                    reach.Backfill(b.Page, b.MorePast);
                    if (b.Page is not null)
                        rows = [.. b.Entries, .. rows];
                    (again, arrived) = (true, true);
                    break;
                case Event.CalendarAhead a:
                    reach.Ahead(a.Page, a.MoreAhead);
                    if (a.Page is not null)
                        rows = [.. rows, .. a.Entries];
                    (again, arrived) = (true, true);
                    break;
                case Event.Settings when !picked:
                    var now = SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.CalendarMine));
                    if (now != mine)
                    {
                        mine = now;
                        settling = true;
                        Whose.SelectedItem = mine ? MineItem : AllItem;
                        settling = false;
                        again = true;
                    }
                    break;
                case Event.Progress:
                    again = true;
                    break;
            }
        }
        if (arrived)
            Arrived();
        else if (again)
            Show();
    }

    /// <summary>
    /// Pages came in: the placeholders, if they showed, stay at least 0.3 s, and the days then
    /// replace them in one standard crossfade.
    /// </summary>
    async void Arrived()
    {
        if (placeholders)
        {
            var left = TimeSpan.FromMilliseconds(300) - (DateTimeOffset.UtcNow - placeholdersSince);
            if (left > TimeSpan.Zero)
                await Task.Delay(left);
        }
        // Plates on screen give way to the days without a blank between; a page that only
        // fills weeks not shown changes nothing on screen.
        if (!plated)
        {
            Show();
            return;
        }
        // The days come up through what stood there from half strength, never from nothing.
        var before = shownKey;
        Show();
        if (shownKey != before && Motion.Enabled)
        {
            Days.Opacity = 0.5;
            Motion.FadeTo(Days, 1, Motion.Standard);
        }
    }

    /// <summary>Whether the days drawn last hold placeholder plates.</summary>
    bool plated;

    long Now => DateTimeOffset.UtcNow.ToUnixTimeSeconds();

    /// <summary>The loaded broadcasts' first and last instants, whoever's list they are on.</summary>
    (long? Oldest, long? Latest) Loaded(List<Broadcast> all) =>
        all.Count == 0 ? (null, null) : (all[0].Airs, all[^1].Airs);

    /// <summary>Asks for the weeks either side of the one shown when the loaded part ends within them.</summary>
    void Ask(DateOnly monday, long? oldest, long? latest, bool retry = false)
    {
        if (!loaded || failed is not null)
            return;
        if (reach.Past(CalendarReach.Start(monday.AddDays(-7), zone), oldest, retry) is { } past)
            ctx.Send(past);
        if (reach.Later(CalendarReach.Start(monday.AddDays(14), zone), latest, retry) is { } later)
            ctx.Send(later);
    }

    /// <summary>Starts or ends the 600 ms of silence and the 4 s of the activity line with the asks that are out.</summary>
    void Track()
    {
        var out_ = reach.AskingPast || reach.AskingAhead;
        if (out_ && !waitingNow)
        {
            placeholders = false;
            slow = false;
            silence.Stop();
            silence.Start();
            activity.Stop();
            activity.Start();
        }
        else if (!out_ && waitingNow)
        {
            silence.Stop();
            activity.Stop();
            placeholders = slow = false;
        }
        waitingNow = out_;
    }

    void Show()
    {
        var anchor = ctx.Known.CalendarAnchor ?? Now;
        var monday = Calendar.WeekStart(Now, week, zone);
        WeekTitle.Text = Calendar.WeekTitle(monday);
        ThisWeek.IsEnabled = week != 0;
        Earlier.IsEnabled = reach.CanGoEarlier(monday, zone);
        Later.IsEnabled = reach.CanGoLater(monday, zone);
        var (oldest, latest) = Loaded(Calendar.Broadcasts(rows, anchor, ctx.Known, false));
        var broadcasts = Calendar.Broadcasts(rows, anchor, ctx.Known, mine);
#if KAERI_TESTING
        // --show=calendar-marks: the fixture has no estimated or dubbed broadcast, so every other
        // row becomes an estimated dub, and the rest sub rows, plain, beside one, for a screenshot.
        if (ctx.Options.Show == "calendar-marks")
            broadcasts = [.. broadcasts.Select((b, i) => i % 2 == 0
                ? b with { Entry = b.Entry with { AiringApproximate = true, AiringTranslation = Kaeri.Engine.Core.Translation.Dub } }
                : b with { Entry = b.Entry with { AiringTranslation = Kaeri.Engine.Core.Translation.Sub } })];
#endif
        Ask(monday, oldest, latest);
        Track();
        var days = Calendar.Week(monday, broadcasts, zone);
        var today = Calendar.Day(Now, zone);
        // The engine answers a visit with the week the page already drew from what it knew, and
        // any progress anywhere says so too: drawn again only when what it shows has changed.
        // Its relative lines keep their own 20 s refresh (Tick).
        var willBeWide = Scroll.ActualWidth == 0 || Scroll.ActualWidth >= WeekWidth;
        var key = Key(monday, today, willBeWide, days, new Plan(this, days, oldest, latest));
        if (key == shownKey)
            return;
        shownKey = key;
        plated = false;
        lastCells = cells;
        cells = [];
        Days.Children.Clear();
        Days.ColumnDefinitions.Clear();
        Days.RowDefinitions.Clear();
        nowTime = null;
        live.Clear();
        wide =Scroll.ActualWidth == 0 || Scroll.ActualWidth >= WeekWidth;
        if (failed is not null)
        {
            Days.Children.Add(States.Unavailable(T.CalendarLoadFailed, T.StateCheckConnection, T.ActionTryAgain, Refresh, error: true));
            return;
        }
        if (!loaded && ctx.Known.Offline)
        {
            Days.Children.Add(States.Unavailable(T.StateOfflineTitle, T.StateOfflineDetail, T.StateOfflineShowDownloads, () => ctx.Window.Go("downloads")));
            return;
        }
        var plan = new Plan(this, days, oldest, latest);
        if (wide == true)
            ShowWeek(days, today, plan);
        else
            ShowAgenda(days, today, plan);
    }

    /// <summary>What the days were last drawn from.</summary>
    string? shownKey;

    /// <summary>Everything the drawn week depends on, as one string to compare.</summary>
    string Key(DateOnly monday, DateOnly today, bool wideNow, List<(DateOnly Day, List<Broadcast> Broadcasts)> days, Plan plan)
    {
        var key = new System.Text.StringBuilder();
        key.Append(monday).Append('|').Append(today).Append('|').Append(wideNow).Append('|').Append(failed?.ToString() ?? "")
            .Append('|').Append(loaded).Append(ctx.Known.Offline).Append(mine)
            .Append(plan.MineEmpty).Append(plan.PastFailed).Append(plan.AheadFailed).Append('|');
        // The plates and the activity line matter only while something is on its way: a loaded
        // week is not laid out again when a page for a week not shown is slow.
        if (!loaded || plan.Pending.Any(p => p))
            key.Append(placeholders).Append(slow);
        // The now line's place in today's column, and which lines carry the water dot, move
        // with the clock: the week is laid out again when they change, keeping its cells.
        var now = Now;
        key.Append('|').Append(days.Where(d => d.Day == today).Sum(d => d.Broadcasts.Count(b => b.Airs <= now))).Append('|');
        for (var i = 0; i < 7; i++)
            key.Append(plan.InRange[i] ? 'r' : '-').Append(plan.Pending[i] ? 'p' : '-').Append(plan.Past[i] ? 'b' : '-')
                .Append(plan.Beyond[i] ? 'a' : '-').Append(plan.End[i] ? 'e' : '-');
        foreach (var (_, list) in days)
        {
            key.Append('/');
            foreach (var b in list)
                key.Append(b.Entry.Id).Append(',').Append(b.Episode).Append(',').Append(b.Airs).Append(',').Append(b.Was)
                    .Append(',').Append(b.Watched).Append(',').Append(b.Dubbed).Append(',').Append(b.Entry.AiringApproximate)
                    .Append(',').Append(b.Entry.Title).Append(',').Append(b.Entry.BannerUrl ?? b.Entry.CoverUrl).Append(',').Append(b.IsNew(now)).Append(';');
        }
        return key.ToString();
    }

    /// <summary>What each day of the shown week is: inside the reach or not, loaded or waiting, and the lines that follow.</summary>
    sealed class Plan
    {
        public readonly bool[] InRange = new bool[7], Pending = new bool[7], PastSide = new bool[7], Past = new bool[7], Beyond = new bool[7], End = new bool[7];
        public readonly bool MineEmpty;
        public readonly bool PastFailed, AheadFailed;

        public Plan(CalendarPage page, List<(DateOnly Day, List<Broadcast> Broadcasts)> days, long? oldest, long? latest)
        {
            var reach = page.reach;
            var floor = reach.Known ? Calendar.Day(reach.Floor, page.zone) : DateOnly.MinValue;
            var ceiling = reach.Known ? Calendar.Day(reach.Ceiling, page.zone) : DateOnly.MaxValue;
            var latestDay = latest is { } l ? Calendar.Day(l, page.zone) : (DateOnly?)null;
            var any = false;
            for (var i = 0; i < 7; i++)
            {
                var day = days[i].Day;
                InRange[i] = reach.InRange(day, page.zone);
                Past[i] = day < floor;
                Beyond[i] = day > ceiling;
                var start = CalendarReach.Start(day, page.zone);
                Pending[i] = page.loaded && reach.Pending(start, CalendarReach.Start(day.AddDays(1), page.zone), oldest, latest);
                PastSide[i] = Pending[i] && oldest is { } o && CalendarReach.Start(day.AddDays(1), page.zone) <= o;
                End[i] = InRange[i] && !Pending[i] && page.loaded && !reach.MoreAhead && latestDay is { } d && day > d;
                any |= days[i].Broadcasts.Count > 0;
            }
            PastFailed = reach.PastFailed;
            AheadFailed = reach.AheadFailed;
            var open = Enumerable.Range(0, 7).Any(i => InRange[i] && !Pending[i] && !End[i]);
            MineEmpty = page.mine && page.loaded && !any && open && !Pending.Any(p => p);
        }

        public bool Failed(int i) => Pending[i] && (PastSide[i] ? PastFailed : AheadFailed);

        /// <summary>The days with nothing of their own to show: outside the reach, waiting, failed or past the end.</summary>
        public bool Plain(int i) => !InRange[i] || Pending[i] || End[i];
    }

    void ShowWeek(List<(DateOnly Day, List<Broadcast> Broadcasts)> days, DateOnly today, Plan plan)
    {
        Days.ColumnSpacing = 8;
        Days.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Days.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        for (var i = 0; i < 7; i++)
            Days.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        for (var i = 0; i < 7; i++)
        {
            var (day, broadcasts) = days[i];
            var column = new StackPanel { Spacing = 12, Padding = new Thickness(8, 10, 8, 16), CornerRadius = new CornerRadius(8) };
            if (day == today)
                column.Background = (Brush)Application.Current.Resources["CardBackgroundFillColorDefaultBrush"];
            column.Children.Add(DayHeading(day, today));
            if (!loaded)
            {
                // Never an empty day before the answer: nothing for 600 ms, then plates.
                if (placeholders)
                    for (var p = 0; p < (i % 3) + 1; p++)
                        column.Children.Add(CellPlaceholder());
            }
            else if (plan.Pending[i])
            {
                // A week moved to while its page is on the way: plates at once, in the cells'
                // own size, so the week that slides in is never blank.
                if (!plan.Failed(i))
                    for (var p = 0; p < (i % 3) + 1; p++)
                        column.Children.Add(CellPlaceholder());
            }
            else if (!plan.Plain(i))
            {
                if (broadcasts.Count == 0 && !plan.MineEmpty)
                    column.Children.Add(Haze(T.CalendarDayEmpty));
                AddBroadcasts(column, broadcasts, day == today, true);
            }
            Grid.SetColumn(column, i);
            Days.Children.Add(column);
        }
        // One line spans each run of days that cannot show their own, under their headings.
        Run(plan, i => plan.Past[i], () => Line(T.CalendarFloor));
        Run(plan, i => plan.Beyond[i], () => Line(T.CalendarCeiling));
        Run(plan, i => plan.End[i], () => Line(T.CalendarEnd));
        Run(plan, i => plan.Failed(i) && plan.PastSide[i], () => Failure(T.CalendarLoadEarlierFailed));
        Run(plan, i => plan.Failed(i) && !plan.PastSide[i], () => Failure(T.CalendarLoadLaterFailed));
        if (plan.MineEmpty)
            Run(plan, i => !plan.Plain(i), MineEmptyLine);
        // The first week past 4 s: one line under all seven days' plates.
        if (!loaded && placeholders && slow)
            Run(plan, _ => true, () => Activity(T.StateLoading), row: 1);
        if (slow)
        {
            Run(plan, i => plan.Pending[i] && !plan.Failed(i) && plan.PastSide[i], () => Activity(T.CalendarLoadingEarlier), row: 1);
            Run(plan, i => plan.Pending[i] && !plan.Failed(i) && !plan.PastSide[i], () => Activity(T.CalendarLoadingLater), row: 1);
        }
    }

    /// <summary>Adds an element across each run of consecutive days <paramref name="pick"/> chooses.</summary>
    void Run(Plan plan, Func<int, bool> pick, Func<FrameworkElement> make, int row = 0)
    {
        for (var i = 0; i < 7; i++)
        {
            if (!pick(i))
                continue;
            var to = i;
            while (to + 1 < 7 && pick(to + 1))
                to++;
            var element = make();
            if (row == 0)
                element.Margin = new Thickness(8, 10 + 20 + 12, 8, 0);
            Grid.SetRow(element, row);
            Grid.SetColumn(element, i);
            Grid.SetColumnSpan(element, to - i + 1);
            element.VerticalAlignment = VerticalAlignment.Top;
            Days.Children.Add(element);
            i = to;
        }
    }

    static FrameworkElement Line(string text)
    {
        var line = Haze(text);
        line.TextWrapping = TextWrapping.WrapWholeWords;
        return line;
    }

    static FrameworkElement Activity(string text)
    {
        var line = Haze(text);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(line, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        line.Margin = new Thickness(8, 4, 8, 0);
        return line;
    }

    /// <summary>One line with Try again where the days that could not load would be; vermilion on the glyph only.</summary>
    FrameworkElement Failure(string text)
    {
        var panel = new StackPanel { Spacing = 4, HorizontalAlignment = HorizontalAlignment.Left };
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        line.Children.Add(States.ErrorGlyph(12));
        var words = Haze(text);
        words.TextWrapping = TextWrapping.WrapWholeWords;
        line.Children.Add(words);
        panel.Children.Add(line);
        var retry = new HyperlinkButton { Content = T.ActionTryAgain, Padding = new Thickness(0), Foreground = (Brush)Application.Current.Resources["SilkBrush"] };
        retry.Click += (_, _) => Retry();
        panel.Children.Add(retry);
        return panel;
    }

    /// <summary>"Nothing on your list airs this week." with the way to All airing, in place of the days' own lines.</summary>
    FrameworkElement MineEmptyLine()
    {
        var panel = new StackPanel { Spacing = 4, HorizontalAlignment = HorizontalAlignment.Left };
        var words = Haze(T.CalendarEmptyMine);
        words.TextWrapping = TextWrapping.WrapWholeWords;
        panel.Children.Add(words);
        var all = new HyperlinkButton { Content = T.CalendarShowAll, Padding = new Thickness(0), Foreground = (Brush)Application.Current.Resources["SilkBrush"] };
        all.Click += (_, _) => ShowAll();
        panel.Children.Add(all);
        return panel;
    }

    void Retry()
    {
        var monday = Calendar.WeekStart(Now, week, zone);
        var (oldest, latest) = Loaded(Calendar.Broadcasts(rows, ctx.Known.CalendarAnchor ?? Now, ctx.Known, false));
        Ask(monday, oldest, latest, retry: true);
        Show();
    }

    void ShowAll()
    {
        Pick(false);
        settling = true;
        Whose.SelectedItem = AllItem;
        settling = false;
    }

    /// <summary>The filter chosen: remembered in the setting, and shown.</summary>
    void Pick(bool onlyMine)
    {
        mine = onlyMine;
        picked = true;
        ctx.Send(new Command.SetSetting(SettingKey.CalendarMine, new SettingValue.Bool(onlyMine)));
        Show();
    }

    /// <summary>
    /// A day's broadcasts in order. Today's column carries the silk hairline of the time now,
    /// between the last that aired and the next, with the time in 11 semibold.
    /// </summary>
    void AddBroadcasts(Panel into, List<Broadcast> broadcasts, bool isToday, bool grid)
    {
        var now = Now;
        var line = isToday && broadcasts.Count > 0;
        var placed = false;
        foreach (var b in broadcasts)
        {
            if (line && !placed && b.Airs > now)
            {
                into.Children.Add(NowLine());
                placed = true;
            }
            into.Children.Add(Kept(b, grid));
        }
        if (line && !placed)
            into.Children.Add(NowLine());
    }

    /// <summary>The cell for <paramref name="b"/>: the one drawn last when it shows the same, else a new one.</summary>
    FrameworkElement Kept(Broadcast b, bool grid)
    {
        var now = Now;
        var key = $"{grid}|{b.Entry.Id}|{b.Episode}|{b.Airs}|{b.Was}|{b.Watched}|{b.Dubbed}|{b.Entry.AiringApproximate}|{b.Entry.Title}|{b.Entry.BannerUrl ?? b.Entry.CoverUrl}|{b.IsNew(now)}";
        if (lastCells.Remove(key, out var kept) && !cells.ContainsKey(key))
        {
            if (kept.Cell.Parent is Panel parent)
                parent.Children.Remove(kept.Cell);
            cells[key] = kept;
            if (kept.Status is { } status)
                live.Add((status, kept.Button, b));
            return kept.Cell;
        }
        var count = live.Count;
        var made = grid ? Cell(b) : AgendaRow(b);
        var (line, button) = live.Count > count ? (live[^1].Status, live[^1].Button) : ((TextBlock?)null, (Button)((made as Button) ?? new Button()));
        cells[key] = (made, line, button, b);
        return made;
    }

    FrameworkElement NowLine()
    {
        nowAt = Now;
        var time = Calendar.Time(nowAt, zone);
        nowTime = new TextBlock
        {
            Text = time,
            FontSize = 11,
            FontWeight = FontWeights.SemiBold,
            Foreground = (Brush)Application.Current.Resources["SilkBrush"],
            VerticalAlignment = VerticalAlignment.Center,
        };
        var line = new Grid { ColumnSpacing = 6, Height = 18 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.Children.Add(nowTime);
        var rule = new Rectangle { Height = 1, Fill = (Brush)Application.Current.Resources["SilkBrush"], VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(rule, 1);
        line.Children.Add(rule);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(line, T.CalendarNowSpoken(time));
        return line;
    }

    /// <summary>
    /// Each 20 s: the broadcasts' lines and names to the clock, the time on the hairline, and a
    /// rebuild when a broadcast has passed it.
    /// </summary>
    void Tick()
    {
        if (!loaded)
            return;
        var at = Now;
        foreach (var (status, button, b) in live)
        {
            var words = b.Status(at);
            if (status.Text != words)
                status.Text = words;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, b.Spoken(at, zone));
        }
        if (week != 0)
        {
            Show();
            return;
        }
        if (nowTime is not null)
            nowTime.Text = Calendar.Time(Now, zone);
        // A broadcast the now line has passed, or a line that gained its water dot: the week is
        // laid out again with the cells it has, so only the line moves and the dot shows.
        Show();
    }

    void ShowAgenda(List<(DateOnly Day, List<Broadcast> Broadcasts)> days, DateOnly today, Plan plan)
    {
        Days.ColumnSpacing = 0;
        var list = new StackPanel { Spacing = 20 };
        // Today first, then the days after it, then the ones before.
        var index = Enumerable.Range(0, 7).OrderBy(i => days[i].Day < today ? 1 : 0).ThenBy(i => days[i].Day).ToList();
        var ceilingDone = false;
        foreach (var i in index)
        {
            var (day, broadcasts) = days[i];
            if (!ceilingDone && days[i].Day < today && plan.Beyond.Any(b => b))
            {
                list.Children.Add(Line(T.CalendarCeiling));
                ceilingDone = true;
            }
            if (!plan.InRange[i])
                continue;
            if (plan.End[i])
                continue;
            if (plan.Pending[i])
            {
                var section = new StackPanel { Spacing = 8 };
                section.Children.Add(DayHeading(day, today));
                if (plan.Failed(i))
                    section.Children.Add(Failure(plan.PastSide[i] ? T.CalendarLoadEarlierFailed : T.CalendarLoadLaterFailed));
                else
                {
                    section.Children.Add(States.Rows(1, 96, 54));
                    plated = true;
                }
                if (slow && !plan.Failed(i))
                    section.Children.Add(Activity(plan.PastSide[i] ? T.CalendarLoadingEarlier : T.CalendarLoadingLater));
                list.Children.Add(section);
                continue;
            }
            if (broadcasts.Count == 0 && day != today && loaded)
                continue;
            var body = new StackPanel { Spacing = 8 };
            body.Children.Add(DayHeading(day, today));
            if (!loaded)
            {
                if (placeholders)
                {
                    body.Children.Add(States.Rows(1, 96, 54));
                    plated = true;
                }
            }
            else if (broadcasts.Count == 0)
            {
                if (plan.MineEmpty)
                    body.Children.Add(MineEmptyLine());
                else
                    body.Children.Add(Haze(T.CalendarEmptyToday));
            }
            AddBroadcasts(body, broadcasts, day == today, false);
            list.Children.Add(body);
        }
        if (!ceilingDone && plan.Beyond.Any(b => b))
            list.Children.Add(Line(T.CalendarCeiling));
        if (plan.End.Any(b => b))
            list.Children.Add(Line(T.CalendarEnd));
        if (plan.Past.Any(b => b))
            list.Children.Add(Line(T.CalendarFloor));
        if (!loaded && placeholders && slow)
            list.Children.Add(Activity(T.StateLoading));
        Days.Children.Add(list);
    }

    /// <summary>The day in the system's locale; today in silk and the others in haze, by weight not colour.</summary>
    static TextBlock DayHeading(DateOnly day, DateOnly today)
    {
        var culture = System.Globalization.CultureInfo.CurrentCulture;
        var text = day == today ? T.CalendarTodayHeading(day.ToString("ddd d", culture)) : day.ToString("ddd d", culture);
        var heading = new TextBlock
        {
            Text = text,
            Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"],
        };
        if (day != today)
        {
            heading.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
            heading.FontWeight = Microsoft.UI.Text.FontWeights.Normal;
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        return heading;
    }

    /// <summary>A cell while its week loads: the banner, then the time, the title and the line, where they will stand.</summary>
    StackPanel CellPlaceholder()
    {
        plated = true;
        var cell = new StackPanel { Spacing = 8 };
        cell.Children.Add(Wide(new Border { CornerRadius = new CornerRadius(6), Background = Ui.Res<Brush>("RaisedBrush") }));
        var lines = new StackPanel { Spacing = 4 };
        lines.Children.Add(States.Plate(40, 12, 3));
        lines.Children.Add(States.Plate(90, 12, 3));
        lines.Children.Add(States.Plate(70, 10, 3));
        cell.Children.Add(lines);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(cell, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return cell;
    }

    /// <summary>Keeps <paramref name="box"/> at 16:9 of the width its column gives it.</summary>
    static T Wide<T>(T box) where T : FrameworkElement
    {
        box.SizeChanged += (_, e) =>
        {
            var height = Math.Round(e.NewSize.Width * 9 / 16);
            if (e.NewSize.Width > 0 && (double.IsNaN(box.Height) || Math.Abs(box.Height - height) > 0.5))
                box.Height = height;
        };
        return box;
    }

    static TextBlock Haze(string text) => new() { Text = text, Style = (Style)Application.Current.Resources["HazeCaptionStyle"] };

    static Button BroadcastButton() => new()
    {
        Padding = new Thickness(0),
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        BorderThickness = new Thickness(0),
        HorizontalAlignment = HorizontalAlignment.Stretch,
        HorizontalContentAlignment = HorizontalAlignment.Stretch,
    };

    /// <summary>
    /// A broadcast in a day's column. One to watch has its banner at 16:9, then the time in silk
    /// with any move beside it in haze, the show in up to two lines, and a haze line with the
    /// water dot when it is new: "E8 out 1 d 2 h ago", "E9 in 1 h 30 min". One watched is a
    /// single quiet line.
    /// </summary>
    FrameworkElement Cell(Broadcast b)
    {
        var button = BroadcastButton();
        button.Click += (_, _) => ctx.OpenTitle(b.Entry.Id);
        HoverPreview.Attach(button, b.Entry.Id);
        if (b.Watched)
        {
            // Watched: one quiet line, a dub still marked.
            var quiet = T.CalendarWatchedLine(b.Entry.Title, b.EpisodeText);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, T.CalendarWatchedSpoken(b.Entry.Title, b.EpisodeText));
            button.Content = new TextBlock { Text = quiet, Style = (Style)Application.Current.Resources["HazeCaptionStyle"] };
            return button;
        }
        var now = Now;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, b.Spoken(now, zone));
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(Wide(Banner(b)));
        var text = new StackPanel { Spacing = 2 };
        text.Children.Add(ClockLine(b));
        text.Children.Add(TitleLine(b, 2));
        text.Children.Add(StatusLine(b, now, button));
        panel.Children.Add(text);
        button.Content = panel;
        return button;
    }

    /// <summary>A broadcast in the agenda: the banner at 16:9 beside the same three lines as a cell.</summary>
    FrameworkElement AgendaRow(Broadcast b)
    {
        var button = BroadcastButton();
        var now = Now;
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var banner = Banner(b);
        banner.Width = 96;
        banner.Height = 54;
        banner.VerticalAlignment = VerticalAlignment.Top;
        grid.Children.Add(banner);
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Spacing = 2 };
        text.Children.Add(ClockLine(b));
        text.Children.Add(TitleLine(b, 1));
        text.Children.Add(StatusLine(b, now, button));
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        button.Content = grid;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, b.Spoken(now, zone));
        button.Click += (_, _) => ctx.OpenTitle(b.Entry.Id);
        HoverPreview.Attach(button, b.Entry.Id);
        return button;
    }

    /// <summary>The show's banner, or its cover where it has none, filling a rounded plate of its colour.</summary>
    Grid Banner(Broadcast b)
    {
        var banner = new Grid { CornerRadius = new CornerRadius(6), Background = new CoverItem(b.Entry).Placeholder };
        var image = new Image { Stretch = Stretch.UniformToFill, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        banner.Children.Add(image);
        ctx.Art.Bind(b.Entry.BannerUrl ?? b.Entry.CoverUrl, s => image.Source = s);
        return banner;
    }

    /// <summary>"16:30" in silk, "~16:30" for an estimate, with "Moved from 23:00" beside it in haze when it moved.</summary>
    TextBlock ClockLine(Broadcast b)
    {
        var line = new TextBlock
        {
            FontSize = 13,
            LineHeight = 16,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Foreground = Ui.Res<Brush>("SilkBrush"),
        };
        Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(line, FontNumeralAlignment.Tabular);
        line.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = b.Clock(zone) });
        if (b.DelayNote(zone) is { } note)
            line.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = " " + note, FontSize = 12, Foreground = Ui.Res<Brush>("HazeBrush") });
        return line;
    }

    static TextBlock TitleLine(Broadcast b, int lines) => new()
    {
        Text = b.Entry.Title,
        Style = Ui.Res<Style>("ShowTitleStyle"),
        FontSize = 13,
        LineHeight = 17,
        MaxLines = lines,
        TextWrapping = lines > 1 ? TextWrapping.Wrap : TextWrapping.NoWrap,
        Foreground = Ui.Res<Brush>("SilkBrush"),
    };

    /// <summary>
    /// "E8 out 1 d 2 h ago" or "E9 in 1 h 30 min" in haze, up to two lines, after the water dot
    /// when it has aired and is not watched. The dot sits on the first line. The words and the
    /// button's name follow the clock while the page is up.
    /// </summary>
    FrameworkElement StatusLine(Broadcast b, long now, Button button)
    {
        var line = new Grid { ColumnSpacing = 4 };
        var fresh = b.IsNew(now);
        if (fresh)
        {
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            line.Children.Add(new Ellipse
            {
                Width = 6,
                Height = 6,
                Fill = Ui.Res<Brush>("WaterBrush"),
                VerticalAlignment = VerticalAlignment.Top,
                Margin = new Thickness(0, 5, 0, 0),
            });
        }
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var status = new TextBlock
        {
            Text = b.Status(now),
            FontSize = 12,
            LineHeight = 16,
            MaxLines = 2,
            TextWrapping = TextWrapping.WrapWholeWords,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Foreground = Ui.Res<Brush>("HazeBrush"),
        };
        Grid.SetColumn(status, fresh ? 1 : 0);
        line.Children.Add(status);
        live.Add((status, button, b));
        return line;
    }

    void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var nowWide = e.NewSize.Width >= WeekWidth;
        if (nowWide != wide)
            Show();
    }

    void OnWhoseChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs e)
    {
        if (settling)
            return;
        Pick(sender.SelectedItem?.Tag as string == "mine");
    }

    /// <summary>Moves to a week with the old one sliding out: one width on the standard token, towards the side it went.</summary>
    void Go(int to)
    {
        var step = Math.Sign(to - week);
        if (step == 0)
            return;
        week = to;
        Show();
        Motion.FadeIn(WeekTitle, Motion.Standard);
        var width = (float)Scroll.ActualWidth;
        if (!Motion.Enabled || width <= 0)
        {
            Motion.FadeIn(Days, Motion.Standard);
            return;
        }
        Days.TranslationTransition = null;
        Days.Translation = new Vector3(step * width, 0, 0);
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
        {
            Days.TranslationTransition = new Vector3Transition { Duration = Motion.Standard };
            Days.Translation = Vector3.Zero;
        });
    }

    void OnEarlier(object sender, RoutedEventArgs e) => Go(week - 1);

    void OnLater(object sender, RoutedEventArgs e) => Go(week + 1);

    void OnThisWeek(object sender, RoutedEventArgs e) => Go(0);

    /// <summary>
    /// Whether a text box has the keys, the title bar's search among them: Ctrl+Left and
    /// Ctrl+Right move by words there, and do not page the weeks.
    /// </summary>
    bool Typing() => XamlRoot is { } root && FocusManager.GetFocusedElement(root) is TextBox or PasswordBox or AutoSuggestBox;

    void OnAccelToday(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Typing())
            return;
        args.Handled = true;
        if (ThisWeek.IsEnabled)
            Go(0);
    }

    void OnAccelEarlier(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Typing())
            return;
        args.Handled = true;
        if (Earlier.IsEnabled)
            Go(week - 1);
    }

    void OnAccelLater(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Typing())
            return;
        args.Handled = true;
        if (Later.IsEnabled)
            Go(week + 1);
    }

    void OnAccelRefresh(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Typing())
            return;
        args.Handled = true;
        Refresh();
    }
}
