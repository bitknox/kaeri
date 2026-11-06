using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Home: the spotlight, then the engine's sections in the engine's order, as one page. The
/// engine decides which title the spotlight holds, which sections there are, what each holds and
/// what every line says; this draws it, and sends back what is done (play, Add to Planning, Not
/// interested, Mark as watched). A section that is still on its way gets placeholders from
/// 600 ms in its own place, so nothing above it moves when it lands.
/// </summary>
public sealed partial class HomePage : Page, IPageKeys
{
    readonly Ctx ctx = Ctx.Current;
    readonly Dictionary<SectionKind, FrameworkElement> shown = [];
    readonly DateTimeOffset opened = DateTimeOffset.Now;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer silence, activity;
    HomeView? drawn;
    HomeView? complained;
    uint? probed;
    bool placeholders;
    DateTimeOffset placeholdersSince;
    StackPanel? waitingPlates;

    internal ScrollViewer PageScroll => Scroll;

    /// <summary>Titles put on Planning from here, until the page goes: their button says so at once.</summary>
    internal HashSet<uint> PlannedHere { get; } = [];

    /// <summary>Titles whose Add to Planning is taking longer than 600 ms.</summary>
    internal HashSet<uint> AddingHere { get; } = [];

    public HomePage()
    {
        InitializeComponent();
        Spot.Use(this);
        State.Placeholders = FirstPlates;
        State.ActivityText = T.HomeLoading;
        silence = Timers.Weak(this, TimeSpan.FromMilliseconds(HomeWording.PlaceholderAfterMs), false, static page => page.Show());
        activity = Timers.Weak(this, TimeSpan.FromMilliseconds(HomeWording.ActivityAfterMs), false, static page => page.ShowActivity());
        // The frame keeps Home for Back and for the next visit, so it comes back where it was:
        // its scroll, its shelves' places, and nothing drawn again that has not changed.
        NavigationCacheMode = NavigationCacheMode.Enabled;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        // Heard from each visit's start, before anything is asked, until the frame moves on. The
        // shelves stay as they are while another page is up, so leaving shows no change.
        ctx.Hear(this, Heard, () =>
        {
            silence.Stop();
            activity.Stop();
        });
        ctx.Send(new Command.LoadHome(false));
        silence.Start();
        activity.Start();
        Show();
    }

    void OnRefresh(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        ctx.Send(new Command.LoadHome(true));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        var again = false;
        foreach (var e in batch)
            switch (e)
            {
                case Event.Home:
                    again = true;
                    // The engine has answered: whatever is waiting on it is done.
                    AddingHere.Clear();
                    Spot.Refresh();
                    break;
                case Event.Sync:
                case Event.Connectivity:
                    again = true;
                    break;
            }
        if (again)
            Show();
    }

    // Drawing

    void Show()
    {
        ShowSyncLine();
        var known = ctx.Known.Home;
        if (known is null)
        {
            State.Wait();
            return;
        }
        var home = Drawn(known);
        ShowOffline(home);
        var empty = home.Spotlight is null && home.Sections.Length == 0;
        if (empty && home.Offline && home.OfflineSince is null)
        {
            State.Empty(States.Empty(T.HomeOfflineEmptyTitle, T.HomeOfflineEmptyBody, T.HomeOpenDownloads, () => ctx.Window.Go("downloads")));
            return;
        }
        if (empty && home.Pending.Length > 0 && !placeholders && !SilenceOver)
        {
            State.Wait();
            return;
        }
        State.Done();

        var before = drawn;
        drawn = home;
        var moved = before is null || home.Spotlight is null || before.Spotlight is null
            ? before?.Spotlight?.Entry.Id != home.Spotlight?.Entry.Id
            : before.Spotlight.Entry.Id != home.Spotlight.Entry.Id || before.Spotlight.Case != home.Spotlight.Case;
        var fresh = Spot.Id is null;
        if (before is null || moved || Spot.Id != home.Spotlight?.Entry.Id)
            Spot.Show(home.Spotlight, crossfade: !fresh);
        else if (home.Spotlight is { } same && !ReferenceEquals(before.Spotlight, same))
            Spot.Update(same);
        else
            Spot.Refresh();
        if (home.Spotlight is { } spotlight && probed != spotlight.Entry.Id)
        {
            probed = spotlight.Entry.Id;
            ctx.Send(new Command.ProbeTitle(spotlight.Entry.Id));
        }

        Reconcile(home);
        ShowPlaceholders(home);
        ShowEnd(home);
        if (home.Failed && !ReferenceEquals(complained, known))
        {
            complained = known;
            ctx.Window.Say(T.HomeFailed, T.HomeTryAgain, () => ctx.Send(new Command.LoadHome(true)));
        }
        if (home.Pending.Length == 0)
        {
            activity.Stop();
            Motion.Hide(Activity);
        }
    }

