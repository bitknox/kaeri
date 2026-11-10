using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.Engine.Model;

namespace Kaeri.App;

// The open reader: one session at a time, what the engine said of it, and what the reader view
// asks. The twin of the Apple app's ReaderModel; the view on Windows is Kaeri.Windows.Reader.

/// <summary>A page of a chapter as the engine names it. A split spread has a file for each half.</summary>
public readonly record struct ReaderPageKey(string Chapter, uint Page, PageHalf? Half = null);

/// <summary>What the end page says when there is nothing to continue to.</summary>
public sealed record ReaderEnd(ChapterRow? Next, NoNext? Why, string? NextSource);

/// <summary>The notice over the page, with the action it offers.</summary>
public sealed record ReaderNoticeLine(int Id, string Text, ReaderAction? Action);

public enum ReaderPhase { Closed, Opening, Open, Failed }

/// <summary>What changed in the session, so the view redraws only that.</summary>
public enum ReaderChange
{
    Phase, Leaves, Page, Novel, NovelStyle, End, Settings, Notice, BackTo, GoTo, NovelGoTo,
    Chapters, Read, Preview, Opening, Sent,
}

public sealed class ReaderSession
{
    internal Action<Command>? Send { get; set; }

    /// <summary>The engine's synchronous answer for the web view's resources.</summary>
    public Func<string, BookResource?>? BookResource { get; set; }

    /// <summary>
    /// The headers a web view's response carries for <paramref name="answer"/>, one per line.
    /// One Content-Type only, the engine's own with its charset when it sends one. With two,
    /// Chromium may not know the type, and with nosniff it may save the page instead of showing it.
    /// </summary>
    public static string ResponseHeaders(BookResource answer)
    {
        var type = answer.Headers.FirstOrDefault(h => h.Name.Equals("Content-Type", StringComparison.OrdinalIgnoreCase))?.Value;
        var lines = new System.Text.StringBuilder();
        lines.Append("Content-Type: ").Append(type ?? answer.Mime).Append("\r\n");
        foreach (var header in answer.Headers)
            if (!header.Name.Equals("Content-Type", StringComparison.OrdinalIgnoreCase))
                lines.Append(header.Name).Append(": ").Append(header.Value).Append("\r\n");
        return lines.ToString();
    }

    public sealed record Request(string Work, string? Chapter, OpenAt At, int Tick);

    public ReaderPhase Phase { get; private set; } = ReaderPhase.Closed;
    public ReadingFailure? Failure { get; private set; }
    /// <summary>The reader shows while this is set: the work and chapter asked for.</summary>
    public Request? Asked { get; private set; }
    public ReaderView? View { get; private set; }
    public Leaf[] Leaves { get; private set; } = [];
    public Dictionary<ReaderPageKey, PageFile> Pages { get; } = [];
    public Dictionary<ReaderPageKey, ReadingFailure> PageFailures { get; } = [];
    public NovelOpenView? Novel { get; private set; }
    public string NovelStyle { get; private set; } = "";
    public string NovelCountKey { get; private set; } = "";
    public uint[] NovelCounted { get; private set; } = [];
    public ReaderEnd? End { get; private set; }
    public ReaderSettingsView? Settings { get; private set; }
    public ReaderNoticeLine? Notice { get; private set; }
    /// <summary>
    /// The page "Back to page 8" offers, from 1, over the scrubber while the controls are up.
    /// The engine offers it after the person's jump and takes it away at the next turn.
    /// </summary>
    public uint? BackTo { get; private set; }
    /// <summary>The leaf shown now.</summary>
    public int Current { get; private set; }
    /// <summary>Where the person was before the last jump, as a leaf, for the ring on the scrubber.</summary>
    public int? Ring { get; private set; }
    /// <summary>Where a strip is asked to scroll to once its images are in.</summary>
    public StripPos? StripRestore { get; private set; }
    /// <summary>The chapters of the work, once the engine has listed them.</summary>
    public VolumeGroup[] Chapters { get; private set; } = [];
    public bool ChaptersLoaded { get; private set; }
    /// <summary>When the open began, for the opening line's wait.</summary>
    public DateTimeOffset? OpenedAt { get; private set; }
    /// <summary>Whether the chapter counted as read.</summary>
    public bool Read { get; private set; }
    /// <summary>The trackers the read went to, for the end page's line.</summary>
    public string[] ReadTrackers { get; private set; } = [];
    /// <summary>What the book server said of the read chapter's progress, for the end page.</summary>
    public (string Host, WriteBackResult Result)? ServerProgress { get; private set; }
    /// <summary>The id of the book server <see cref="ServerProgress"/> names, for Try again.</summary>
    public string? ServerProgressServer { get; private set; }
    /// <summary>
    /// The read chapter's progress is on its way to this book server, which has not answered yet:
    /// from <see cref="ReaderEvent.ChapterRead"/> until <see cref="ReaderEvent.ProgressSent"/>. The
    /// end page shows "Sending to" once it has waited 600 ms from <c>Since</c>.
    /// </summary>
    public (string Host, DateTimeOffset Since)? ServerSending { get; private set; }

