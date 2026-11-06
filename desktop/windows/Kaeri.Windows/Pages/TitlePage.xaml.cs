using System.ComponentModel;
using Kaeri.App;
using Kaeri.Windows.Discussion;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>An episode row: "Episode 7 · The Lantern Road", its runtime or what is left, and its still.</summary>
public sealed partial class EpisodeItem : INotifyPropertyChanged
{
    ImageSource? still;
    bool narrow, room, held;
    double rowHeight = Synopsis.RowWide, textOpacity = 1, blurOpacity;
    string description = "";
    Microsoft.UI.Dispatching.DispatcherQueueTimer? growing;

    internal EpisodeItem(EpisodeRow row, uint title = 0)
    {
        Row = row;
        Title = title;
        Discussion = Ctx.Current is { } ctx && uint.TryParse(row.Number, out var n) ? ctx.Known.Discussions.GetValueOrDefault((title, n)) : 0;
    }

    public uint Title { get; }

    uint discussion;

    /// <summary>The comments in the episode's discussion, once the engine found one.</summary>
    public uint Discussion
    {
        get => discussion;
        set
        {
            if (discussion == value)
                return;
            discussion = value;
            Changed(nameof(DiscussionLine), nameof(DiscussionShown));
        }
    }

    public string DiscussionLine => T.DiscussionRow(Discussion);
    public Visibility DiscussionShown => Discussion > 0 ? Visibility.Visible : Visibility.Collapsed;

    public EpisodeRow Row { get; private set; }

    bool target;

    /// <summary>The row Go to episode landed on: it keeps the selection fill until the next jump.</summary>
    public bool Target
    {
        get => target;
        set
        {
            if (target == value)
                return;
            target = value;
            Changed(nameof(TargetFill));
        }
    }

    static readonly SolidColorBrush SelectionFill = new(global::Windows.UI.Color.FromArgb(0xFF, 0x34, 0x3A, 0x57));
    static readonly SolidColorBrush NoFill = new(Microsoft.UI.Colors.Transparent);
    public Brush TargetFill => target ? SelectionFill : NoFill;

    public string Heading => long.TryParse(Row.Number, out var n)
        ? Row.Title is { Length: > 0 } t ? T.EpisodeWithTitle(n, t) : T.EpisodeNumber(n)
        : Row.Title is { Length: > 0 } named ? T.EpisodeTextWithTitle(Row.Number, named) : T.EpisodeNumberText(Row.Number);
    /// <summary>What Narrator reads for the row: its heading, then its line when it has one.</summary>
    public string Name => Line.Length > 0 ? T.EpisodeRowSpoken(Heading, Line) : Heading;
    public double Watched => Row.Watched;
    public bool Completed => Row.Completed;
    /// <summary>An episode no source lists is dimmed whole; a watched one is only a little.</summary>
    public double Dim => Row.Unsourced ? 0.55 : Row.Completed ? 0.6 : 1;
    /// <summary>Play on TV in the episode's menu, once the engine gives a sender.</summary>
    public Visibility OnTv => Ctx.Current.Tv.Available ? Visibility.Visible : Visibility.Collapsed;
    public Visibility PartWay => Row.Watched is > 0 and < 1 && !Row.Completed ? Visibility.Visible : Visibility.Collapsed;
    public bool Playable => EpisodeSources.Playable(Row);
    public string WatchedAction => Row.Completed ? T.EpisodeMarkUnwatched : T.EpisodeMarkWatched;

    /// <summary>The facts line: length or what is left, when it aired, a filler episode saying so.</summary>
    public string Line => Synopsis.Facts(Row, DateTimeOffset.UtcNow.ToUnixTimeSeconds(), at => Synopsis.Date(at));

    /// <summary>A narrow row whose title stands alone: the number moves to the facts.</summary>
    bool TitleAlone => narrow && Row.Title is { Length: > 0 } && Synopsis.Number(Row.Number) is not null;
    /// <summary>The row's name as shown: the title alone on a narrow row, else the heading.</summary>
    public string RowHeading => TitleAlone ? Row.Title! : Heading;
    public TextWrapping HeadingWrap => narrow ? TextWrapping.WrapWholeWords : TextWrapping.NoWrap;
    public int HeadingLines => narrow ? 2 : 1;
    /// <summary>The facts as shown: on a narrow row with its title alone, "E8" first and the date on its own line.</summary>
    public string RowFacts => TitleAlone
        ? Synopsis.Facts(Row, DateTimeOffset.UtcNow.ToUnixTimeSeconds(), at => Synopsis.Date(at), Synopsis.Number(Row.Number))
        : Line;

    // What the row's layout is, from the window's width and what the engine says of text and
    // protection. The page sets it with Lay; every figure here is a size, never read from the text.
    public double RowHeight => rowHeight;
    public double StillWidth => narrow ? Synopsis.StillNarrow : Synopsis.StillWide;
    public double StillHeight => StillWidth * 9 / 16;
    public int StillRowSpan => narrow ? 1 : 2;
    public int SynopsisColumn => narrow ? 0 : 1;
    public int SynopsisColumnSpan => narrow ? 3 : 1;
    /// <summary>Two lines beside the still, where the row has less room; three under it.</summary>
    public int SynopsisLines => narrow ? 3 : 2;
    public Thickness HeadMargin => narrow ? new Thickness(0) : new Thickness(0, 10, 0, 0);
    public VerticalAlignment HeadAlign => narrow ? VerticalAlignment.Center : VerticalAlignment.Top;
    public Thickness RowPad => narrow && room ? new Thickness(0, 8, 0, 8) : new Thickness(0);
    /// <summary>Beside the still the room is always there, so a reveal moves nothing; under it, only where the row is tall.</summary>
    public Visibility TextVisible => !narrow || room ? Visibility.Visible : Visibility.Collapsed;
    public string Description => description;
    public double TextOpacity => textOpacity;
    /// <summary>The blurred copy of the still over the sharp one, fading away on a reveal.</summary>
    public double BlurOpacity => blurOpacity;
    public bool Protectable { get; private set; }
    public bool Held => held;
    public Visibility HeldShown => held ? Visibility.Visible : Visibility.Collapsed;
    /// <summary>The whole synopsis, for the row's tooltip.</summary>
    public string? Tip => room && !held ? Synopsis.TextOf(Row) : null;
    /// <summary>What Narrator reads after the name: the synopsis, or that spoiler protection hides it. Not read again when text arrives late.</summary>
    public string HelpText => held ? T.EpisodeSynopsisHidden : room ? Synopsis.TextOf(Row) ?? "" : "";

    /// <summary>
    /// Takes the layout the width, the text outlook and spoiler protection give. A narrow row that
    /// grows takes its height first (the standard token) and its text fades in once there is room.
    /// </summary>
    internal void Lay(bool narrow, EpisodeText outlook, bool protectable, bool held, bool animate)
    {
        growing?.Stop();
        var from = rowHeight;
        this.narrow = narrow;
        Protectable = protectable;
        this.held = held;
        room = Synopsis.HasRoom(outlook, held);
        blurOpacity = held && Row.Thumbnail is not null ? 1 : 0;
        var to = Synopsis.RowHeight(narrow, room);
        Changed(nameof(StillWidth), nameof(StillHeight), nameof(StillRowSpan), nameof(SynopsisColumn), nameof(SynopsisColumnSpan), nameof(SynopsisLines),
            nameof(HeadMargin), nameof(HeadAlign), nameof(RowPad), nameof(TextVisible), nameof(BlurOpacity), nameof(Held), nameof(HeldShown),
            nameof(Tip), nameof(HelpText), nameof(RowHeading), nameof(HeadingWrap), nameof(HeadingLines), nameof(RowFacts));
        var text = Synopsis.TextOf(Row);
        if (!animate || !narrow || !Motion.Enabled || Math.Abs(from - to) < 1)
        {
            rowHeight = to;
            Changed(nameof(RowHeight));
            SetText(text, animate);
            return;
        }
        // The row takes its height first, so no line is ever clipped by a growing frame.
        if (!room)
            SetText(null, false);
        Motion.Ease(this, from, to, height =>
        {
            rowHeight = height;
            Changed(nameof(RowHeight));
        });
        if (room)
        {
            growing = Timers.Weak(this, Motion.Standard, false, static item =>
            {
                item.growing?.Stop();
                item.SetText(Synopsis.TextOf(item.Row), true);
            });
            growing.Start();
        }
    }