    bool SilenceOver => DateTimeOffset.Now - opened >= TimeSpan.FromMilliseconds(HomeWording.PlaceholderAfterMs);

    /// <summary>
    /// Home as the engine sent it, less what was removed here and is not yet gone from the
    /// engine's: Not interested anywhere, Remove from Continue on Continue and its spotlight. A
    /// later send that still holds such a title cannot bring its card back.
    /// </summary>
    HomeView Drawn(HomeView home)
    {
        var dismissed = ctx.Known.DismissedHere;
        var hidden = ctx.Known.HiddenFromContinue;
        if (dismissed.Count == 0 && hidden.Count == 0)
            return home;
        bool Gone(uint id, bool continuing) => dismissed.Contains(id) || (continuing && hidden.Contains(id));
        var spot = home.Spotlight is { } s && Gone(s.Entry.Id, s.Case == SpotlightCase.Continuing) ? null : home.Spotlight;
        var sections = home.Sections
            .Select(section =>
            {
                var kept = section.Items.Where(i => !Gone(i.Entry.Id, section.Kind == SectionKind.Continue)).ToArray();
                return kept.Length == section.Items.Length ? section
                    : section with { Items = kept, Total = (uint)Math.Max(0, (long)section.Total - (section.Items.Length - kept.Length)) };
            })
            .Where(section => section.Items.Length > 0)
            .ToArray();
        return home with { Spotlight = spot, Sections = sections };
    }