    /// <summary>A place the paged reader is told to show (Back to page, Start over, another device's).</summary>
    public (int Leaf, int Tick)? GoTo { get; private set; }
    /// <summary>A novel place the web view is told to show.</summary>
    public (NovelPos Pos, int Tick)? NovelGoTo { get; private set; }
    /// <summary>The next chapter's first page, for the end page and a strip's divider.</summary>
    public PageFile? NextPreview { get; private set; }
    /// <summary>What an open waits on, for the opening screen.</summary>
    public (ChapterRow? Chapter, ReaderWait? Wait)? Opening { get; private set; }
    /// <summary>The paginator laid a page out and said where it is.</summary>
    public bool NovelPainted { get; private set; }

    public event Action<ReaderChange>? Changed;

    ReaderSurface? surface, sentSurface;
    ulong session;
    int ticks, noticeCount;
    bool opened;
    /// <summary>The reader has moved off the chapter's start: a new layout keeps the page on screen.</summary>
    bool moved;
    /// <summary>Opens let go of before the engine named their session, until the engine says they went.</summary>
    int dropped;
    int? pendingGoTo;
    Request? afterClose;
    CancellationTokenSource? noticeTimer;
    DateTimeOffset lastStrip = DateTimeOffset.MinValue;
    CancellationTokenSource? pendingStrip;

    public ulong SessionId => session;
    public Medium Medium => View?.Medium ?? Medium.Manga;
    public bool IsOpen => Asked is not null;

    int Bump() => ++ticks;

    void Ask(ReaderCommand command) => Send?.Invoke(new Command.Reader(command));

    // Asking

    /// <summary>
    /// Open <paramref name="work"/> at <paramref name="chapter"/>, or where the person stopped.
    /// The view reports its size with <see cref="SurfaceChanged"/>, and the engine hears of the
    /// open then.
    /// </summary>
    public void Open(string work, string? chapter = null, OpenAt? at = null)
    {
        var next = new Request(work, chapter, at ?? new OpenAt.Resume(), Bump());
        if (session != 0)
        {
            // One at a time: the first closes, then this opens.
            Ask(new ReaderCommand.Close(session));
            afterClose = next;
            Reset();
            Asked = null;
            Changed?.Invoke(ReaderChange.Phase);
            return;
        }
        Begin(next);
    }

    void Begin(Request next)
    {
        Reset();
        Asked = next;
        Phase = ReaderPhase.Opening;
        OpenedAt = DateTimeOffset.UtcNow;
        SendOpen();
        Changed?.Invoke(ReaderChange.Phase);
    }

    void SendOpen()
    {
        if (Asked is not { } asked || surface is not { } at || Phase != ReaderPhase.Opening || session != 0 || opened)
            return;
        opened = true;
        Ask(new ReaderCommand.Open(asked.Work, asked.Chapter, asked.At, at));
        Send?.Invoke(new Command.Reading(new ReadingCommand.LoadChapters(asked.Work, null)));
    }

    /// <summary>The reader's view has a size: the engine lays out for it, and opens on it the first time.</summary>
    public void SurfaceChanged(ReaderSurface next)
    {
        if (surface == next)
            return;
        surface = next;
        if (Asked is not null && !opened)
        {
            SendOpen();
            return;
        }
        if (session == 0 || sentSurface == next)
            return;
        sentSurface = next;
        Ask(new ReaderCommand.Surface(session, next));
    }