    /// <summary>
    /// The synopsis the row shows: text that arrives fades in over 0.2 s into the room the row kept.
    /// A row laid out without room (protected, or no text coming) takes none until its next layout.
    /// </summary>
    internal void SetText(string? text, bool animate)
    {
        text = room && !held ? text : null;
        if ((text ?? "") == description)
            return;
        description = text ?? "";
        Changed(nameof(Description), nameof(Tip), nameof(HelpText));
        textOpacity = 1;
        if (text is not null && animate)
        {
            textOpacity = Motion.Enabled ? 0 : 1;
            Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread()?.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
            {
                textOpacity = 1;
                Changed(nameof(TextOpacity));
            });
        }
        Changed(nameof(TextOpacity));
    }

    /// <summary>
    /// Takes over what the row of the same episode showed before the table was drawn again, so text
    /// that arrives late fades into the room the row kept rather than the row starting over.
    /// </summary>
    /// <summary>The same episode as the engine now has it: its watched state and what follows from it change in place.</summary>
    internal void Take(EpisodeRow row)
    {
        if (row == Row)
            return;
        Row = row;
        Changed(nameof(Watched), nameof(Completed), nameof(Dim), nameof(PartWay), nameof(Playable), nameof(WatchedAction), nameof(Line), nameof(RowFacts), nameof(Name));
    }

    internal void Inherit(EpisodeItem was)
    {
        narrow = was.narrow;
        rowHeight = was.rowHeight;
        description = was.description;
        blurOpacity = was.blurOpacity;
    }

    EpisodeDownload download = EpisodeDownload.Of([], 0, "");
    bool reveal;

    /// <summary>Where the episode stands in the download queue.</summary>
    public EpisodeDownload Download
    {
        get => download;
        set
        {
            if (download == value)
                return;
            download = value;
            Changed(nameof(DownloadOpacity), nameof(DownloadSay), nameof(DownloadGlyph), nameof(DownloadTint), nameof(GlyphShown),
                nameof(RingShown), nameof(RingLoaded), nameof(RingWaiting), nameof(RingActive), nameof(RingValue), nameof(CanDownload));
        }
    }

    /// <summary>The pointer over the row, or focus in it: Download shows.</summary>
    public bool Reveal
    {
        get => reveal;
        set
        {
            if (reveal == value)
                return;
            reveal = value;
            Changed(nameof(DownloadOpacity));
        }
    }

    public bool CanDownload => Playable && Download.Asks;
    public string DownloadSay => Download.Say;
    /// <summary>Nothing yet, and no pointer or focus here: the cell is empty, though it keeps its place.</summary>
    public double DownloadOpacity => !Playable || (Download.Kind == EpisodeDownloadKind.None && !Reveal) ? 0 : 1;
    public string DownloadGlyph => Download.Kind == EpisodeDownloadKind.Done ? "\uE73E" : "\uE896";
    public Brush DownloadTint => Download.Kind switch
    {
        EpisodeDownloadKind.Done => (Brush)Application.Current.Resources["WaterBrush"],
        EpisodeDownloadKind.Failed => new SolidColorBrush(Ui.Vermilion),
        _ => (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
    };
    public Visibility GlyphShown => Download.Kind is EpisodeDownloadKind.None or EpisodeDownloadKind.Done or EpisodeDownloadKind.Failed ? Visibility.Visible : Visibility.Collapsed;
    public Visibility RingShown => GlyphShown == Visibility.Visible ? Visibility.Collapsed : Visibility.Visible;
    public bool RingLoaded => RingShown == Visibility.Visible;
    public bool RingWaiting => Download.Kind == EpisodeDownloadKind.Queued;
    /// <summary>A paused download's ring stands still at how far it got.</summary>
    public bool RingActive => Download.Kind != EpisodeDownloadKind.Paused;
    public double RingValue => Download.Fraction * 100;

    void Changed(params string[] names)
    {
        foreach (var name in names)
            PropertyChanged?.Invoke(this, new(name));
    }

    public ImageSource? Still
    {
        get => still;
        set { still = value; PropertyChanged?.Invoke(this, new(nameof(Still))); }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>The quiet line between episode rows: the folded watched episodes, or where spoiler protection starts.</summary>
public sealed partial class NoteItem : INotifyPropertyChanged
{
    string line = "", action = "";

    internal NoteItem(bool fold, long episode, string line, string action)
    {
        Fold = fold;
        Episode = episode;
        this.line = line;
        this.action = action;
    }

    public bool Fold { get; }

    /// <summary>For a protection line, the last episode before it.</summary>
    public long Episode { get; }

    public string Line => line;
    public string Action => action;
    public Visibility LineShown => line.Length > 0 ? Visibility.Visible : Visibility.Collapsed;

    internal void Say(string line, string action)
    {
        this.line = line;
        this.action = action;
        PropertyChanged?.Invoke(this, new(nameof(Line)));
        PropertyChanged?.Invoke(this, new(nameof(Action)));
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>Picks an episode row's template or a note's.</summary>
public sealed partial class EpisodeTemplates : DataTemplateSelector
{
    public DataTemplate? Episode { get; set; }
    public DataTemplate? Note { get; set; }

    protected override DataTemplate? SelectTemplateCore(object item) => item is NoteItem ? Note : Episode;

    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) => SelectTemplateCore(item);
}

/// <summary>
/// A title's page: the banner with the ground over its lower part, the cover beside the show
/// title, the metadata line, the one amber action labelled with what it plays, the list status
/// and More, then the episodes. Details and related titles sit in a side column when the content
/// is wide enough, and under the episodes when it is not.
/// </summary>
public sealed partial class TitlePage : Page, IPageKeys
{
    const double SideWidth = 900;

    /// <summary>The connected animation's key: a cover grid prepares it, the title page's cover takes it.</summary>
    internal const string CoverAnimation = "cover";

    readonly Ctx ctx = Ctx.Current;
    uint id;
    /// <summary>The title the frame opened this page with; <see cref="id"/> differs once a season is picked here.</summary>
    uint openedWith;
    QueueItem? action;

    public TitlePage()
    {
        InitializeComponent();
        foreach (var (segment, name) in new[]
        {
            (LibrarySegment.Watching, T.LibraryStatusWatching), (LibrarySegment.Planning, T.LibraryStatusPlanning),
            (LibrarySegment.Completed, T.LibraryStatusCompleted), (LibrarySegment.Paused, T.LibraryStatusPaused),
            (LibrarySegment.Dropped, T.LibraryStatusDropped),
        })
        {
            var item = new RadioMenuFlyoutItem { Text = name, GroupName = "status", Tag = segment };
            item.Click += (_, _) =>
            {
                // The engine only queues the change for the tracker: the status shows at once from here.
                ctx.Send(new Command.SetStatus(id, segment));
                ctx.Known.SetStatus(id, segment);
                if (ctx.Known.Entry(id) is { } entry)
                    ShowStatus(entry);
            };
            StatusMenu.Items.Add(item);
        }
        SourceItem.Text = T.SourcePickerTitle;
        EpisodeList.ItemTemplateSelector = (DataTemplateSelector)EpisodeList.Resources["Templates"];
        State.Placeholders = Placeholders;
        State.ActivityText = T.TitleLoading;
        EpisodesState.Placeholders = EpisodePlaceholders;
        EpisodesState.ActivityText = T.TitleLoading;
        var settings = new Microsoft.UI.Xaml.Input.KeyboardAccelerator { Key = global::Windows.System.VirtualKey.I, Modifiers = global::Windows.System.VirtualKeyModifiers.Control };
        settings.Invoked += (_, a) =>
        {
            a.Handled = true;
            OpenShowSettings();
        };
        KeyboardAccelerators.Add(settings);
        // The page's own key is in the shortcuts list; a tooltip naming it over the whole page is not wanted.
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
        KeysForGo();
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Hear(this, Heard, () =>
        {
            discussion?.Close();
        });
    }

    /// <summary>
    /// The banner's height and where the cover and title start under it. Before the page has a
    /// size of its own, as it is navigated to, the frame's width stands in, so the first frame
    /// drawn already has the band it keeps: otherwise its foot drew at the XAML's 300 and then
    /// jumped, with the title under it.
    /// </summary>
    void Layout()
    {
        var width = Scroll.ActualWidth > 0 ? Scroll.ActualWidth : Frame?.ActualWidth ?? 0;
        if (width <= 0)
            return;
        var band = Math.Min(360, Math.Max(220, width * 9 / 32));
        if (Math.Abs(Banner.Height - band) < 0.5 && Banner.Clip is not null)
            return;
        Banner.Height = band;
        // The art fills the band by overflowing it (UniformToFill), and the scrim ends at the
        // band's edge, so the band is clipped to its height: any overflow would show as a light
        // line under the scrim. Only the height matters; the width is generous.
        Banner.Clip = new RectangleGeometry { Rect = new global::Windows.Foundation.Rect(0, 0, 100_000, band) };
        Columns.Padding = new Thickness(36, band - 120, 36, 24);
    }

    /// <summary>The page's own layout as raised plates: banner, cover and title, then episode rows.</summary>
    UIElement Placeholders()
    {
        // The band as tall as the real one, and the cover and title where they will stand on it.
        var band = double.IsNaN(Banner.Height) ? 220 : Banner.Height;
        var page = new StackPanel { Spacing = 20 };
        page.Children.Add(new Border { Height = band, Background = Ui.Res<Brush>("RaisedBrush") });
        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 20, Margin = new Thickness(36, -120 - 20, 36, 0) };
        head.Children.Add(States.Plate(120, 180, 8));
        var bars = new StackPanel { Spacing = 10, VerticalAlignment = VerticalAlignment.Bottom };
        bars.Children.Add(States.Plate(320, 28, 4));
        bars.Children.Add(States.Plate(200, 14, 3));
        bars.Children.Add(States.Plate(150, 40, 6));
        head.Children.Add(bars);
        page.Children.Add(head);
        var rows = States.Rows(4, 192, 108);
        if (rows is FrameworkElement f)
            f.Margin = new Thickness(36, 8, 36, 0);
        page.Children.Add(rows);
        return page;
    }

    /// <summary>
    /// The episode rows while they load, in the rows' own layout and size: the still and two
    /// lines, each row as high as a real one with its margin. As many as the title has, where
    /// that is known, up to a screenful, so the rows that replace them take the room they kept.
    /// </summary>
    UIElement EpisodePlaceholders()
    {
        var count = (int)Math.Clamp(ctx.Known.Entry(id)?.Episodes ?? 4, 1, 12);
        var narrow = Scroll.ActualWidth is > 0 and < global::Kaeri.App.Synopsis.NarrowBelow;
        var still = narrow ? global::Kaeri.App.Synopsis.StillNarrow : global::Kaeri.App.Synopsis.StillWide;
        var rows = States.Rows(count, still, Math.Round(still * 9 / 16), rowHeight: global::Kaeri.App.Synopsis.RowHeight(narrow, true) + 4, spacing: 0);
        if (rows is FrameworkElement f)
            f.Margin = new Thickness(0, 8, 0, 0);
        return rows;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        id = openedWith = (uint)e.Parameter;
        var known = ctx.Known.Entry(id) is not null;
        if (!known)
            State.Wait();
        // Episodes not yet known: nothing for 600 ms, then plates in the rows' place.
        if (!ctx.Known.Episodes.ContainsKey(id))
            EpisodesState.Wait();
        Load();
#if KAERI_TESTING
        // --show=title-another: as if Surprise me opened this title, so Another is there.
        if (ctx.Options.Show == "title-another" && ctx.Options.Title == id)
            ctx.Pick = new Pick(id, new SurprisePool.Planning(), true);
#endif
        ShowAnother();
        // The cover a grid sent flies in to this one, on the standard spring as the Mac's art flies;
        // anything else crossfades.
        var service = ConnectedAnimationService.GetForCurrentView();
        var flight = service.GetAnimation(CoverAnimation);
        if (flight is not null)
        {
            if (known && Motion.Enabled)
            {
                service.DefaultDuration = Motion.Standard;
                service.DefaultEasingFunction = Motion.SpringEasing(Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(CoverFrame).Compositor);
                flight.Configuration = new BasicConnectedAnimationConfiguration();
                flight.TryStart(CoverFrame);
            }
            else
                flight.Cancel();
        }
        if (ctx.Options.Show is "settings" or "settings-loading" or "settings-failed" && ctx.Options.Title == id)
            DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, OpenShowSettings);
#if KAERI_TESTING
        if (ctx.Options.Show is { } more && more.StartsWith("more-open") && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                if (Current() is { } now && XamlRoot is { } root)
                    new TitleMoreDialog(ctx, root, now).Show();
            }));
        if (ctx.Options.Show is "source-picker" or "source-picker-pinned" or "source-picker-releases" or "source-picker-loading" && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(OpenSourcePicker));
        if (ctx.Options.Show is { } matchShow && matchShow.StartsWith("match", StringComparison.Ordinal) && ctx.Options.Title == id)
            _ = StageMatch(matchShow);
        // --show=source-unsourced-end: the table scrolled to its end, where the last unsourced rows are.
        if (ctx.Options.Show == "source-unsourced-end")
            _ = Task.Delay(2500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Scroll.ChangeView(null, Scroll.ScrollableHeight, null, true)));
        // --show=discussion...: the quiet rows under watched episodes, and the pane open on episode 2.
        if (Discussion.DiscussionShowcase.Is(ctx.Options.Show) && ctx.Options.Title == id)
            _ = Task.Delay(1800).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                Event[] found = [new Event.DiscussionFound(id, 1, 38), new Event.DiscussionFound(id, 2, 214)];
                foreach (var f in found)
                    ctx.Known.Take(f);
                ctx.Heard(found);
                if (ctx.Options.Show == "discussion-row")
                    return;
                OpenDiscussion(2, null);
                var show = ctx.Options.Show!;
                _ = Task.Delay(700).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
                {
                    if (discussion is { } pane)
                        Discussion.DiscussionShowcase.Then(show, pane);
                }));
            }));
        // --show=trailer-window, trailer-window-stuck: the Play trailer window open, playing or never starting.
        if (ctx.Options.Show is "trailer-window" or "trailer-window-stuck" && ctx.Options.Title == id)
            _ = Task.Delay(3500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                if (ctx.Known.Entry(id) is not { } entry)
                    return;
                if (ctx.Options.Show == "trailer-window-stuck")
                    entry = entry with { Trailer = new Trailer(TrailerHost.YouTube, Trailers.FixtureStuckId) };
                TrailerWindow.Play(ctx, entry);
            }));
        // --show=synopsis...: the rows from the one playing next in view; synopsis-revealed shows
        // episode 9 through protection, synopsis-ranges opens the range menu.
        if (ctx.Options.Show is { } synopsisShot && synopsisShot.StartsWith("synopsis", StringComparison.Ordinal) && ctx.Options.Title == id)
            _ = Task.Delay(3500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => StageSynopsis(synopsisShot)));
        // --show=source-menu: the More menu open, for a screenshot.
        if (ctx.Options.Show == "source-menu" && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => MoreButton.Flyout.ShowAt(MoreButton)));