    void ShowSyncLine()
    {
        var waiting = ctx.Known.Anilist?.Outbox ?? 0;
        if (waiting > 0)
            SyncLine.Text = T.ContinueSyncWaiting(waiting);
        // It fades in place: the words stay while it fades out.
        if ((waiting > 0) != (SyncLine.Opacity > 0.5))
            Motion.Fade(SyncLine, waiting > 0 ? 1 : 0);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(SyncLine,
            waiting > 0 ? Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Content : Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
    }

    /// <summary>"Offline. Showing Home from 14:20." over a kept Home; the plain offline line over what is only local.</summary>
    void ShowOffline(HomeView home)
    {
        var on = home.Offline || ctx.Known.Offline;
        OfflineText.Text = home.OfflineSince is { } since ? HomeWording.OfflineLine(since, TimeZoneInfo.Local) : T.StateOfflineLine;
        if (on && OfflineLine.Visibility != Visibility.Visible)
            Motion.Show(OfflineLine);
        else if (!on && OfflineLine.Visibility == Visibility.Visible)
            Motion.Hide(OfflineLine);
    }

    /// <summary>
    /// Takes the engine's sections in its order. A section that is already up is updated in place;
    /// one that left fades out and closes the gap; one that arrived fades in where it belongs.
    /// </summary>
    void Reconcile(HomeView home)
    {
        var kinds = home.Sections.Select(s => s.Kind).ToHashSet();
        foreach (var kind in shown.Keys.Where(k => !kinds.Contains(k)).ToList())
            Remove(kind);
        var index = 0;
        foreach (var section in home.Sections)
        {
            if (shown.TryGetValue(section.Kind, out var element))
            {
                Update(element, section, home);
                if (Shelves.Children.IndexOf(shown[section.Kind]) != index)
                {
                    Shelves.Children.Remove(shown[section.Kind]);
                    Shelves.Children.Insert(Math.Min(index, Shelves.Children.Count), shown[section.Kind]);
                }
            }
            else
            {
                var made = Make(section, home);
                shown[section.Kind] = made;
                Shelves.Children.Insert(Math.Min(index, Shelves.Children.Count), made);
                if (drawn is not null && Spot.Id is not null)
                    Motion.FadeIn(made, Motion.Standard);
            }
            index++;
        }
        FirstMargin();
    }

    /// <summary>
    /// Every shelf sits 40 under what is above it; the first, with no spotlight, has nothing to
    /// clear. Set for all of them each time, since a shelf that was first once (a week drawn
    /// before Continue arrived) is not first later and needs its gap back.
    /// </summary>
    void FirstMargin()
    {
        for (var i = 0; i < Shelves.Children.Count; i++)
        {
            if (Shelves.Children[i] is not FrameworkElement shelf)
                continue;
            var top = i == 0 && Spot.Visibility != Visibility.Visible ? 0 : HomeWording.ShelfGap;
            if (shelf.Margin.Top != top)
                shelf.Margin = new Thickness(0, top, 0, 0);
        }
    }

    FrameworkElement Make(HomeSection section, HomeView home) =>
        HomeWording.Shape(section.Kind) == HomeShape.Week
            ? new HomeWeek(this, section) { Tag = WeekKey(section) }
            : new HomeShelf(this, section, home.Season);

    void Update(FrameworkElement element, HomeSection section, HomeView home)
    {
        if (element is not HomeShelf shelf)
        {
            // This week changes in place, and only when what it shows has changed.
            var key = WeekKey(section);
            if (element.Tag as string == key)
                return;
            element.Tag = key;
            if (element is HomeWeek week)
                week.Update(section);
            return;
        }
        var now = section.Items.Select(i => i.Entry.Id).ToHashSet();
        var leaving = shelf.Cards.Select(c => c.Id).Where(id => !now.Contains(id)).ToList();
        if (leaving.Count == 0)
        {
            shelf.Update(section, home.Season, []);
            return;
        }
        // Cards that left fade out, then the ones after them close the gap.
        var left = leaving.Count;
        foreach (var id in leaving)
            shelf.Leave(id, () =>
            {
                if (--left == 0 && shown.TryGetValue(section.Kind, out var current) && current == shelf)
                    shelf.Update(drawn?.Sections.FirstOrDefault(s => s.Kind == section.Kind) ?? section, drawn?.Season, []);
            });
    }

    /// <summary>What This week draws, so it is drawn again only when that changes, or the day does.</summary>
    static string WeekKey(HomeSection section) =>
        DateTime.Today.ToString("yyyy-MM-dd", System.Globalization.CultureInfo.InvariantCulture) + "|"
        + string.Join(";", section.Items.Select(i => $"{i.Entry.Id},{i.Entry.Title},{i.Episode},{i.AirsAt},{i.Day},{i.New},{i.Playable}"));

    void Remove(SectionKind kind)
    {
        var element = shown[kind];
        shown.Remove(kind);
        if (!Motion.Enabled)
        {
            Shelves.Children.Remove(element);
            return;
        }
        // Fades out, then the shelves below close the gap on the standard token.
        var height = element.ActualHeight;
        Motion.Fade(element, 0);
        _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
        {
            Motion.Ease(element, height, 0, v => element.Height = Math.Max(0, v));
            _ = Task.Delay(Motion.Standard).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Shelves.Children.Remove(element)));
        }));
    }

    // Placeholders and the activity line

    void ShowPlaceholders(HomeView home)
    {
        var want = home.Pending.Length > 0 && SilenceOver;
        if (want && !placeholders)
        {
            placeholders = true;
            placeholdersSince = DateTimeOffset.Now;
            waitingPlates = new StackPanel();
            foreach (var _ in home.Pending.Where(p => p != HomePart.Week).DefaultIfEmpty(HomePart.Week))
                waitingPlates.Children.Add(Plates(true));
            if (home.Pending.Contains(HomePart.Week))
                waitingPlates.Children.Add(Plates(false));
            Waiting.Children.Add(waitingPlates);
            Motion.FadeIn(waitingPlates);
        }
        else if (!want && placeholders && waitingPlates is { } plates)
        {
            placeholders = false;
            waitingPlates = null;
            var left = TimeSpan.FromMilliseconds(HomeWording.PlaceholderLeastMs) - (DateTimeOffset.Now - placeholdersSince);
            // One crossfade, held at least 0.3 s, and the plates are the real shelf's size.
            _ = Task.Delay(left > TimeSpan.Zero ? left : TimeSpan.Zero).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                // Faded, the plates' place closes on the standard token, so what is under them glides up.
                Motion.Fade(plates, 0);
                _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
                {
                    Motion.Ease(plates, plates.ActualHeight, 0, v => plates.Height = Math.Max(0, v));
                    _ = Task.Delay(Motion.Standard).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Waiting.Children.Remove(plates)));
                }));
            }));
        }
    }

    /// <summary>A heading bar and a row of plates at the final size of a poster shelf, or of the week.</summary>
    static StackPanel Plates(bool posters) => Plates(posters ? HomeShape.Poster : HomeShape.Week);

    /// <summary>
    /// A shelf's plates at its final size: the heading's line, then each card's art and, under a
    /// card or a poster, its title and its line, each the height of the text that replaces it.
    /// </summary>
    static StackPanel Plates(HomeShape shape)
    {
        var panel = new StackPanel { Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0) };
        panel.Children.Add(Line(160, 20, 28, 12));
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = HomeWording.Gap };
        for (var i = 0; i < 7; i++)
        {
            if (shape == HomeShape.Week)
            {
                row.Children.Add(States.Plate(150, 66, Art.Radius(44)));
                continue;
            }
            var width = shape == HomeShape.Card ? HomeWording.CardWidth : HomeWording.PosterWidth;
            var card = new StackPanel { Width = width };
            card.Children.Add(States.Plate(width, shape == HomeShape.Card ? HomeWording.CardHeight : HomeWording.PosterHeight, Art.Radius(width)));
            if (shape == HomeShape.Card)
                card.Children.Add(new Border { Height = 9 });
            card.Children.Add(new Border { Height = 8 });
            card.Children.Add(Line(width * 0.8, 12, 19, 0));
            card.Children.Add(Line(width * 0.55, 10, 16, 0));
            row.Children.Add(card);
        }
        panel.Children.Add(row);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(panel, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return panel;
    }

    /// <summary>A bar in a line of text's height, so the plate holds the text's place.</summary>
    static Border Line(double width, double bar, double height, double below) => new()
    {
        Height = height,
        Margin = new Thickness(0, 0, 0, below),
        Child = new Border
        {
            Width = width,
            Height = bar,
            CornerRadius = new CornerRadius(3),
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Center,
            Background = Ui.Res<Brush>("RaisedBrush"),
        },
    };

    /// <summary>Home's first load: the spotlight's place, a Continue shelf and a poster shelf, in Home's own layout.</summary>
    UIElement FirstPlates()
    {
        var column = new StackPanel { Padding = new Thickness(0, 8, 36, 48) };
        var spot = new Grid { ColumnSpacing = 40 };
        spot.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        spot.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(Line(120, 12, 19, 8));
        words.Children.Add(Line(320, 26, 38, 4));
        words.Children.Add(Line(220, 10, 16, 10));
        for (var i = 0; i < 3; i++)
            words.Children.Add(Line(i == 2 ? 260 : 420, 12, 20, 0));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 34, 0, 0) };
        buttons.Children.Add(States.Plate(160, 32, 4));
        buttons.Children.Add(States.Plate(90, 32, 4));
        words.Children.Add(buttons);
        spot.Children.Add(words);
        // The art at the share of the width the spotlight gives it.
        var art = States.Plate(HomeWording.ArtMaxWidth, HomeWording.ArtMaxWidth * 9 / 16, 16);
        Grid.SetColumn(art, 1);
        spot.Children.Add(art);
        spot.SizeChanged += (_, e) =>
        {
            var w = Math.Min(HomeWording.ArtMaxWidth, Math.Round(e.NewSize.Width * HomeWording.ArtShare));
            if (w > 0 && Math.Abs(art.Width - w) >= 0.5)
            {
                art.Width = w;
                art.Height = Math.Round(w * 9 / 16);
            }
        };
        column.Children.Add(spot);
        column.Children.Add(Plates(HomeShape.Card));
        column.Children.Add(Plates(HomeShape.Poster));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(column, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return column;
    }

    void ShowActivity()
    {
        if (ctx.Known.Home is { Pending.Length: > 0 })
        {
            Activity.Text = T.HomeLoading;
            Motion.Show(Activity);
        }
    }

    // The closing line

    void ShowEnd(HomeView home)
    {
        var key = home.End?.ToString();
        if ((EndLine.Tag as string) == key)
            return;
        EndLine.Tag = key;
        EndLine.Children.Clear();
        if (home.End is not { } end)
            return;
        var line = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 16,
            Margin = new Thickness(0, 32, 0, 0),
            BorderBrush = Ui.Res<Brush>("FaintRuleBrush"),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(0, 16, 0, 0),
        };
        switch (end)
        {
            case HomeEnd.SignIn:
                line.Children.Add(Ui.Haze(T.HomeEndSignedOut, wrap: false));
                var signIn = new Button { Content = T.HomeSignIn, MinHeight = 32 };
                signIn.Click += (_, _) => ctx.SignIn("anilist");
                line.Children.Add(signIn);
                break;
            case HomeEnd.NoSources:
                line.Children.Add(Linked(T.HomeEndNoSources, (T.NavPluginStores, () => ctx.Window.Go("stores"))));
                break;
            default:
                line.Children.Add(Linked(T.HomeEnd, (T.NavSeasonal, () => ctx.Window.Go("seasonal")), (T.NavCalendar, () => ctx.Window.Go("calendar"))));
                break;
        }
        EndLine.Children.Add(line);
        Motion.FadeIn(line);
    }

    /// <summary>A haze sentence with some of its words as links.</summary>
    static TextBlock Linked(string sentence, params (string Words, Action Go)[] links)
    {
        var block = new TextBlock { Style = Ui.Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.Wrap };
        var rest = sentence;
        while (rest.Length > 0)
        {
            var next = links.Select(l => (l, At: rest.IndexOf(l.Words, StringComparison.CurrentCulture))).Where(x => x.At >= 0).OrderBy(x => x.At).Cast<((string Words, Action Go) L, int At)?>().FirstOrDefault();
            if (next is not { } hit)
            {
                block.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = rest });
                break;
            }
            if (hit.At > 0)
                block.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = rest[..hit.At] });
            var link = new Microsoft.UI.Xaml.Documents.Hyperlink();
            link.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = hit.L.Words });
            var go = hit.L.Go;
            link.Click += (_, _) => go();
            block.Inlines.Add(link);
            rest = rest[(hit.At + hit.L.Words.Length)..];
        }
        return block;
    }

    // What a card or the spotlight does

    internal void OpenTitle(uint id) => ctx.OpenTitle(id);

    /// <summary>A click: a Continue card plays what is next; a poster opens its title.</summary>
    internal void OpenCard(HomeCard card, HomeShape shape)
    {
        if (shape == HomeShape.Card && card.Item.Action is HomeAction.Resume or HomeAction.Play && card.Item.Episode is { } episode)
            ctx.Play(card.Id, episode.ToString());
        else
            ctx.OpenTitle(card.Id);
    }

    internal void OpenViewAll(SectionKind kind, HomeLink? link)
    {
        switch (link)
        {
            case HomeLink.Watching:
                ctx.Window.Go("library");
                break;
            case HomeLink.SeasonalView:
                ctx.Window.Go("seasonal");
                break;
            case HomeLink.Browse browse:
                ctx.Window.ShowBrowse(browse.Query);
                break;
            default:
                ctx.Window.Go(kind == SectionKind.ThisWeek || link is HomeLink.CalendarView ? "calendar" : "seasonal");
                break;
        }
    }

    internal void MainAction(Spotlight spot, HomeAction action)
    {
        switch (action)
        {
            case HomeAction.Resume or HomeAction.Play:
                ctx.Play(spot.Entry.Id, spot.Episode.ToString());
                break;
            case HomeAction.AddToPlanning:
                AddToPlanning(spot.Entry.Id);
                break;
            case HomeAction.Details:
                ctx.OpenTitle(spot.Entry.Id);
                break;
        }
    }

    internal void SecondAction(Spotlight spot, SpotSecond what)
    {
        if (what == SpotSecond.Add)
            AddToPlanning(spot.Entry.Id);
        else if (what == SpotSecond.Details)
            ctx.OpenTitle(spot.Entry.Id);
    }

    /// <summary>Add to Planning: "Adding" only if saving takes 600 ms, then "On your Planning list" with a check.</summary>
    internal void AddToPlanning(uint id)
    {
        ctx.Send(new Command.SetStatus(id, LibrarySegment.Planning));
        ctx.Known.SetStatus(id, LibrarySegment.Planning);
        ctx.Known.PlannedFromUpNext(id);
        PlannedHere.Add(id);
        AddingHere.Add(id);
        Spot.Refresh();
        _ = Task.Delay(HomeWording.PlaceholderAfterMs).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
        {
            AddingHere.Remove(id);
            Spot.Refresh();
        }));
    }

    internal void MarkWatched(uint id, uint episode)
    {
        ctx.Send(new Command.SetWatched(id, [episode.ToString()], true));
        ctx.Window.Say(T.HomeMarkedWatched(episode));
    }

    /// <summary>Remove from Continue: the card fades and the gap closes at once, ahead of the engine's next Home.</summary>
    internal void RemoveFromContinue(uint id)
    {
        ctx.Known.HiddenFromContinue.Add(id);
        ctx.Send(new Command.HideFromContinue(id));
        Show();
    }

    /// <summary>
    /// Not interested: the title leaves every shelf at once, and a notice says so with Undo, for
    /// 8 s. The engine sends the sections that held it again from what it kept.
    /// </summary>
    internal void NotInterested(Entry entry, bool untilAirs)
    {
        var id = entry.Id;
        ctx.Known.DismissedHere.Add(id);
        ctx.Send(new Command.DismissUpNext(id));
        Show();
        ctx.Window.Say(untilAirs ? T.UpNextHiddenUntilAirs(entry.Title) : T.UpNextHidden(entry.Title), T.ActionUndo, () =>
        {
            ctx.Known.DismissedHere.Remove(id);
            ctx.Send(new Command.RestoreUpNext(id));
            Show();
        });
    }

    /// <summary>A card's menu, made as it opens, so it says what is true then: Add to Planning goes once the title is on a list.</summary>
    internal MenuFlyout MenuFor(HomeCard card, HomeShape shape)
    {
        var menu = new MenuFlyout();
        menu.Opening += (_, _) => Fill(menu, card, shape);
        return menu;
    }

    internal void Fill(MenuFlyout menu, HomeCard card, HomeShape shape)
    {
        menu.Items.Clear();
        var item = card.Item;
        MenuFlyoutItem Add(string text, string glyph, Action act, string? keys = null)
        {
            var entry = new MenuFlyoutItem { Text = text, Icon = new FontIcon { Glyph = glyph } };
            if (keys is not null)
                entry.KeyboardAcceleratorTextOverride = keys;
            entry.Click += (_, _) => act();
            menu.Items.Add(entry);
            return entry;
        }
        var episode = item.Episode ?? 1;
        if (shape == HomeShape.Card)
        {
            var resume = item.Resume is not null;
            if (item.Action is HomeAction.Resume or HomeAction.Play)
            {
                Add(resume ? T.ContinueMenuResume : T.ActionPlay, "", () => ctx.Play(card.Id, episode.ToString()));
                if (resume)
                    Add(T.EpisodePlayFromBeginning, "", () => ctx.Play(card.Id, episode.ToString(), fromStart: true));
            }
            Add(T.HomeMarkWatched(episode), "", () => MarkWatched(card.Id, episode), "Ctrl+Shift+M");
            Add(T.HomeRemoveContinue, "", () => RemoveFromContinue(card.Id), "Delete");
        }
        else
        {
            if (item.Action == HomeAction.Play && item.Playable != Playable.NotFound)
                Add(T.HomePlayEpisode(episode), "", () => ctx.Play(card.Id, episode.ToString()));
            // A title already on a list, in any status, is not offered again.
            if (item.Action != HomeAction.Nothing && item.Reason != UpNextReason.Planning && (ctx.Known.Entry(card.Id)?.ListStatus ?? item.Entry.ListStatus) is null)
                Add(T.UpNextAddPlanning, "", () =>
                {
                    AddToPlanning(card.Id);
                    ctx.Window.Say(T.LibraryAddedPlanning);
                });
            if (shape == HomeShape.Poster)
                Add(T.UpNextNotInterested, "", () => NotInterested(card.Item.Entry, card.Item.Starts is not null), "Delete");
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        Add(T.HomeDetails, "", () => ctx.OpenTitle(card.Id));
    }

    internal void CardKey(HomeCard card, HomeShape shape, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Delete)
            return;
        e.Handled = true;
        if (shape == HomeShape.Card)
            RemoveFromContinue(card.Id);
        else
            NotInterested(card.Item.Entry, card.Item.Starts is not null);
    }

    // Ctrl+Shift+M and Delete, from the window, on the focused card.

    (HomeCard Card, HomeShape Shape)? Focused()
    {
        if (FocusManager.GetFocusedElement(XamlRoot) is FrameworkElement { Tag: HomeCard card })
            return (card, card.Item.Progress is not null || card.Item.Resume is not null || Shelves.Children.OfType<HomeShelf>().Any(s => s.Kind == SectionKind.Continue && s.Cards.Contains(card)) ? HomeShape.Card : HomeShape.Poster);
        return null;
    }

    bool IPageKeys.MarkWatched()
    {
        if (Focused() is { Shape: HomeShape.Card } focused && focused.Card.Item.Episode is { } episode)
        {
            MarkWatched(focused.Card.Id, episode);
            return true;
        }
        return false;
    }

    bool IPageKeys.Delete() => false;
}