    public void Close()
    {
        // A book waiting for this one to close doesn't open behind a reader that has gone.
        afterClose = null;
        if (Asked is null && session == 0)
            return;
        if (session != 0)
            Ask(new ReaderCommand.Close(session));
        else if (opened)
        {
            // The engine has the open but hasn't named its session: session 0 drops the open
            // still waiting, so nothing more is fetched for a reader that has gone.
            dropped++;
            Ask(new ReaderCommand.Close(0));
        }
        Reset();
        Asked = null;
        Phase = ReaderPhase.Closed;
        Changed?.Invoke(ReaderChange.Phase);
    }

    /// <summary>A leaf was painted.</summary>
    public void Shown(int leaf, TurnHow how)
    {
        if (session == 0 || View is not { } view || leaf < 0 || leaf >= Leaves.Length)
            return;
        if (how == TurnHow.Jump && leaf != Current)
            Ring = Current;
        Current = leaf;
        moved = true;
        Ask(new ReaderCommand.Shown(session, view.Chapter.Id, Leaves[leaf].Index, how));
    }

    /// <summary>A strip settled: the image and how far down it, at most every 500 ms.</summary>
    public async void StripAt(StripPos at, bool footVisible)
    {
        if (session == 0 || View is not { } view)
            return;
        pendingStrip?.Cancel();
        var since = DateTimeOffset.UtcNow - lastStrip;
        if (since < TimeSpan.FromMilliseconds(500))
        {
            var wait = pendingStrip = new CancellationTokenSource();
            try
            {
                await Task.Delay(TimeSpan.FromMilliseconds(500) - since, wait.Token);
            }
            catch (TaskCanceledException)
            {
                return;
            }
        }
        if (session == 0 || View?.Chapter.Id != view.Chapter.Id)
            return;
        lastStrip = DateTimeOffset.UtcNow;
        Ask(new ReaderCommand.StripAt(session, view.Chapter.Id, at, footVisible));
    }

    /// <summary>The paginator turned a page.</summary>
    public void NovelAt(NovelPos pos, int page, int pages, TurnHow how)
    {
        if (session == 0 || View is not { } view)
            return;
        NovelPainted = true;
        Ask(new ReaderCommand.NovelAt(session, view.Chapter.Id, pos, (uint)Math.Max(page, 0), (uint)Math.Max(pages, 0), how));
    }

    /// <summary>The paginator counted the volume's pages under the style's key.</summary>
    public void NovelCountedPages(string key, IEnumerable<uint> chapterPages)
    {
        if (session == 0)
            return;
        Ask(new ReaderCommand.NovelCounted(session, key, [.. chapterPages]));
    }

    public void OpenChapter(string id)
    {
        if (session == 0)
            return;
        End = null;
        Read = false;
        ClearNotices();
        Ask(new ReaderCommand.OpenChapter(session, id));
        Phase = ReaderPhase.Opening;
        OpenedAt = DateTimeOffset.UtcNow;
        Changed?.Invoke(ReaderChange.Phase);
    }

    /// <summary>
    /// A strip's marker into the next chapter reached the window's middle: the next chapter opens
    /// under it in the same scroll, with no opening screen and no kept place. The engine opens a
    /// strip's next chapter at its first image and says nothing of where it was.
    /// </summary>
    public void FlowOn()
    {
        if (session == 0 || Flowing || View?.Next is not { } next)
            return;
        Flowing = true;
        End = null;
        Read = false;
        ClearNotices();
        Ask(new ReaderCommand.OpenChapter(session, next.Id));
    }

    /// <summary>A strip is flowing into the next chapter: from <see cref="FlowOn"/> until the strip took its images.</summary>
    public bool Flowing { get; private set; }

    /// <summary>The strip has the flowing chapter's images in its scroll.</summary>
    public void Flowed() => Flowing = false;