#endif
    }

    /// <summary>Asks for what the title's page shows and draws what is known of it now.</summary>
    void Load()
    {
        ctx.Send(new Command.LoadDetail(id));
        ctx.Send(new Command.LoadEpisodes(id));
        // The queue, for each row's download cell.
        ctx.Send(new Command.LoadDownloads());
        ctx.Send(new Command.LoadDub(id));
        // What the show sets for itself, its spoiler protection among it.
        ctx.Send(new Command.LoadShowSettings(id));
        DubLine.Opacity = 0;
        Layout();
        Show();
        ShowEpisodes();
    }

    /// <summary>
    /// Another season of the show, picked here: the page stays and takes it in place, as on the
    /// Mac. What stood crossfades to the new season's words and rows, the scroll stays where it
    /// is, and Back still goes to where the title was opened from, not through every season tried.
    /// </summary>
    void SwitchTo(uint to)
    {
        if (to == id)
            return;
        Crossfade.Swap(Scroll, () =>
        {
            discussion?.Close();
            id = to;
            // What was chosen for the last season does not carry over to this one.
            chosenRange = null;
            opened = false;
            revealAll = false;
            revealed.Clear();
            shownShape = null;
            protection = null;
            targetNumber = null;
            askedPlaysHere = null;
            detailsKey = null;
            availKey = null;
            shownSynopsis = null;
            // The new season's words wait 600 ms over what stood, then plates; its rows, when
            // not yet known, are plates at once, so no row of the last season is clicked for this one.
            if (ctx.Known.Entry(id) is null)
                State.WaitKeeping(true);
            if (!ctx.Known.Episodes.ContainsKey(id))
            {
                allItems = null;
                EpisodeList.ItemsSource = null;
                EpisodesState.WaitKeeping(false);
            }
            Load();
            ShowAnother();
        });
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        // A season picked here changed the title in place: once the frame has put this page on
        // its back stack, its entry is made to say that season, so Back comes to it.
        if (e.NavigationMode != NavigationMode.New || id == openedWith || Frame is not { } frame)
            return;
        var shown = id;
        var opened = openedWith;
        void Fix(object sender, NavigationEventArgs args)
        {
            frame.Navigated -= Fix;
            var stack = frame.BackStack;
            if (stack.Count > 0 && stack[^1] is { Parameter: uint was } entry && entry.SourcePageType == typeof(TitlePage) && was == opened)
            {
                stack.RemoveAt(stack.Count - 1);
                stack.Add(new PageStackEntry(typeof(TitlePage), shown, entry.NavigationTransitionInfo));
            }
        }
        frame.Navigated += Fix;
    }

#if KAERI_TESTING
    void Later(double seconds, Action act)
    {
        var timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(seconds) };
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            act();
        };
        timer.Start();
    }

    /// <summary>The synopsis shots' state: see --show=synopsis... where the page opens.</summary>
    void StageSynopsis(string shot)
    {
        // The Go to episode shots: the dialog empty, with a refusal, or a number typed, or landed on.
        if (shot.StartsWith("synopsis-go", StringComparison.Ordinal))
        {
            switch (shot)
            {
                case "synopsis-go-dialog": OpenGoTo(); break;
                case "synopsis-go-none": OpenGoTo("300"); break;
                case "synopsis-go-not-out": OpenGoTo("245"); break;
                case "synopsis-go-typed": typed = "142"; SayTyped(); break;
                case "synopsis-go-landed":
                    if (Browsing.Go(Rows(), "142") is GoTo.Row landing)
                        Land(landing);
                    break;
                // Motion D: the range menu, then Go to episode, 128 typed and Go, across a range.
                case "synopsis-go-motion":
                    RangePicker.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0.2 });
                    Later(1.0, () => RangePicker.Flyout?.ShowAt(RangePicker));
                    Later(2.4, () =>
                    {
                        RangePicker.Flyout?.Hide();
                        OpenGoTo("go:128");
                    });
                    break;
                // Motion E: 2, 3, 3 typed with episode 230 at the top, a short jump within the screen.
                case "synopsis-go-digits":
                    Later(0.6, () =>
                    {
                        if (Episodes().FirstOrDefault(i => i.Row.Number == "230") is { } from && EpisodeList.ContainerFromItem(from) is UIElement row)
                            row.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 });
                    });
                    // The list has focus on that row, as after a click on it.
                    Later(1.0, () =>
                    {
                        if (Episodes().FirstOrDefault(i => i.Row.Number == "230") is { } from && EpisodeList.ContainerFromItem(from) is Control row)
                            row.Focus(FocusState.Programmatic);
                    });
                    Later(1.6, () => TypeDigit(2));
                    Later(1.9, () => TypeDigit(3));
                    Later(2.2, () => TypeDigit(3));
                    break;
            }
            return;
        }
        if (shot == "synopsis-ranges")
        {
            RangePicker.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0.2 });
            _ = Task.Delay(600).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => RangePicker.Flyout?.ShowAt(RangePicker)));
            return;
        }
        if (shot == "synopsis-revealed")
        {
            revealed.Add("9");
            Relay(true);
        }
        var next = Episodes().FirstOrDefault(i => i.Row.Number == "7");
        if (next is not null && EpisodeList.ContainerFromItem(next) is UIElement row)
            row.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0.1 });
    }

    /// <summary>
    /// --show=match, match-menu, match-failed, match-empty, match-results, match-picking,
    /// match-picked, match-cleared and match-source-picker, on --scene=unsure: the picker in each
    /// of its states, once the engine has said what the title is matched to.
    /// </summary>
    async Task StageMatch(string show)
    {
        for (var tries = 0; !ctx.Known.Matches.IsKnown(id) && tries < 80; tries++)
            await Task.Delay(250);
        // The menu's states show the picker as a match is in place: the Matched now row.
        void InPlace(bool byYou) => ctx.Inject(new Event.TitleMatch(id, new CurrentMatch("hotaru-open-archive", "Hotaru Open Archive", "Hoshi Tabi Diaries", byYou)));
        switch (show)
        {
            case "match":
            case "match-picking":
                OpenMatchPicker(fromMenu: false);
                if (show == "match-picking")
                {
                    await Task.Delay(800);
                    if (ctx.Known.Matches.Asked(id)?.Candidates.FirstOrDefault() is { } best)
                    {
                        ctx.Known.Matches.Pick(id, best, "Hotaru Open Archive");
                        ctx.Send(new Command.FixMatch(id, "hotaru-open-archive", best.Key, best.Title));
                    }
                }
                break;
            case "match-source-picker":
                OpenSourcePicker();
                break;
            case "match-menu":
            case "match-failed":
                InPlace(false);
                OpenMatchPicker(fromMenu: true);
                break;
            case "match-empty":
                InPlace(false);
                OpenMatchPicker(fromMenu: true, "hoshikuzu tabibto");
                break;
            case "match-results":
                InPlace(true);
                OpenMatchPicker(fromMenu: true, "Stardust Wayfarer");
                break;
            case "match-picked":
            case "match-cleared":
                // Picked for real: the show itself, which the source finds by its English title.
                OpenMatchPicker(fromMenu: true, "Stardust Wayfarer");
                await Task.Delay(1500);
                if (ctx.Known.Matches.Search(id) is MatchSearch.Found { Candidates: [var show1, ..], ProviderId: var provider, Source: var source })
                {
                    ctx.Known.Matches.Pick(id, show1, source);
                    ctx.Send(new Command.FixMatch(id, provider, show1.Key, show1.Title));
                }
                if (show == "match-cleared")
                {
                    await Task.Delay(2500);
                    OpenMatchPicker(fromMenu: true);
                    await Task.Delay(1200);
                    ctx.Known.Matches.Cleared(id);
                    ctx.Window.Say(T.MatchCleared);
                    ctx.Send(new Command.ClearMatch(id));
                    Dialogs.HideOpen();
                }
                break;
        }
    }
