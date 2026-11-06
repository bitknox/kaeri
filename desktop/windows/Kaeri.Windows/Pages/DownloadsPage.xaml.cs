using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Downloads: the queue by title, each title with its cover and its episodes, each episode with
/// what it is doing and what can be done to it; what is seeding after watching; and how much
/// room the downloads take of what the drive has. The engine says where the queue stands once a
/// second while anything downloads, so each title and episode keeps its row from one reading to
/// the next and only what changed is redrawn: a button being pressed is still there when the
/// pointer lets go, and the focus stays where it was.
/// </summary>
public sealed partial class DownloadsPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    readonly Dictionary<uint, TitleView> titles = [];
    readonly Dictionary<long, EpisodeView> episodes = [];
    readonly Dictionary<string, SeedView> seeds = [];
    readonly TextBlock seedingHeading = Ui.Heading(T.DownloadsSeedingAfterWatching);
    readonly TextBlock background = Ui.Haze("");
    StackPanel? empty;

    public DownloadsPage()
    {
        InitializeComponent();
        // Each title's card: its cover and its lines, with the card's own padding between them.
        Loading.Placeholders = () => States.Rows(3, 64, 96, spacing: 32);
        Loading.NeedsConnection = false;
        ctx.Hear(this, Heard);
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.LoadDownloads());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        // A title's name or cover can arrive later than its downloads; the rows take it in place.
        if (batch.Any(e => e is Event.Downloads or Event.Seeding or Event.Settings)
            || batch.Any(e => e is Event.Content) && titles.Values.Any(t => t.Arrived(ctx)))
            Show();
    }

    string? Folder => ctx.Known.Setting(SettingKey.DownloadDir) is { } row ? SettingsCatalog.TypedText(row) : null;

    ulong? free;
    string? freeFor;
    DateTimeOffset freeAt;

    /// <summary>The room left on the downloads' drive, asked of Windows at most every 10 seconds.</summary>
    ulong? Free()
    {
        var folder = Folder;
        var now = DateTimeOffset.UtcNow;
        if (folder == freeFor && now - freeAt < TimeSpan.FromSeconds(10))
            return free;
        freeFor = folder;
        freeAt = now;
        try
        {
            var root = Path.GetPathRoot(string.IsNullOrEmpty(folder) ? AppContext.BaseDirectory : Path.GetFullPath(folder));
            free = root is null ? null : (ulong)new DriveInfo(root).AvailableFreeSpace;
        }
        catch (Exception)
        {
            free = null;
        }
        return free;
    }

    void Show()
    {
        var queue = ctx.Known.Queue;
        if (!queue.Loaded)
        {
            // Never "No downloads" before the engine has said: nothing for 600 ms, then plates.
            Subtitle.Text = "";
            Meter.Visibility = Visibility.Collapsed;
            PauseAll.IsEnabled = ClearDone.IsEnabled = false;
            Loading.Wait();
            return;
        }
        Loading.Done();
        var rows = queue.Rows;
        var used = queue.BytesOnDisk;
        var room = Free();
        Subtitle.Text = room is { } f ? T.DownloadsFreeUsed(Wording.Bytes(f), Wording.Bytes(used)) : T.DownloadsInDownloads(Wording.Bytes(used));
        Meter.Visibility = room is null || used == 0 ? Visibility.Collapsed : Visibility.Visible;
        if (room is { } left)
            Meter.Value = Math.Min(1, (double)used / (used + left));
        var running = rows.Any(r => r.State is DownloadState.Active or DownloadState.Queued);
        PauseAll.IsEnabled = running;
        ClearDone.IsEnabled = rows.Any(r => r.State == DownloadState.Done);

        var wanted = new List<UIElement>();
        if (rows.Length == 0 && ctx.Known.Seeding.Length == 0)
        {
            empty ??= States.Empty(T.DownloadsEmptyTitle, T.DownloadsEmptyDetail);
            wanted.Add(empty);
        }
        else
        {
            var groups = queue.Groups();
            foreach (var gone in titles.Keys.Except(groups.Select(g => g.Id)).ToList())
                titles.Remove(gone);
            foreach (var gone in episodes.Keys.Except(rows.Select(r => r.Id)).ToList())
                episodes.Remove(gone);
            foreach (var group in groups)
            {
                if (!titles.TryGetValue(group.Id, out var title))
                    titles[group.Id] = title = new TitleView(ctx, group.Id);
                var lines = new List<UIElement>();
                foreach (var row in group.Rows)
                {
                    if (!episodes.TryGetValue(row.Id, out var episode))
                        episodes[row.Id] = episode = new EpisodeView(this);
                    episode.Update(row, queue);
                    lines.Add(episode.Root);
                }
                title.Update(ctx, group, lines);
                wanted.Add(title.Card);
            }
            var seeding = ctx.Known.Seeding;
            foreach (var gone in seeds.Keys.Except(seeding.Select(s => s.Name)).ToList())
                seeds.Remove(gone);
            if (seeding.Length > 0)
            {
                wanted.Add(seedingHeading);
                foreach (var seed in seeding)
                {
                    if (!seeds.TryGetValue(seed.Name, out var view))
                        seeds[seed.Name] = view = new SeedView();
                    view.Update(seed);
                    wanted.Add(view.Card);
                }
            }
            if (running)
            {
                background.Text = ctx.Window.WhenClosing switch
                {
                    CloseChoice.KeepGoing => T.DownloadsBackgroundKeeps,
                    CloseChoice.Stop => T.DownloadsBackgroundStops,
                    _ => T.DownloadsBackgroundAsks,
                };
                wanted.Add(background);
            }
        }
        Drawn.Arrange(Body.Children, wanted, Ui.HoldingFocus(Body, Body.Children));
    }

    /// <summary>A title's card: its cover, its name and size, and its episodes' rows under them.</summary>
    sealed class TitleView
    {
        public readonly Border Card;
        readonly Grid grid = new() { ColumnSpacing = 16 };
        readonly TextBlock name = Ui.Text("", "BodyStrongTextBlockStyle", wrap: false);
        readonly TextBlock line = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly StackPanel column = new() { Spacing = 6 };
        readonly StackPanel head = new() { Spacing = 1 };
        readonly uint id;
        Border cover;
        bool coverHasEntry;

        public TitleView(Ctx ctx, uint id)
        {
            this.id = id;
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            cover = MakeCover(ctx);
            grid.Children.Add(cover);
            Grid.SetColumn(column, 1);
            head.Children.Add(name);
            head.Children.Add(line);
            column.Children.Add(head);
            grid.Children.Add(column);
            Card = Ui.Card(grid);
        }

        Border MakeCover(Ctx ctx)
        {
            coverHasEntry = ctx.Known.Entries.ContainsKey(id);
            var made = Ui.Cover(ctx, id, 64, 96);
            made.VerticalAlignment = VerticalAlignment.Top;
            return made;
        }

        /// <summary>Whether the title's name and cover have come from the engine since its card was made.</summary>
        public bool Arrived(Ctx ctx) => !coverHasEntry && ctx.Known.Entries.ContainsKey(id);

        public void Update(Ctx ctx, DownloadGroup group, List<UIElement> lines)
        {
            // The cover is bound once, and again only when the title's art first becomes known.
            if (!coverHasEntry && ctx.Known.Entries.ContainsKey(id))
            {
                grid.Children.Remove(cover);
                cover = MakeCover(ctx);
                grid.Children.Insert(0, cover);
            }
            Set(name, ctx.Known.Entries.GetValueOrDefault(group.Id)?.Title ?? group.Title);
            var bytes = group.BytesOnDisk;
            var count = T.EpisodesCount(group.Rows.Length);
            Set(line, bytes > 0 ? T.DownloadsGroupLine(count, Wording.Bytes(bytes)) : count);
            lines.Insert(0, head);
            Drawn.Arrange(column.Children, lines, Ui.HoldingFocus(column, column.Children));
        }
    }

    /// <summary>
    /// An episode's row. Its buttons are made once and shown for the states they belong to, so
    /// Pause becomes Resume on the same button and keeps the focus.
    /// </summary>
    sealed class EpisodeView
    {
        public readonly Grid Root = new() { MinHeight = 40, ColumnSpacing = 8 };
        readonly TextBlock name = Ui.Text("", wrap: false);
        readonly TextBlock ahead = Ui.Text(T.DownloadsAhead, "HazeCaptionStyle", wrap: false);
        readonly FontIcon done = new() { Glyph = "", FontSize = 12, Foreground = Ui.Res<Brush>("WaterBrush") };
        readonly FontIcon failed = States.ErrorGlyph(12);
        readonly TextBlock words = Ui.Haze("", wrap: false);
        readonly ProgressBar bar = new()
        {
            Minimum = 0, Maximum = 1, Height = 4, MaxWidth = 420, Width = 420,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        readonly StackPanel actions = new() { Orientation = Orientation.Horizontal, Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        readonly Button play, folder, delete, startNow, tryAgain, pause, remove;
        readonly DownloadsPage page;
        DownloadRow? row;

        public EpisodeView(DownloadsPage page)
        {
            this.page = page;
            var ctx = page.ctx;
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var text = new StackPanel { Spacing = 3, VerticalAlignment = VerticalAlignment.Center };
            var top = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            top.Children.Add(name);
            top.Children.Add(ahead);
            text.Children.Add(top);
            var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            state.Children.Add(done);
            state.Children.Add(failed);
            state.Children.Add(words);
            text.Children.Add(state);
            text.Children.Add(bar);
            Root.Children.Add(text);

            play = Ui.Icon("", "", (_, _) =>
            {
                if (row is { Path: { } path } r)
                    ctx.Send(new Command.PlayLocal(r.AnilistId, r.Episode, r.Title, path));
            });
            folder = Ui.Icon("", T.DownloadsShowInFolder, (_, _) => ShowInFolder(row?.Path));
            delete = Ui.Icon("", T.ActionDelete, (_, _) =>
            {
                if (row is { } r)
                    page.ConfirmDelete(r, T.EpisodeLabel(r.Episode));
            });
            // To the front of the queue, at once: the last one running waits behind it.
            startNow = Ui.Label(T.DownloadsStartNow, (_, _) => Send(r => new Command.DownloadStartNow(r.Id)));
            tryAgain = Ui.Label(T.ActionTryAgain, (_, _) => Send(r => new Command.DownloadEpisode(r.AnilistId, r.Episode)));
            pause = Ui.Icon("", T.DownloadsPauseOne, (_, _) => Send(r => new Command.DownloadPause(r.Id)));
            remove = Ui.Icon("", T.DownloadsRemoveFromQueue, (_, _) => Send(r => new Command.DownloadCancel(r.Id)));
            foreach (var button in new[] { play, folder, delete, startNow, tryAgain, pause, remove })
                actions.Children.Add(button);
            Grid.SetColumn(actions, 1);
            Root.Children.Add(actions);
        }

        void Send(Func<DownloadRow, Command> command)
        {
            if (row is { } r)
                page.ctx.Send(command(r));
        }

        public void Update(DownloadRow now, DownloadQueue queue)
        {
            var was = row;
            row = now;
            var label = T.EpisodeLabel(now.Episode);
            Set(name, label);
            Reveal(ahead, now.Ahead);

            Reveal(done, now.State == DownloadState.Done);
            Reveal(failed, now.State == DownloadState.Failed);
            Set(words, Words(now, queue));
            ToolTipService.SetToolTip(words, now.State == DownloadState.Failed ? words.Text : null);

            var progress = now.State is DownloadState.Active or DownloadState.Paused && now.Total > 0;
            Reveal(bar, progress);
            if (progress)
            {
                bar.Value = now.Fraction;
                // Water while it runs, haze while it waits: a state, never the accent.
                if (was?.State != now.State)
                    bar.Foreground = Ui.Res<Brush>(now.State == DownloadState.Active ? "WaterBrush" : "HazeBrush");
                if (was?.Episode != now.Episode)
                    AutomationProperties.SetName(bar, T.DownloadsProgressName(label));
            }

            if (was?.State == now.State && was.Episode == now.Episode)
                return;
            // A button that goes with its state hands the focus to the row's next one.
            var focused = Ui.HoldingFocus(actions, actions.Children.OfType<Button>());
            var how = focused?.FocusState ?? FocusState.Unfocused;
            Reveal(play, now.State == DownloadState.Done);
            Reveal(folder, now.State == DownloadState.Done);
            Reveal(delete, now.State == DownloadState.Done);
            Reveal(startNow, now.State == DownloadState.Queued);
            Reveal(tryAgain, now.State == DownloadState.Failed);
            Reveal(pause, now.State is DownloadState.Active or DownloadState.Paused);
            Reveal(remove, now.State != DownloadState.Done);
            var playName = T.DownloadsPlayLabel(now.Episode);
            ToolTipService.SetToolTip(play, playName);
            AutomationProperties.SetName(play, playName);
            var resume = now.State == DownloadState.Paused;
            ((FontIcon)pause.Content).Glyph = resume ? "" : "";
            var pauseName = resume ? T.DownloadsResumeOne : T.DownloadsPauseOne;
            ToolTipService.SetToolTip(pause, pauseName);
            AutomationProperties.SetName(pause, pauseName);
            if (focused is { Visibility: Visibility.Collapsed } && actions.Children.OfType<Button>().FirstOrDefault(b => b.Visibility == Visibility.Visible) is { } next)
                next.Focus(how == FocusState.Unfocused ? FocusState.Programmatic : how);
        }

        string Words(DownloadRow row, DownloadQueue queue)
        {
            switch (row.State)
            {
                case DownloadState.Done:
                    return row.Ahead ? T.DownloadsCaptionAhead : T.DownloadsCaptionDone;
                case DownloadState.Active when row.Total == 0:
                    return T.DownloadsCaptionMetadata;
                case DownloadState.Active:
                {
                    var text = T.DownloadsCaptionRunning(Wording.Bytes(row.Downloaded), Wording.Bytes(row.Total));
                    if (queue.Rates.TryGetValue(row.Id, out var rate) && rate > 0)
                        text = T.DownloadsLineRate(text, Wording.Rate((ulong)rate));
                    if (queue.SecondsLeft(row) is { } left)
                        text = T.DownloadsLineLeft(text, Wording.Countdown(left));
                    return text;
                }
                case DownloadState.Paused:
                    return T.DownloadsCaptionPaused(Wording.Percent(row.Fraction * 100));
                case DownloadState.Queued:
                    return T.DownloadsCaptionQueuedSlot;
                default:
                    // The glyph is the one vermilion thing; the words are haze, with the reason the
                    // engine kept on the row when it has one.
                    return row.Error is { Length: > 0 } why ? T.DownloadsCaptionFailedWhy(why) : T.DownloadsFailedRow;
            }
        }
    }

    /// <summary>A file seeding after it was watched: its ratio towards the goal, and its size.</summary>
    sealed class SeedView
    {
        public readonly Border Card;
        readonly TextBlock name = Ui.Text("", wrap: false);
        readonly TextBlock line = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly ProgressBar bar = new() { Minimum = 0, Maximum = 1, Height = 4, Width = 420, HorizontalAlignment = HorizontalAlignment.Left };
        readonly TextBlock size = Ui.Haze("", wrap: false);

        public SeedView()
        {
            var grid = new Grid { ColumnSpacing = 12 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var text = new StackPanel { Spacing = 3 };
            text.Children.Add(name);
            text.Children.Add(line);
            text.Children.Add(bar);
            grid.Children.Add(text);
            size.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(size, 1);
            grid.Children.Add(size);
            Card = grid.WrapCard();
        }

        public void Update(SeedRow seed)
        {
            Set(name, seed.Name);
            Set(line, T.DownloadsSeedingLine(Wording.Number(seed.Ratio, 1), Wording.Number(seed.Goal, 1), Wording.Countdown((long)seed.LeftSecs)));
            bar.Value = seed.Goal > 0 ? Math.Min(1, seed.Ratio / seed.Goal) : 0;
            Reveal(size, seed.Size() is not null);
            if (seed.Size() is { } bytes)
                Set(size, Wording.Bytes(bytes));
        }
    }

    /// <summary>Sets a line's words when they changed, so an unchanged line is not laid out again.</summary>
    static void Set(TextBlock block, string text)
    {
        if (block.Text != text)
            block.Text = text;
    }

    static void Reveal(UIElement element, bool shown) =>
        element.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;

    async void ConfirmDelete(DownloadRow row, string name)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = T.DownloadsDeleteTitle(name),
            Content = T.DownloadsDeleteDetail,
            PrimaryButtonText = T.ActionDelete,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        if (await Dialogs.Show(dialog) == ContentDialogResult.Primary)
            ctx.Send(new Command.DownloadDelete(row.Id));
    }

    static async void ShowInFolder(string? path)
    {
        if (path is null)
            return;
        try
        {
            var folder = Path.GetDirectoryName(path);
            if (folder is not null)
                await global::Windows.System.Launcher.LaunchFolderPathAsync(folder);
        }
        catch (Exception)
        {
            // No folder to show: nothing to do.
        }
    }

    void OnPauseAll(object sender, RoutedEventArgs e) => ctx.PauseAllDownloads();

    void OnClearCompleted(object sender, RoutedEventArgs e) => ctx.Send(new Command.DownloadClearCompleted());

    async void OnOpenFolder(object sender, RoutedEventArgs e)
    {
        try
        {
            if (Folder is { Length: > 0 } folder && Directory.Exists(folder))
                await global::Windows.System.Launcher.LaunchFolderPathAsync(folder);
        }
        catch (Exception)
        {
            // No folder yet: nothing to open.
        }
    }
}

static class CardExtensions
{
    public static Border WrapCard(this UIElement child) => Ui.Card(child);
}