    public void Retry(uint? page)
    {
        if (session == 0 && Asked is { } asked && Phase == ReaderPhase.Failed)
        {
            // Failed before the engine named a session: open again.
            Begin(asked with { Tick = Bump() });
            return;
        }
        if (session == 0 || View is not { } view)
            return;
        if (page is { } p)
            foreach (var key in PageFailures.Keys.Where(k => k.Page == p).ToList())
                PageFailures.Remove(key);
        Ask(new ReaderCommand.Retry(session, view.Chapter.Id, page));
        if (page is null)
        {
            ClearNotices();
            Phase = ReaderPhase.Opening;
            Failure = null;
            OpenedAt = DateTimeOffset.UtcNow;
            Changed?.Invoke(ReaderChange.Phase);
        }
        else
            Changed?.Invoke(ReaderChange.Page);
    }

    public void NeedPage(uint page)
    {
        if (session == 0 || View is not { } view)
            return;
        Ask(new ReaderCommand.NeedPage(session, view.Chapter.Id, page));
    }

    public void Act(ReaderAction action)
    {
        if (session == 0)
            return;
        switch (action)
        {
            case ReaderAction.StartOver: Ask(new ReaderCommand.StartOver(session)); break;
            case ReaderAction.GoBack: Ask(new ReaderCommand.GoBack(session)); break;
        }
        if (action != ReaderAction.ChangeMode)
            Dismiss(action);
    }

    /// <summary>The notice leaves.</summary>
    public void Dismiss(ReaderAction? action)
    {
        Notice = null;
        noticeTimer?.Cancel();
        noticeTimer = null;
        noticeLeft = null;
        Changed?.Invoke(ReaderChange.Notice);
    }

    /// <summary>Back to the page the last jump left, while the engine offers it.</summary>
    public void GoBackTo()
    {
        if (session == 0 || BackTo is null)
            return;
        BackTo = null;
        Changed?.Invoke(ReaderChange.BackTo);
        Ask(new ReaderCommand.BackTo(session));
    }

    // Settings

    public ReaderSettingRow? Row(ReaderSettingKey key) => Settings?.Rows.FirstOrDefault(r => r.Key == key);

    public string? Choice(ReaderSettingKey key) => Row(key)?.Value is ReaderValue.Choice c ? c.Name : null;

    public double? Amount(ReaderSettingKey key) => Row(key)?.Value is ReaderValue.Amount a ? a.Value : null;

    public bool IsOn(ReaderSettingKey key) => Row(key)?.Value is ReaderValue.Toggle { On: true };

    /// <summary>Change a setting for the medium's default, or for the series with a series scope.</summary>
    public void Set(ReaderSettingKey key, ReaderValue value, SettingScope? scope = null)
    {
        var where = scope ?? HomeScope(key);
        var work = where is SettingScope.Series or SettingScope.SeriesHere ? View?.Work : null;
        Ask(new ReaderCommand.SetSetting(session == 0 ? null : session, Medium, work, key, where, value));
    }

    public void UseDefault(ReaderSettingKey key)
    {
        if (View is not { } view)
            return;
        Ask(new ReaderCommand.UseDefault(Medium, view.Work, key));
    }

    public enum ModeChoice { Auto, RightToLeft, LeftToRight, Vertical, Strip }

    /// <summary>
    /// The reading mode picker: Auto drops the series' choice; the others set mode and direction
    /// for the series, which follows the person.
    /// </summary>
    public void ChooseMode(ModeChoice mode)
    {
        switch (mode)
        {
            case ModeChoice.Auto:
                UseDefault(ReaderSettingKey.Mode);
                UseDefault(ReaderSettingKey.Direction);
                break;
            case ModeChoice.RightToLeft or ModeChoice.LeftToRight:
                Set(ReaderSettingKey.Mode, new ReaderValue.Choice("paged"), SettingScope.Series);
                Set(ReaderSettingKey.Direction, new ReaderValue.Choice(mode == ModeChoice.RightToLeft ? "rtl" : "ltr"), SettingScope.Series);
                break;
            case ModeChoice.Strip:
                Set(ReaderSettingKey.Mode, new ReaderValue.Choice("strip"), SettingScope.Series);
                break;
            case ModeChoice.Vertical:
                Set(ReaderSettingKey.Mode, new ReaderValue.Choice("vertical"), SettingScope.Series);
                break;
        }
    }