#endif

    void Heard(IReadOnlyList<Event> batch)
    {
        HeardAnother(batch);
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Detail d when d.V1.Id == id:
                    Show();
                    break;
                case Event.Progress p when p.Id == id:
                    // The event carries the rows' watched state, which Known has put in the
                    // table: the rows change in place, with no new listing and no rebuilt list.
                    if (ctx.Known.Entry(id) is { } progressed)
                        ShowAction(progressed);
                    if (p.EpisodesValue.Length > 0)
                        ShowEpisodes();
                    else
                        ctx.Send(new Command.LoadEpisodes(id));
                    break;
                case Event.DiscussionFound found when found.Media == id:
                    foreach (var item in Episodes().Where(i => i.Row.Number == found.Episode.ToString()))
                        item.Discussion = found.Comments;
                    break;
                case Event.TitleDub td when td.Id == id:
                    var line = Wording.DubLine(td.Dub);
                    if (line is not null)
                        DubLine.Text = line;
                    DubLine.Opacity = line is null ? 0 : 1;
                    break;
                case Event.MatchChoices m when m.Id == id:
                case Event.TitleMatch tm when tm.Id == id:
                    // The source could not choose, or a pick settled: the main action follows.
                    if (ctx.Known.Entry(id) is { } matched)
                        ShowAction(matched);
                    break;
                case Event.Availability av when av.V1.Id == id:
                    ShowAction(Current()!);
                    break;
                case Event.Episodes ep when ep.Id == id:
                    ShowEpisodes();
                    // The button plays the first episode after the next that a source lists.
                    if (ctx.Known.Entry(id) is { } known)
                        ShowAction(known);
                    break;
                case Event.Downloads d:
                    ShowDownloads(d.V1);
                    break;
                case Event.Settings:
                case Event.ShowSettings sheet when sheet.V1.Id == id:
                    ShowSpoilers();
                    break;
                case Event.TitlesChanged:
                    // Named another way now, with its relations and recommendations.
                    ctx.Send(new Command.LoadDetail(id));
                    break;
            }
        }
    }

    Entry? Current() =>
#if KAERI_TESTING
        ctx.Known.Entry(id) is { } e ? Sampled(e) : null;
#else
        ctx.Known.Entry(id);
#endif

#if KAERI_TESTING
    /// <summary>--show=more...: made-up facts and sheet content, as the fixtures carry none. +film, +hiatus, +cancelled.</summary>
    Entry Sampled(Entry e)
    {
        if (ctx.Options.Show is not { } show || !show.StartsWith("more"))
            return e;
        var filmed = show.Contains("+film");
        AiringFact run = show.Contains("+hiatus") ? new AiringFact.Hiatus(new FuzzyDate(2026, 3, null))
            : show.Contains("+cancelled") ? new AiringFact.Cancelled(6)
            : filmed ? new AiringFact.Finished(new FuzzyDate(2025, 5, 2), null)
            : new AiringFact.Finished(new FuzzyDate(2023, 4, 8), new FuzzyDate(2023, 6, 24));
        return e with
        {
            Format = filmed ? "MOVIE" : "ONA",
            Studio = "Ginrei Studio",
            Source = SourceMaterial.LightNovel,
            Airing = run,
            FilmLength = filmed ? new RunLength(1, 52) : null,
            Country = "CN",
            MadeOutsideJapan = show.Contains("+china"),
            Detail = new TitleDetail(
                Tags: [new TitleTag("Found family", 90, false, false, null), new TitleTag("Rural", 70, false, false, null), new TitleTag("Betrayal", 60, true, false, null), new TitleTag("Hidden past", 50, false, true, null)],
                Rankings: [new Ranking(4, RankingKind.Rated, "", false, Season.Spring, 2023), new Ranking(11, RankingKind.Popular, "", true, null, null)],
                Characters: [new CastMember(Name: "Mira Okonkwo", Role: CastRole.Main, VoiceActors: [new Person("Haru Tanabe", null)]), new CastMember(Name: "Tobias Lindqvist", Role: CastRole.Main, VoiceActors: [new Person("Ren Aoyama", null)]), new CastMember(Name: "Elsa Marchetti", Role: CastRole.Supporting, VoiceActors: [])],
                Staff: [new StaffCredit("Noor Haddad", "Director"), new StaffCredit("Pavel Novak", "Series composition")],
                StreamingLinks: [new StreamingLink("Lanternflix", "https://lanternflix.example/show", null), new StreamingLink("Kiri Stream", "https://kiri.example/show", "Japanese")],
                Following: show.Contains("+signed-out") ? [] : [new FollowedEntry("Ines", null, LibrarySegment.Completed, 90, 10), new FollowedEntry("Jonas", null, LibrarySegment.Watching, null, 4)]),
        };
    }