    /// <summary>Whether the series has a choice of its own for the reading mode.</summary>
    public bool ModeOverridden => (Row(ReaderSettingKey.Mode)?.Overridden ?? false) || (Row(ReaderSettingKey.Direction)?.Overridden ?? false);

    /// <summary>The mode the picker shows as chosen.</summary>
    public ModeChoice ChosenMode =>
        !ModeOverridden ? ModeChoice.Auto
        : Choice(ReaderSettingKey.Mode) == "strip" ? ModeChoice.Strip
        : Choice(ReaderSettingKey.Mode) == "vertical" ? ModeChoice.Vertical
        : Choice(ReaderSettingKey.Direction) == "ltr" ? ModeChoice.LeftToRight
        : ModeChoice.RightToLeft;

    /// <summary>Where a setting is kept when the reader changes it: the choices follow the person, the layout stays on the device.</summary>
    public static SettingScope HomeScope(ReaderSettingKey key) => key switch
    {
        ReaderSettingKey.Mode or ReaderSettingKey.Direction or ReaderSettingKey.Colour
            or ReaderSettingKey.Look or ReaderSettingKey.Font => SettingScope.Medium,
        _ => SettingScope.Device,
    };

    // Hearing

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.Reader reader:
                Apply(reader.V1);
                break;
            case Event.Reading { V1: ReadingEvent.Chapters chapters }:
                if (chapters.Work != View?.Work && chapters.Work != Asked?.Work)
                    return;
                Chapters = chapters.Volumes;
                ChaptersLoaded = true;
                Changed?.Invoke(ReaderChange.Chapters);
                break;
        }
    }

    public void Apply(ReaderEvent e)
    {
        switch (e)
        {
            case ReaderEvent.Opened opened:
                var view = opened.V1;
                if (dropped > 0 || Asked is null)
                {
                    // An open the person left before it came: the engine closes it too. The one
                    // this reader is closing already heard.
                    if (view.Session != session)
                        Ask(new ReaderCommand.Close(view.Session));
                    return;
                }
                var flowedFrom = Flowing ? View?.Chapter.Id : null;
                session = view.Session;
                View = view;
                Phase = ReaderPhase.Open;
                Failure = null;
                Leaves = [];
                moved = false;
                // A strip flowing on keeps the files of the chapter it flows from: those images
                // stay in its scroll, and the engine doesn't send them again.
                foreach (var gone in Pages.Keys.Where(k => k.Chapter != flowedFrom).ToList())
                    Pages.Remove(gone);
                PageFailures.Clear();
                Novel = null;
                NovelPainted = false;
                End = null;
                Read = false;
                ReadTrackers = [];
                ServerProgress = null;
                ServerProgressServer = null;
                ServerSending = null;
                NextPreview = null;
                Opening = null;
                Ring = null;
                BackTo = null;
                Notice = null;
                Current = 0;
                if (view.Settings is { } settings)
                    Settings = settings;
                Ask(new ReaderCommand.LoadSettings(view.Medium, view.Work));
                StripRestore = view.Mode == ReaderMode.Strip && !Flowing ? view.Start.Strip : null;
                if (view.Mode != ReaderMode.Strip)
                    Flowing = false;
                if (surface is { } now && sentSurface != now)
                    sentSurface = now;
                Changed?.Invoke(ReaderChange.Phase);
                Changed?.Invoke(ReaderChange.Settings);
                break;
            case ReaderEvent.Leaves leaves:
                if (leaves.Session != session)
                    return;
                var before = Leaves;
                Leaves = leaves.LeavesValue;
                Phase = ReaderPhase.Open;
                if (Leaves.Length > 0)
                    Current = Placed(before, Current);
                Changed?.Invoke(ReaderChange.Leaves);
                if (pendingGoTo is { } page && LeafOfPageIndex((uint)page) is { } at)
                    Show(at);
                break;
            case ReaderEvent.PageReady ready:
                if (ready.Session != session)
                    return;
                var key = new ReaderPageKey(ready.Chapter, ready.Page, ready.Half);
                Pages[key] = ready.File;
                PageFailures.Remove(key);
                if (ready.Half is null)
                    PageFailures.Remove(key with { Half = null });
                Changed?.Invoke(ReaderChange.Page);
                break;
            case ReaderEvent.PageFailed failed:
                if (failed.Session != session)
                    return;
                if (Leaves.Length == 0 && failed.Page == 0)
                {
                    Flowing = false;
                    ClearNotices();
                    Phase = ReaderPhase.Failed;
                    Failure = failed.Failure;
                    Changed?.Invoke(ReaderChange.Phase);
                }
                PageFailures[new ReaderPageKey(failed.Chapter, failed.Page)] = failed.Failure;
                Changed?.Invoke(ReaderChange.Page);
                break;
            case ReaderEvent.NovelOpen novelOpen:
                if (novelOpen.V1.Session != session)
                    return;
                Novel = novelOpen.V1;
                NovelStyle = novelOpen.V1.Style;
                NovelCountKey = novelOpen.V1.CountKey;
                NovelCounted = novelOpen.V1.Counted;
                Phase = ReaderPhase.Open;
                if (novelOpen.V1.Restore is { } restore)
                    NovelGoTo = (restore, Bump());
                Changed?.Invoke(ReaderChange.Novel);
                break;
            case ReaderEvent.NovelStyle style:
                if (style.Session != session)
                    return;
                NovelStyle = style.Style;
                NovelCountKey = style.CountKey;
                NovelCounted = style.Counted;
                Changed?.Invoke(ReaderChange.NovelStyle);
                break;
            case ReaderEvent.ChapterRead read:
                if (read.Chapter == View?.Chapter.Id)
                {
                    Read = true;
                    ReadTrackers = read.Trackers;
                    // What the engine already knows (offline, held) came before this; otherwise the
                    // server is asked now and its answer follows.
                    // The engine names the server that receives the work's progress (progress_host).
                    if (ServerProgress is null && ServerSending is null && View?.ProgressHost is { Length: > 0 } host)
                        ServerSending = (host, DateTimeOffset.UtcNow);
                    Changed?.Invoke(ReaderChange.Read);
                }
                break;
            case ReaderEvent.ProgressSent sent:
                if (sent.Chapter == View?.Chapter.Id)
                {
                    ServerProgress = (sent.Host, sent.Result);
                    ServerProgressServer = sent.Server;
                    ServerSending = null;
                    Changed?.Invoke(ReaderChange.Sent);
                }
                break;
            case ReaderEvent.EndPage end:
                if (end.Session != session)
                    return;
                End = new ReaderEnd(end.Next, end.WhyNone, end.NextSource);
                Changed?.Invoke(ReaderChange.End);
                break;
            case ReaderEvent.NextPreview preview:
                if (preview.Session != session)
                    return;
                NextPreview = preview.File;
                Changed?.Invoke(ReaderChange.Preview);
                break;
            case ReaderEvent.Opening opening:
                // An open the person left says nothing over the next one.
                if (dropped > 0 || Asked is null)
                    return;
                Opening = (opening.Chapter, opening.Wait);
                Changed?.Invoke(ReaderChange.Opening);
                break;
            case ReaderEvent.Notice notice:
                Show(notice.V1);
                break;
            case ReaderEvent.BackToPage offer:
                if (offer.Session != session || BackTo == offer.Page)
                    return;
                BackTo = offer.Page;
                Changed?.Invoke(ReaderChange.BackTo);
                break;
            case ReaderEvent.Moved:
                // The notice that comes with it says where, and offers the way back.
                break;
            case ReaderEvent.Settings told:
                // Settings' Reading pane asks for each medium's defaults: an open book keeps its own.
                if (View is { } shown && (told.V1.Medium != shown.Medium || (told.V1.Work is not null && told.V1.Work != shown.Work)))
                    return;
                if (View is null && Asked is null)
                    return;
                Settings = told.V1;
                Changed?.Invoke(ReaderChange.Settings);
                break;
            case ReaderEvent.GoTo go:
                if (go.Session != session)
                    return;
                GoToPlace(go.Place);
                break;
            case ReaderEvent.Closed closed:
                if (closed.Session == 0)
                {
                    // An open dropped before it had a session is gone: what follows is the next one's.
                    if (dropped > 0)
                        dropped--;
                    return;
                }
                if (closed.Session != session)
                    return;
                session = 0;
                opened = false;
                if (afterClose is { } next)
                {
                    afterClose = null;
                    Begin(next);
                }
                break;
        }
    }

    int StartLeaf() => View?.Start.Page is { } page && LeafOfPageIndex(page) is { } at ? at : 0;

    /// <summary>
    /// The leaf to show once the leaves came: the chapter's start until the person moves (the
    /// leaves may still be coming in as an archive extracts), and after that the page that was
    /// on screen, wherever the new layout (full screen, a resize, a setting) puts it.
    /// </summary>
    int Placed(Leaf[] before, int was)
    {
        if (!moved || was < 0 || was >= before.Length)
            return Math.Clamp(StartLeaf(), 0, Leaves.Length - 1);
        var leaf = before[was];
        if (leaf.EndPage)
            return Array.FindIndex(Leaves, l => l.EndPage) is var end and >= 0 ? end : Leaves.Length - 1;
        if (leaf.Parts.Length > 0 && LeafOfPageIndex(leaf.Parts.Min(p => p.Page)) is { } at)
            return at;
        return Math.Clamp(was, 0, Leaves.Length - 1);
    }

    /// <summary>The leaf that holds a page, counted from 0.</summary>
    int? LeafOfPageIndex(uint page)
    {
        for (var i = 0; i < Leaves.Length; i++)
            if (Leaves[i].Parts.Any(p => p.Page >= page))
                return i;
        return null;
    }

    void GoToPlace(ReadPlace place)
    {
        if (place.Novel is { } pos)
        {
            NovelGoTo = (pos, Bump());
            Changed?.Invoke(ReaderChange.NovelGoTo);
            return;
        }
        if (place.Strip is { } strip)
        {
            StripRestore = strip;
            GoTo = ((int)strip.Image, Bump());
            Changed?.Invoke(ReaderChange.GoTo);
            return;
        }
        if (place.Page is not { } page)
            return;
        if (LeafOfPageIndex(page) is { } at)
            Show(at);
        else
            pendingGoTo = (int)page;
    }

    void Show(int leaf)
    {
        pendingGoTo = null;
        Current = leaf;
        moved = true;
        GoTo = (leaf, Bump());
        Changed?.Invoke(ReaderChange.GoTo);
    }

    /// <summary>Whether a screen reader runs now, for how long a notice holds. The window sets it.</summary>
    public Func<bool>? ScreenReader { get; set; }

    bool noticeHover, inactive;
    TimeSpan? noticeLeft;
    DateTimeOffset noticeRunFrom;

    /// <summary>The pointer or keyboard focus is on the notice (true) or left it: its time waits meanwhile.</summary>
    public void HoldNotice(bool held)
    {
        noticeHover = held;
        NoticeClock();
    }

    /// <summary>The window came to the front or went behind another: a notice's time runs only in front.</summary>
    public void WindowActive(bool active)
    {
        inactive = !active;
        NoticeClock();
    }

    /// <summary>
    /// How long a notice holds, in milliseconds, or null until it is dismissed: the engine's own
    /// rule (<c>reader_notice_hold</c>). A second more for every 10 words past 12, and a notice
    /// with a button waits for the person while a screen reader runs.
    /// </summary>
    public static uint? NoticeHold(uint lastsMs, uint words, bool hasAction, bool screenReader) =>
        KaeriModelMethods.ReaderNoticeHold(lastsMs, words, hasAction, screenReader);

    static uint Words(string text) => (uint)text.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries).Length;

    void Show(ReaderNotice incoming)
    {
        var line = new ReaderNoticeLine(++noticeCount, incoming.Msg.Text(), incoming.Action);
        noticeTimer?.Cancel();
        noticeTimer = null;
        Notice = line;
        var hasAction = incoming.Action is ReaderAction.StartOver or ReaderAction.GoBack or ReaderAction.ChangeMode;
        var hold = NoticeHold(incoming.LastsMs, Words(line.Text), hasAction, ScreenReader?.Invoke() ?? false);
        // 0 is e-ink's: the notice stays until the next turn clears it.
        noticeLeft = hold is { } ms && ms > 0 ? TimeSpan.FromMilliseconds(ms) : null;
        Changed?.Invoke(ReaderChange.Notice);
        NoticeClock();
    }

    /// <summary>The notice's time runs, or waits while something holds it.</summary>
    void NoticeClock()
    {
        if (Notice is not { } line || noticeLeft is not { } left)
            return;
        if (noticeHover || inactive)
        {
            if (noticeTimer is null)
                return;
            noticeTimer.Cancel();
            noticeTimer = null;
            noticeLeft = left - (DateTimeOffset.UtcNow - noticeRunFrom);
            return;
        }
        if (noticeTimer is null)
            RunNotice(line.Id, left);
    }

    async void RunNotice(int id, TimeSpan left)
    {
        var timer = noticeTimer = new CancellationTokenSource();
        noticeRunFrom = DateTimeOffset.UtcNow;
        try
        {
            await Task.Delay(left > TimeSpan.Zero ? left : TimeSpan.Zero, timer.Token);
        }
        catch (TaskCanceledException)
        {
            return;
        }
        if (noticeTimer != timer || Notice?.Id != id)
            return;
        noticeTimer = null;
        noticeLeft = null;
        Notice = null;
        Changed?.Invoke(ReaderChange.Notice);
    }

    /// <summary>Another chapter opens, or the open failed: neither the notice nor Back to stays over it.</summary>
    void ClearNotices()
    {
        noticeTimer?.Cancel();
        noticeTimer = null;
        noticeLeft = null;
        var had = Notice is not null;
        var hadBack = BackTo is not null;
        Notice = null;
        BackTo = null;
        if (had)
            Changed?.Invoke(ReaderChange.Notice);
        if (hadBack)
            Changed?.Invoke(ReaderChange.BackTo);
    }

    void Reset()
    {
        noticeTimer?.Cancel();
        noticeTimer = null;
        noticeLeft = null;
        noticeHover = false;
        pendingStrip?.Cancel();
        View = null;
        Leaves = [];
        Pages.Clear();
        PageFailures.Clear();
        Novel = null;
        NovelPainted = false;
        NovelStyle = "";
        End = null;
        Notice = null;
        BackTo = null;
        Current = 0;
        Ring = null;
        StripRestore = null;
        Flowing = false;
        GoTo = null;
        NovelGoTo = null;
        Read = false;
        ReadTrackers = [];
        ServerProgress = null;
        ServerProgressServer = null;
        ServerSending = null;
        NextPreview = null;
        Opening = null;
        Chapters = [];
        ChaptersLoaded = false;
        Failure = null;
        opened = false;
        moved = false;
        pendingGoTo = null;
        lastStrip = DateTimeOffset.MinValue;
        sentSurface = null;
        Phase = ReaderPhase.Closed;
    }

    // Derived

    /// <summary>The pages of the chapter, as the leaves count them.</summary>
    public int PageCount => Leaves.Length == 0 ? 0 : (int)Leaves.Max(l => l.Parts.Length == 0 ? 0 : l.Parts.Max(p => p.Page)) + 1;

    /// <summary>The page number, from 1, a leaf starts at.</summary>
    public int PageNumber(int leaf) =>
        leaf >= 0 && leaf < Leaves.Length && Leaves[leaf].Parts.Length > 0 ? (int)Leaves[leaf].Parts.Min(p => p.Page) + 1 : leaf + 1;

    /// <summary>The leaf a page, from 1, is on.</summary>
    public int LeafOfPage(int number) => LeafOfPageIndex((uint)Math.Max(number - 1, 0)) ?? Math.Max(Leaves.Length - 1, 0);

    /// <summary>The file a part shows, once it has landed.</summary>
    public PageFile? FileOf(LeafPart part, string chapter) =>
        part.File ?? (Pages.TryGetValue(new ReaderPageKey(chapter, part.Page, part.Half), out var half) ? half
            : Pages.TryGetValue(new ReaderPageKey(chapter, part.Page), out var whole) ? whole : null);

    public ReadingFailure? FailureOf(LeafPart part, string chapter) =>
        PageFailures.TryGetValue(new ReaderPageKey(chapter, part.Page), out var failure) ? failure : null;

    /// <summary>The leaves the person sees as pages: the end page's leaf is not one.</summary>
    public int ReadingLeaves => Leaves.Count(l => !l.EndPage);
}