#endif

    /// <summary>
    /// The banner's art, faded in once the page has been laid out. Drawn at once, it showed bare in
    /// the page's first frame, before its scrim, and its foot read as a light line. The same art
    /// arriving again leaves it as it is.
    /// </summary>
    void ShowBanner(ImageSource art)
    {
        var again = BannerImage.Source == art && BannerImage.Opacity > 0;
        if (again)
            return;
        if (BannerImage.Source is not null && BannerImage.Opacity > 0 && IsLoaded)
        {
            Crossfade.Swap(Art, () => BannerImage.Source = art);
            return;
        }
        BannerImage.Source = art;
        if (IsLoaded)
        {
            Motion.FadeIn(BannerImage);
            return;
        }
        // From the cache the art is there before the page's first frame, and the scrim over it
        // draws about 0.1 s after the page is loaded: the fade starts once it is surely there.
        void Laid(object sender, RoutedEventArgs e)
        {
            Loaded -= Laid;
            _ = Task.Delay(150).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                if (BannerImage.Source == art)
                    Motion.FadeIn(BannerImage);
            }));
        }
        Loaded += Laid;
    }

    void Show()
    {
        if (Current() is not { } entry)
            return;
        State.Done();
        var cover = new CoverItem(entry);
        CoverFrame.Background = cover.Placeholder;
        // No cover colour behind the banner: until its art fades in, the band is the ground, as
        // under the scrim, so nothing lighter ends at its foot.
        ctx.Art.Bind(entry.CoverUrl, s => CoverImage.Source = s);
        ctx.Art.Bind(entry.BannerUrl ?? entry.CoverUrl, ShowBanner);
        ShowTitle.Text = entry.Title;
        Secondary.Text = entry.Secondary ?? "";
        Secondary.Visibility = entry.Secondary is { Length: > 0 } ? Visibility.Visible : Visibility.Collapsed;
        Meta.Text = Wording.MetaLine(entry);
        if (shownSynopsis != entry.Synopsis)
        {
            shownSynopsis = entry.Synopsis;
            Synopsis.MaxLines = 8;
            SynopsisMore.Visibility = Visibility.Collapsed;
            Synopsis.Text = entry.Synopsis;
        }
        DemoCredit.Visibility = entry.IsDemo ? Visibility.Visible : Visibility.Collapsed;
        DemoCredit.Content = entry.IsDemo ? DemoHome.Credit(KaeriFfiMethods.DemoInfo()) : null;

        ShowAction(entry);

        ShowStatus(entry);
        TrailerItem.IsEnabled = entry.TrailerUrl is not null;

        ShowSeasons(entry);
        ShowDetails(entry);
    }

    /// <summary>
    /// The one amber action and what depends on it: the engine's next episode, or once the table
    /// is loaded the first after it that some source lists. With none, a line says so in its place.
    /// </summary>
    /// <summary>More shows only where the synopsis is cut at its eight lines.</summary>
    string? shownSynopsis;

    void OnSynopsisTrimmed(TextBlock sender, IsTextTrimmedChangedEventArgs args) =>
        SynopsisMore.Visibility = sender.IsTextTrimmed ? Visibility.Visible : Visibility.Collapsed;

    void OnSynopsisMore(object sender, RoutedEventArgs args)
    {
        Synopsis.MaxLines = 0;
        SynopsisMore.Visibility = Visibility.Collapsed;
    }

    /// <summary>What the availability line and the More menu's Watch on items were drawn from.</summary>
    string? availKey;

    TitleAvailability? Avail => ctx.Known.Availability.GetValueOrDefault(id);
    bool? askedPlaysHere;

    /// <summary>
    /// Which licensed services have the title, once the sources have answered. The engine decides the
    /// case; this only asks again when whether it plays here changes.
    /// </summary>
    void AskAvailability(bool playsHere)
    {
        if (askedPlaysHere == playsHere)
            return;
        askedPlaysHere = playsHere;
        ctx.Send(new Command.Licensed(new LicensedCommand.LoadAvailability(id, playsHere)));
    }

    void OpenService(ServiceLink link, uint? episode = null) =>
        ctx.Send(new Command.Licensed(new LicensedCommand.Open(id, link.Service, link.Url, episode)));

    /// <summary>Case A and C lines, the Case B action and the More menu's Watch on items.</summary>
    void ShowAvailability()
    {
        var key = Drawn.Fingerprint(Avail);
        if (key == availKey)
            return;
        availKey = key;
        AvailLine.Children.Clear();
        AvailLine.Visibility = Visibility.Collapsed;
        foreach (var old in MoreMenu.Items.Where(i => i.Tag as string == "avail").ToList())
            MoreMenu.Items.Remove(old);
        if (Avail is not { } a)
            return;
        var at = 0;
        foreach (var link in a.Yours ?? [])
        {
            var item = new MenuFlyoutItem { Text = T.TitleWatchOn(link.Service), Tag = "avail", Icon = new FontIcon { Glyph = "\uE8A7" } };
            item.Click += (_, _) => OpenService(link);
            MoreMenu.Items.Insert(at++, item);
        }
        if (at > 0)
            MoreMenu.Items.Insert(at, new MenuFlyoutSeparator { Tag = "avail" });

        void Text(string text) => AvailLine.Children.Add(new TextBlock { Text = text, Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords });
        HyperlinkButton Link(string text, Action go)
        {
            var b = new HyperlinkButton { Content = text, Padding = new Thickness(4, 0, 4, 0) };
            b.Click += (_, _) => go();
            AvailLine.Children.Add(b);
            return b;
        }
        // The not-found line is a sentence and a link: stacked, so a narrow window wraps the sentence instead of clipping the link.
        AvailLine.Orientation = a.Case == AvailabilityCase.NotInSources ? Orientation.Vertical : Orientation.Horizontal;
        switch (a.Case)
        {
            case AvailabilityCase.PlaysAlsoOn or AvailabilityCase.OnService when (a.AlsoOn ?? []).Length > 0:
                Text(T.TitleAlsoOnServices("").TrimEnd());
                foreach (var link in a.AlsoOn!)
                    Link(link.Service, () => OpenService(link));
                break;
            case AvailabilityCase.NotInSources when (a.Others ?? []).Length > 0:
                Text(T.TitleListedOn(string.Join(", ", a.Others!)));
                if (a.ChooseServices)
                    Link(T.TitleChooseServices, () => ctx.Window.Push(typeof(LicensedPage), null));
                break;
        }
        AvailLine.Visibility = AvailLine.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void ShowAction(Entry entry)
    {
        action = PlayableNext.Item(entry, ctx.Known.Episodes.GetValueOrDefault(id), Casing.Sentence);
        FixMatchItem.IsEnabled = MatchBook.CanFix(ctx.Known.Providers);
        if (ctx.Known.Matches.Asked(id) is { } asked)
        {
            // The source could not choose between its titles: choosing is the way on, in Play's place.
            PrimaryAction.Content = T.MatchChoose;
            PrimaryAction.Visibility = Visibility.Visible;
            WaitingLine.Text = MatchWords.Uncertain(asked);
            WaitingLine.Visibility = Visibility.Visible;
            PartyItem.IsEnabled = false;
            TvAction.Visibility = Visibility.Collapsed;
            return;
        }
        if (ctx.Known.Episodes.ContainsKey(id))
            AskAvailability(action is { Playable: true });
        // The demo plays in parts: "Play part 1".
        var words = entry.IsDemo && action.Button is not null ? T.TitlePlayPart(action.Episode.ToString()) : action.Button;
        if (PrimaryAction.Content as string != words)
            PrimaryAction.Content = words;
        PrimaryAction.Visibility = action.Button is not null ? Visibility.Visible : Visibility.Collapsed;
        WaitingLine.Text = action.Waiting ?? "";
        WaitingLine.Visibility = action.Waiting is not null ? Visibility.Visible : Visibility.Collapsed;
        // A party plays the episode the primary action would.
        PartyItem.IsEnabled = action is { Playable: true };
        // Items that would do nothing are greyed, not left to fail in silence: nothing to
        // download yet, or no episode before the next one left unwatched.
        var table = ctx.Known.Episodes.GetValueOrDefault(id);
        DownloadItem.IsEnabled = table is { } listed && EpisodeSources.Downloadable(listed).Length > 0;
        MarkPreviousItem.IsEnabled = table is { } all && all.Any(r => uint.TryParse(r.Number, out var n) && n < action.Episode && !r.Completed);
        // Play on TV comes and goes with a fade, not a snap of the buttons beside it.
        var tv = action is { Playable: true } && ctx.Tv.Available;
        if (tv != (TvAction.Visibility == Visibility.Visible))
        {
            if (tv)
                Motion.Show(TvAction);
            else
                Motion.Hide(TvAction);
        }
        // Case B: it does not play here, so the amber action opens the service that has it.
        if (Avail is { Case: AvailabilityCase.OnService, Main: { } main } && action is not { Playable: true })
        {
            if (PrimaryAction.Content is not StackPanel { Tag: string said } || said != main.Service)
                PrimaryAction.Content = ExternalLabel(T.TitleWatchOn(main.Service), main.Service);
            PrimaryAction.Visibility = Visibility.Visible;
            WaitingLine.Visibility = Visibility.Collapsed;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PrimaryAction, T.TitleWatchOn(main.Service));
        }
        else
            PrimaryAction.ClearValue(Microsoft.UI.Xaml.Automation.AutomationProperties.NameProperty);
        ShowAvailability();
    }

    static StackPanel ExternalLabel(string text, string service)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Tag = service };
        row.Children.Add(new TextBlock { Text = text });
        row.Children.Add(new FontIcon { Glyph = "\uE8A7", FontSize = 12 });
        return row;
    }

    IReadOnlyList<Kaeri.App.TitleSeasons.Choice> seasons = [];

    /// <summary>The franchise's seasons, once the engine has walked them, with this one picked.</summary>
    void ShowSeasons(Entry entry)
    {
        var now = Kaeri.App.TitleSeasons.Of(entry);
        if (now.SequenceEqual(seasons))
            return;
        seasons = now;
        SeasonPicker.SelectionChanged -= OnSeasonPicked;
        SeasonPicker.Items.Clear();
        foreach (var choice in seasons)
            SeasonPicker.Items.Add(choice.Label);
        SeasonPicker.SelectedIndex = seasons.ToList().FindIndex(c => c.IsThis);
        SeasonPicker.SelectionChanged += OnSeasonPicked;
        if (seasons.Count > 0)
            Motion.Show(SeasonPicker);
        else
            Motion.Set(SeasonPicker, false);
    }

    void OnSeasonPicked(object sender, SelectionChangedEventArgs e)
    {
        if (SeasonPicker.SelectedIndex is not (var at and >= 0) || at >= seasons.Count || seasons[at].IsThis)
            return;
        SwitchTo(seasons[at].Id);
    }

    void ShowDetails(Entry entry)
    {
        var wide = Scroll.ActualWidth == 0 || Scroll.ActualWidth >= SideWidth;
        var related = Kaeri.App.TitleSeasons.Related(entry);
        var key = $"{wide}|{entry.Id}|{entry.Source}|{Drawn.Fingerprint(entry.Airing)}|{TitleFacts.Country(entry)}|{string.Join(",", entry.Genres ?? [])}|{BroadcastLine(entry)}|"
            + string.Join(";", related.Take(8).Select(r => $"{r.Id},{r.Title},{r.Relation},{r.Format}"));
        if (key == detailsKey)
            return;
        // On the same side, the column changes as a whole, crossfaded in its place.
        var same = detailsKey is not null && detailsKey.StartsWith($"{wide}|", StringComparison.Ordinal);
        detailsKey = key;
        if (same && Motion.Enabled && (wide ? Side : DetailsBelow).ActualHeight > 0)
        {
            Crossfade.Swap(wide ? Side : DetailsBelow, () => LayDetails(entry, wide, related));
            return;
        }
        LayDetails(entry, wide, related);
    }

    /// <summary>What the side column was last laid from, and where.</summary>
    string? detailsKey;

    void LayDetails(Entry entry, bool wide, RelatedTitle[] related)
    {
        SideColumn.Width = wide ? new GridLength(260) : new GridLength(0);
        Side.Visibility = wide ? Visibility.Visible : Visibility.Collapsed;
        var host = wide ? Side : DetailsBelow;
        Side.Children.Clear();
        DetailsBelow.Children.Clear();

        // Facts, a label and its value each. Studio and the season are in the metadata line.
        var facts = new Grid { ColumnSpacing = 12, RowSpacing = 6 };
        facts.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(72) });
        facts.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        void Fact(string label, string? value)
        {
            if (string.IsNullOrEmpty(value))
                return;
            var row = facts.RowDefinitions.Count;
            facts.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var name = new TextBlock { Text = label, FontSize = 12, Style = Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.WrapWholeWords };
            var text = new TextBlock { Text = value, TextWrapping = TextWrapping.WrapWholeWords };
            Grid.SetRow(name, row);
            Grid.SetRow(text, row);
            Grid.SetColumn(text, 1);
            facts.Children.Add(name);
            facts.Children.Add(text);
        }
        Fact(T.TitleBasedOn, entry.Source is { } source ? MaterialName(source) : null);
        if (TitleFacts.Airing(entry.Airing) is { } airing)
            Fact(airing.Label, airing.Value);
        Fact(T.FilterCountry, TitleFacts.Country(entry));
        Fact(T.TitleGenres, entry.Genres is { Length: > 0 } genres ? string.Join(" · ", genres.Select(Searches.GenreName)) : null);
        host.Children.Add(facts);
        var more = new Button { Content = T.TitleMoreAbout, MinHeight = 32, HorizontalAlignment = HorizontalAlignment.Stretch, Background = Res<Brush>("RaisedBrush") };
        more.Click += (_, _) =>
        {
            if (Current() is { } now && XamlRoot is { } root)
                new TitleMoreDialog(ctx, root, now).Show();
        };
        host.Children.Add(more);
        if (BroadcastLine(entry) is { Length: > 0 } broadcast)
            host.Children.Add(Ui.Haze(broadcast));

        if (related.Length > 0)
        {
            var list = new StackPanel { Spacing = 6 };
            list.Children.Add(new TextBlock { Text = T.TitleRelated, Style = Res<Style>("BodyStrongTextBlockStyle") });
            foreach (var r in related.Take(8))
            {
                var link = new HyperlinkButton { Padding = new Thickness(0), HorizontalAlignment = HorizontalAlignment.Left };
                var text = new StackPanel();
                text.Children.Add(new TextBlock { Text = r.Title, Style = Res<Style>("ShowTitleStyle") });
                text.Children.Add(new TextBlock { Text = RelationLine(r.Relation, r.Format), Style = Res<Style>("HazeCaptionStyle") });
                link.Content = text;
                var target = r.Id;
                link.Click += (_, _) => ctx.OpenTitle(target);
                HoverPreview.Attach(link, target);
                list.Children.Add(link);
            }
            host.Children.Add(list);
        }
    }

    // How the episode list is drawn: the window's width, the range and fold the viewer chose, and
    // what spoiler protection holds back.
    bool narrowList, opened, revealAll;
    int? chosenRange;
    bool shownProtection;
    EpisodeText outlook = EpisodeText.None;
    EpisodeRange[] ranges = [];
    NoteItem? protection;
    readonly HashSet<string> revealed = [];

    IEnumerable<EpisodeItem> Episodes() => allItems?.OfType<EpisodeItem>() ?? [];

    /// <summary>Whether spoiler protection is on for this show: what it sets for itself, else Settings.</summary>
    bool ProtectionOn() => Spoilers.On(ctx.Known, id);

    bool Held(EpisodeRow row) =>
        global::Kaeri.App.Synopsis.Held(row, ProtectionOn(), revealAll || revealed.Contains(row.Number));

    /// <summary>The rows built with the page: more than a tall window shows.</summary>
    const int FirstRows = 14;

    /// <summary>Every row of the shown list, built or still to come.</summary>
    List<object>? allItems;

    /// <summary>Adds the rest of <paramref name="items"/> to <paramref name="showing"/>, a few rows each time the UI thread is idle.</summary>
    void MoreRows(System.Collections.ObjectModel.ObservableCollection<object> showing, List<object> items)
    {
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
        {
            // The list was replaced since: its rows are not these.
            if (!ReferenceEquals(EpisodeList.ItemsSource, showing))
                return;
            for (var n = 0; n < 8 && showing.Count < items.Count; n++)
                showing.Add(items[showing.Count]);
            if (showing.Count < items.Count)
                MoreRows(showing, items);
        });
    }

    /// <summary>Whether two rows of one episode differ only in how far it was watched.</summary>
    static bool OnlyProgress(EpisodeRow was, EpisodeRow now) =>
        now with { Watched = was.Watched, Completed = was.Completed, SpoilerAhead = was.SpoilerAhead, PastProgress = was.PastProgress } == was;

    /// <summary>The shown rows' numbers and places, and what decides how they are laid: the list is rebuilt only when it changes.</summary>
    string? shownShape;

    void ShowEpisodes()
    {
        if (!ctx.Known.Episodes.TryGetValue(id, out var rows))
            return;
        narrowList = Scroll.ActualWidth is > 0 and < global::Kaeri.App.Synopsis.NarrowBelow;
        var (rs, at, plan) = global::Kaeri.App.Synopsis.Layout(rows, chosenRange, opened, DateTimeOffset.UtcNow.ToUnixTimeSeconds());
        ranges = rs;
        outlook = rows.Length == 0 ? EpisodeText.None : KaeriFfiMethods.EpisodeText(rows);
        ShowRanges(rows, at);

        var on = ProtectionOn();
        shownProtection = on;
        var entry = ctx.Known.Entry(id);
        // A row without a still of its own stands on the show's wide art, never blurred.
        var wide = entry?.BannerUrl ?? entry?.CoverUrl;
        var before = new Dictionary<string, EpisodeItem>();
        foreach (var old in Episodes())
            before.TryAdd(old.Row.Number, old);

        var shown = rows[(plan.Start + plan.Fold)..plan.End];
        var start = global::Kaeri.App.Synopsis.ProtectionStart(shown, on);
        // The same rows in the same places, only their state changed (an episode marked
        // watched): each row takes its new state where it stands, and focus stays on it.
        var shape = $"{plan.Start}|{plan.Fold}|{plan.End}|{start?.At}|{start?.Before}|{narrowList}|{on}|{outlook}|{targetNumber}|{string.Join(",", shown.Select(r => r.Number))}";
        var byNumber = shape == shownShape ? shown.GroupBy(r => r.Number).ToDictionary(g => g.Key, g => g.First()) : null;
        if (byNumber is not null && allItems is { } standing
            && standing.OfType<EpisodeItem>().All(item => byNumber.TryGetValue(item.Row.Number, out var row) && OnlyProgress(item.Row, row)))
        {
            foreach (var item in standing.OfType<EpisodeItem>())
            {
                if (!byNumber.TryGetValue(item.Row.Number, out var now))
                    continue;
                var protectable = global::Kaeri.App.Synopsis.Protectable(now, on);
                var changed = now != item.Row;
                item.Take(now);
                if (changed)
                    item.Lay(narrowList, outlook, protectable, Held(now), true);
            }
            ShowDownloads(ctx.Known.Downloads);
            return;
        }
        shownShape = shape;
        var items = new List<object>();
        protection = null;
        if (plan.Fold > 0)
        {
            var first = global::Kaeri.App.Synopsis.Number(rows[plan.Start].Number) ?? 1;
            var last = global::Kaeri.App.Synopsis.Number(rows[plan.Start + plan.Fold - 1].Number) ?? first;
            items.Add(new NoteItem(true, 0, "", T.TitleWatchedRange(first, last)));
        }
        for (var i = 0; i < shown.Length; i++)
        {
            if (start is { } line && line.At == i)
            {
                protection = new NoteItem(false, line.Before, "", "");
                SayProtection();
                items.Add(protection);
            }
            var row = shown[i];
            var item = new EpisodeItem(row, id) { Target = row.Number == targetNumber };
            var known = before.GetValueOrDefault(row.Number);
            if (known is not null)
                item.Inherit(known);
            item.Lay(narrowList, outlook, global::Kaeri.App.Synopsis.Protectable(row, on), Held(row), known is not null);
            ctx.Art.Bind(row.Thumbnail ?? wide, s => item.Still = s);
            items.Add(item);
        }
        // A screenful of rows now, so the page shows at once; the rest follow a few at a time
        // while the UI thread is idle. Each row is a still, a ring, a check box and a menu, and a
        // long season built whole held the page's first frame back.
        allItems = items;
        var target = targetNumber is null ? -1 : items.FindIndex(i => i is EpisodeItem e && e.Row.Number == targetNumber);
        var built = Math.Min(items.Count, Math.Max(FirstRows, target + 4));
        var showing = new System.Collections.ObjectModel.ObservableCollection<object>(items.Take(built));
        EpisodeList.ItemsSource = showing;
        if (built < items.Count)
            MoreRows(showing, items);
        EpisodesState.Done();
        ShowDownloads(ctx.Known.Downloads);
        var metadataOnly = rows.Length > 0 && rows.All(r => r.MetadataOnly);
        EpisodesNote.Text = metadataOnly ? T.TitleEpisodesNoSource : "";
        EpisodesNote.Visibility = metadataOnly ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>The range picker beside the season picker: shown for a show of over 100 episodes.</summary>
    void ShowRanges(EpisodeRow[] rows, int selected)
    {
        shownRange = selected;
        RangeMenu.Items.Clear();
        if (ranges.Length > 1)
        {
            var items = global::Kaeri.App.Synopsis.RangeItems(rows, ranges);
            for (var i = 0; i < items.Length; i++)
            {
                var item = items[i];
                var at = i;
                var entry = new RadioMenuFlyoutItem
                {
                    Text = item.State is null ? item.Label : $"{item.Label} · {item.State}",
                    GroupName = "ranges",
                    IsChecked = i == selected,
                };
                AutomationProperties.SetName(entry, item.Spoken);
                entry.Click += (_, _) => OnRangePicked(at);
                RangeMenu.Items.Add(entry);
            }
            RangePicker.Content = items[Math.Min(selected, items.Length - 1)].Label;
            // The menu ends in Go to episode, after a rule.
            RangeMenu.Items.Add(new MenuFlyoutSeparator());
            var go = new MenuFlyoutItem { Text = T.EpisodeGoTo, KeyboardAcceleratorTextOverride = "Ctrl+G" };
            go.Click += (_, _) => OpenGoTo();
            RangeMenu.Items.Add(go);
        }
        if (ranges.Length > 1)
            Motion.Show(RangePicker);
        else
            Motion.Set(RangePicker, false);
    }

    /// <summary>
    /// A range was picked: the page as it stood is kept as a picture over the new rows and fades
    /// away on the standard token, so the old list and the new crossfade in place. The pickers
    /// come into view first when they were above the screen.
    /// </summary>
    async void OnRangePicked(int at)
    {
        if (at < 0 || at >= ranges.Length)
            return;
        RenderTargetBitmap? before = null;
        if (Motion.Enabled && Scroll.ActualWidth > 0 && Scroll.ActualHeight > 0)
        {
            try
            {
                before = new RenderTargetBitmap();
                await before.RenderAsync(Scroll);
            }
            catch (Exception)
            {
                // Nothing to fade from: the new rows simply show.
                before = null;
            }
        }
        chosenRange = at;
        opened = false;
        ShowEpisodes();
        BringPickersIntoView();
        if (before is null)
            return;
        RangeGhost.Source = before;
        RangeGhost.Width = Scroll.ActualWidth;
        RangeGhost.Height = Scroll.ActualHeight;
        RangeGhost.Opacity = 1;
        RangeGhost.Visibility = Visibility.Visible;
        Motion.HideStandard(RangeGhost);
    }

    /// <summary>Scrolls the episode pickers to the top of the view when they are above it.</summary>
    void BringPickersIntoView()
    {
        if (RangePicker.Parent is not FrameworkElement row || Scroll.Content is not UIElement content)
            return;
        var top = row.TransformToVisual(content).TransformPoint(new global::Windows.Foundation.Point(0, 0)).Y;
        if (Scroll.VerticalOffset > top)
            Scroll.ChangeView(null, Math.Max(0, top), null, true);
    }

    /// <summary>The protection line's words, for whether the rows are showing.</summary>
    void SayProtection()
    {
        if (protection is not { } note)
            return;
        note.Say(revealAll ? T.EpisodeProtectionShown(note.Episode)
                : narrowList ? T.EpisodeProtectionLineShort(note.Episode) : T.EpisodeProtectionLine(note.Episode),
            revealAll ? T.EpisodeProtectionHide : T.EpisodeProtectionShow);
    }

    /// <summary>Every row takes the layout the width, the text outlook and protection give now.</summary>
    void Relay(bool animate, EpisodeItem? only = null)
    {
        var on = ProtectionOn();
        foreach (var item in Episodes().Where(i => only is null || i == only))
            item.Lay(narrowList, outlook, global::Kaeri.App.Synopsis.Protectable(item.Row, on), Held(item.Row), animate);
    }

    void OnNote(object sender, RoutedEventArgs e)
    {
        if (ItemOfNote(sender) is not { } note)
            return;
        if (note.Fold)
        {
            opened = true;
            ShowEpisodes();
            return;
        }
        revealAll = !revealAll;
        if (!revealAll)
            revealed.Clear();
        SayProtection();
        Relay(true);
    }

    static NoteItem? ItemOfNote(object sender) => (sender as FrameworkElement)?.Tag as NoteItem;

    /// <summary>Show synopsis on one protected episode: its still and its text, for this visit.</summary>
    void OnEpisodeReveal(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { } item)
            return;
        revealed.Add(item.Row.Number);
        Relay(true, item);
    }

    static T Res<T>(string key) => (T)Application.Current.Resources[key];

    /// <summary>
    /// The rows' blur, after spoiler protection changed in Settings or for this show: the rows are
    /// laid again, each keeping its item, so the focus and the scroll place stay and the blur follows.
    /// </summary>
    void ShowSpoilers()
    {
        if (ProtectionOn() != shownProtection)
            ShowEpisodes();
    }

    /// <summary>Each episode row's download cell, from the queue.</summary>
    void ShowDownloads(DownloadRow[] rows)
    {
        var items = Episodes().ToList();
        foreach (var item in items)
            item.Download = EpisodeDownload.Of(rows, id, item.Row.Number);
#if KAERI_TESTING
        // A shot's hover: the row after the one playing next, as if the pointer were on it.
        if (ctx.Options.Show == "episode-download" && ctx.Known.Entry(id)?.Progress?.Next is { } next)
            foreach (var item in items.Where(i => i.Row.Number == (next + 1).ToString()))
            {
                item.Reveal = true;
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
                    (EpisodeList.ContainerFromItem(item) as UIElement)?.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0.5, AnimationDesired = false }));
            }
#endif
    }

    readonly HashSet<ListViewItem> watchedContainers = [];

    /// <summary>A row's container: its pointer and focus reveal the row's Download, whichever episode it carries now.</summary>
    void OnEpisodeContainer(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        if (args.ItemContainer is not ListViewItem container || !watchedContainers.Add(container))
            return;
        static void Set(object sender, bool on)
        {
            if (sender is ListViewItem { Content: EpisodeItem item } c)
                item.Reveal = on || c.FocusState != FocusState.Unfocused;
        }
        container.PointerEntered += (s, _) => Set(s, true);
        container.PointerExited += (s, _) => Set(s, false);
        container.PointerCanceled += (s, _) => Set(s, false);
        container.GotFocus += (s, _) => Set(s, true);
        container.LostFocus += (s, _) =>
        {
            // Focus moving within the row, to its Download, keeps the cell shown.
            if (s is ListViewItem { Content: EpisodeItem item } c && XamlRoot is { } root
                && FocusManager.GetFocusedElement(root) is DependencyObject now && IsWithin(now, c))
                return;
            Set(s, false);
        };
    }

    static bool IsWithin(DependencyObject element, DependencyObject ancestor)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == ancestor)
                return true;
        return false;
    }

    void OnDownloadCellFocus(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            item.Reveal = true;
    }

    /// <summary>The download cell: Download when there is none or it failed, else the Downloads page.</summary>
    void OnDownloadCell(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { Playable: true } item)
            return;
        if (item.Download.Asks)
            ctx.Send(new Command.DownloadEpisode(id, item.Row.Number));
        else
            ctx.Go("downloads");
    }

    void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        Layout();
        // A narrow window stacks each row's text under its still.
        var narrow = Scroll.ActualWidth is > 0 and < global::Kaeri.App.Synopsis.NarrowBelow;
        if (narrow != narrowList && EpisodeList.ItemsSource is not null)
        {
            narrowList = narrow;
            SayProtection();
            Relay(false);
        }
        if (Current() is { } entry)
            ShowDetails(entry);
    }

    void OnPrimary(object sender, RoutedEventArgs e)
    {
        if (ctx.Known.Matches.Asks(id))
        {
            OpenMatchPicker(fromMenu: false);
            return;
        }
        if (action is { Playable: true })
            ctx.Play(id, action.Episode.ToString());
        else if (Avail is { Case: AvailabilityCase.OnService, Main: { } main })
            OpenService(main);
    }

    void OnEpisodeClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is EpisodeItem { Playable: true } item)
            ctx.Play(id, item.Row.Number);
        else if (e.ClickedItem is EpisodeItem off && Avail is { Case: AvailabilityCase.OnService, Main: { } main } a
                 && uint.TryParse(off.Row.Number, out var n))
        {
            // An episode with its own page opens there; any other opens the title's link.
            var own = a.Episodes?.FirstOrDefault(l => l.Episode == n);
            OpenService(own is null ? main : new ServiceLink(own.Service, own.Url), n);
        }
    }

    void OnWatchedToggle(object sender, RoutedEventArgs e)
    {
        if (sender is CheckBox { Tag: EpisodeItem item } box)
            ctx.Send(new Command.SetWatched(id, [item.Row.Number], box.IsChecked == true));
    }

    static EpisodeItem? ItemOf(object sender) => (sender as FrameworkElement)?.Tag as EpisodeItem;

    void OnEpisodePlay(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { Playable: true } item)
            ctx.Play(id, item.Row.Number);
    }

    void OnEpisodeFromStart(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { Playable: true } item)
            ctx.Play(id, item.Row.Number, fromStart: true);
    }

    /// <summary>An episode on the TV: from where it was left when it is the one being resumed.</summary>
    void OnEpisodeOnTv(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { Playable: true } item || ctx.Known.Entry(id) is not { } entry)
            return;
        var from = action is { } a && a.Episode.ToString() == item.Row.Number ? a.Resume?.Position : null;
        ctx.Tv.Offer(new TvPlay(id, entry.Title, item.Row.Number, from));
    }

    void OnPlayOnTv(object sender, RoutedEventArgs e)
    {
        if (action is { Playable: true } a && ctx.Known.Entry(id) is { } entry)
            ctx.Tv.Offer(new TvPlay(id, entry.Title, a.Episode.ToString(), a.Resume?.Position));
    }

    void OnEpisodeWatched(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            ctx.Send(new Command.SetWatched(id, [item.Row.Number], !item.Row.Completed));
    }

    // The episode's discussion, in a pane on the trailing side; over the page when the window is narrow.

    DiscussionPane? discussion;
    FrameworkElement? opener;
    Microsoft.UI.Xaml.Shapes.Rectangle? scrim;

    void OnEpisodeDiscussion(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item && uint.TryParse(item.Row.Number, out var episode))
            OpenDiscussion(episode, sender as FrameworkElement);
    }

    internal void OpenDiscussion(uint episode, FrameworkElement? from)
    {
        discussion?.Close();
        opener = from;
        var pane = new DiscussionPane(ctx, id, episode);
        discussion = pane;
        pane.Closed += () =>
        {
            if (discussion != pane)
                return;
            discussion = null;
            Shell.Children.Remove(pane);
            if (scrim is not null)
                Shell.Children.Remove(scrim);
            scrim = null;
            ShellColumn(false);
            if (opener?.XamlRoot is not null)
                opener.Focus(FocusState.Programmatic);
        };
        Shell.Children.Add(pane);
        PlaceDiscussion();
        pane.Start();
    }

    /// <summary>Beside the page from 1000 epx of window; over it, with a light dismiss, below.</summary>
    void PlaceDiscussion()
    {
        if (discussion is not { } pane)
            return;
        var window = XamlRoot?.Size.Width ?? Shell.ActualWidth;
        var over = window > 0 && window < DiscussionPane.OverlayBelow;
        Grid.SetRow(pane, 0);
        if (over)
        {
            ShellColumn(false);
            Grid.SetColumn(pane, 0);
            Grid.SetColumnSpan(pane, 2);
            if (scrim is null)
            {
                scrim = new Microsoft.UI.Xaml.Shapes.Rectangle { Fill = (Brush)Application.Current.Resources["ScrimBrush"] };
                Grid.SetColumnSpan(scrim, 2);
                scrim.Tapped += (_, _) => discussion?.Close();
                Shell.Children.Insert(Shell.Children.IndexOf(pane), scrim);
            }
        }
        else
        {
            if (scrim is not null)
                Shell.Children.Remove(scrim);
            scrim = null;
            Grid.SetColumn(pane, 1);
            Grid.SetColumnSpan(pane, 1);
            ShellColumn(true);
        }
    }

    void ShellColumn(bool pane) =>
        Shell.ColumnDefinitions[1].Width = pane ? GridLength.Auto : new GridLength(0);

    void OnShellSized(object sender, SizeChangedEventArgs e) => PlaceDiscussion();

    void OnEpisodeDownload(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { CanDownload: true } item)
            ctx.Send(new Command.DownloadEpisode(id, item.Row.Number));
    }

    /// <summary>Ctrl+Shift+M: the focused episode, or the next one, marked watched (or unwatched).</summary>
    public bool MarkWatched()
    {
        var focused = FocusManager.GetFocusedElement(XamlRoot) as DependencyObject;
        var item = focused is not null ? EpisodeList.ItemFromContainer(focused) as EpisodeItem : null;
        if (item is null && action is not null && Episodes().ToList() is var all)
            item = all.FirstOrDefault(i => i.Row.Number == action.Episode.ToString());
        if (item is null)
            return false;
        ctx.Send(new Command.SetWatched(id, [item.Row.Number], !item.Row.Completed));
        return true;
    }

    void OnShowSettings(object sender, RoutedEventArgs e) => OpenShowSettings();

    void OnPickSource(object sender, RoutedEventArgs e) => OpenSourcePicker();

    void OnFixMatch(object sender, RoutedEventArgs e) => OpenMatchPicker(fromMenu: true);

    /// <summary>The match picker: the question when the page asks one, else (Fix this match) the title searched at once.</summary>
    void OpenMatchPicker(bool fromMenu, string? query = null)
    {
        if (ctx.Known.Entry(id) is { } entry && XamlRoot is { } root)
            _ = MatchPickerDialog.Open(ctx, id, entry.Title, root, fromMenu && !ctx.Known.Matches.Asks(id), query);
    }

    void OpenSourcePicker()
    {
        if (ctx.Known.Entry(id) is { } entry && XamlRoot is { } root)
            _ = SourcePickerDialog.Open(ctx, id, entry.Title, root);
    }

    void OnWatchParty(object sender, RoutedEventArgs e)
    {
        if (action is { Playable: true })
            ctx.Window.ShowParty(new PartyDialog.For.Episode(id, action.Episode.ToString()));
    }

    void OpenShowSettings()
    {
        if (ctx.Known.Entry(id) is { } entry && XamlRoot is { } root)
            _ = ShowSettingsDialog.Open(ctx, id, entry.Title, root);
    }

    /// <summary>Download…: a range of the show's episodes, from the next one to the last by default.</summary>
    async void OnDownloadRange(object sender, RoutedEventArgs e)
    {
        if (XamlRoot is not { } root || !ctx.Known.Episodes.TryGetValue(id, out var rows))
            return;
        // A range skips what no source lists: nothing could play it.
        var playable = EpisodeSources.Downloadable(rows);
        if (playable.Length == 0)
            return;
        var next = action?.Episode.ToString();
        var start = Math.Max(0, Array.IndexOf(playable, next));
        var from = new ComboBox { Header = T.DownloadRangeFrom, MinWidth = 120 };
        var to = new ComboBox { Header = T.DownloadRangeTo, MinWidth = 120 };
        foreach (var n in playable)
        {
            var name = long.TryParse(n, out var whole) ? T.EpisodeNumber(whole) : T.EpisodeNumberText(n);
            from.Items.Add(name);
            to.Items.Add(name);
        }
        from.SelectedIndex = start;
        to.SelectedIndex = playable.Length - 1;
        var pick = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        pick.Children.Add(from);
        pick.Children.Add(to);
        var body = new StackPanel { Spacing = 12 };
        body.Children.Add(pick);
        body.Children.Add(Ui.Text(T.DownloadRangeQualityNote, "HazeCaptionStyle"));
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.DownloadRangeTitle,
            Content = body,
            PrimaryButtonText = T.TitleEpisodeDownload,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        void Check() => dialog.IsPrimaryButtonEnabled = from.SelectedIndex >= 0 && to.SelectedIndex >= from.SelectedIndex;
        from.SelectionChanged += (_, _) => Check();
        to.SelectionChanged += (_, _) => Check();
        if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
            return;
        var chosen = playable[from.SelectedIndex..(to.SelectedIndex + 1)];
        ctx.Send(new Command.DownloadMany(id, chosen));
    }

    void OnMarkPrevious(object sender, RoutedEventArgs e)
    {
        if (action is null || !ctx.Known.Episodes.TryGetValue(id, out var rows))
            return;
        var before = rows.Where(r => uint.TryParse(r.Number, out var n) && n < action.Episode && !r.Completed).Select(r => r.Number).ToArray();
        if (before.Length > 0)
            ctx.Send(new Command.SetWatched(id, before, true));
    }

    void OnTrailer(object sender, RoutedEventArgs e)
    {
        if (ctx.Known.Entry(id) is { } entry)
            TrailerWindow.Play(ctx, entry);
    }

    void ShowStatus(Entry entry)
    {
        StatusButton.Content = entry.ListStatus is { } status ? StatusName(status) : T.LibraryAddToList;
        foreach (var item in StatusMenu.Items.OfType<RadioMenuFlyoutItem>())
            item.IsChecked = item.Tag is LibrarySegment s && s == entry.ListStatus;
    }

    static string StatusName(LibrarySegment status) => status switch
    {
        LibrarySegment.Watching => T.LibraryStatusWatching,
        LibrarySegment.Planning => T.LibraryStatusPlanning,
        LibrarySegment.Completed => T.LibraryStatusCompleted,
        LibrarySegment.Paused => T.LibraryStatusPaused,
        _ => T.LibraryStatusDropped,
    };

    static string MaterialName(SourceMaterial source) => source switch
    {
        SourceMaterial.Original => T.MaterialOriginal,
        SourceMaterial.Manga => T.MaterialManga,
        SourceMaterial.LightNovel => T.MaterialLightNovel,
        SourceMaterial.VisualNovel => T.MaterialVisualNovel,
        SourceMaterial.VideoGame => T.MaterialVideoGame,
        SourceMaterial.Novel => T.MaterialNovel,
        SourceMaterial.Doujinshi => T.MaterialDoujinshi,
        SourceMaterial.Anime => T.MaterialAnime,
        SourceMaterial.WebNovel => T.MaterialWebNovel,
        SourceMaterial.LiveAction => T.MaterialLiveAction,
        SourceMaterial.Game => T.MaterialGame,
        SourceMaterial.Comic => T.MaterialComic,
        SourceMaterial.MultimediaProject => T.MaterialMultimedia,
        SourceMaterial.PictureBook => T.MaterialPictureBook,
        _ => T.RelationOther,
    };

    /// <summary>A related title's line: "Sequel · TV". AniList's relation comes as its wire name, "SIDE_STORY".</summary>
    static string RelationLine(string relation, string? format)
    {
        var name = relation.ToUpperInvariant() switch
        {
            "PREQUEL" => T.RelationPrequel,
            "SEQUEL" => T.RelationSequel,
            "PARENT" => T.RelationParent,
            "SIDE_STORY" => T.RelationSideStory,
            "SPIN_OFF" => T.RelationSpinOff,
            "ALTERNATIVE" => T.RelationAlternative,
            "SUMMARY" => T.RelationSummary,
            "COMPILATION" => T.RelationCompilation,
            "CONTAINS" => T.RelationContains,
            "SAME_UNIVERSE" => T.RelationSameUniverse,
            "CHARACTER" => T.RelationSharesCharacters,
            "ADAPTATION" => T.RelationAdaptation,
            "SOURCE" => T.RelationOriginalWork,
            _ => T.RelationOther,
        };
        return format is { Length: > 0 } f ? T.TitleRelationFormat(name, Wording.Format(f)) : name;
    }

    /// <summary>
    /// What most recently aired and what is next: "E5 out 2d ago · Episode 6 airs in 4d 20h". Latest
    /// first, so a narrow column cuts the countdown before the news. A finished show has neither.
    /// </summary>
    static string BroadcastLine(Entry entry)
    {
        var parts = new List<string>();
        // An episode known out only because the next one's number says so has no time to date it.
        uint? proven = entry.NextEpisode is { } next && next >= 2 ? next - 1 : null;
        if (entry.LastAired is { } aired && !(proven > aired.Episode))
            parts.Add(T.TitleEpisodeOutAgo(aired.Episode, Wording.Ago(aired.Ago)));
        else if (proven is { } out_)
            parts.Add(T.CalendarEpisodeOut(out_));
        if (entry.NextAiringIn is { } seconds && seconds > 0)
            parts.Add(entry.NextEpisode is { } coming ? T.EpisodeAirs(coming, Wording.In(seconds)) : T.TitleNextAirs(Wording.In(seconds)));
        return string.Join(" · ", parts);
    }

    void OnOpenAnilist(object sender, RoutedEventArgs e) =>
        ctx.Send(new Command.OpenExternal($"https://anilist.co/anime/{id}"));
}
