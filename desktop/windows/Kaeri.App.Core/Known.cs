using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// What several pages read, kept as the engine last said it: every title seen, the lists, the
/// settings, the trackers, the downloads. <see cref="Take"/> folds one event in; the pages then
/// read from here rather than each keeping its own copy. Only the UI thread touches it.
/// </summary>
public sealed class Known
{
    /// <summary>Every title seen in a list or on its page, by AniList id, as it last stood.</summary>
    public Dictionary<uint, Entry> Entries { get; } = [];

    /// <summary>Each list's titles as the engine last sent them, by the list.</summary>
    public Dictionary<ContentFor, ContentList> Lists { get; } = [];

    /// <summary>A title's episodes, by AniList id.</summary>
    public Dictionary<uint, EpisodeRow[]> Episodes { get; } = [];

    /// <summary>
    /// Whether a title has a dub, by AniList id, once the engine has said. A title its source
    /// could not say for counts as without one.
    /// </summary>
    public Dictionary<uint, bool> Dubbed { get; } = [];

    /// <summary>
    /// The comments in an episode's discussion, by title and episode number, once the engine has
    /// found one. An episode with none is absent.
    /// </summary>
    public Dictionary<(uint Id, uint Episode), uint> Discussions { get; } = [];

    public SettingsView? Settings { get; private set; }

    /// <summary>Each show's own settings sheet, as the engine last sent it, by AniList id.</summary>
    public Dictionary<uint, ShowSettingsView> ShowSheets { get; } = [];

    /// <summary>Testing builds: a reason finding skip times is paused to say, whatever the engine does.</summary>
    public SkipFindPause? ForcedPause { get; set; }

    /// <summary>Why finding skip times from the audio is paused or can't run, as the engine last said.</summary>
    public SkipFindPause? SkipFindPaused => ForcedPause ?? Settings?.SkipFindPaused;

    /// <summary>Play the demo was pressed and the player has not opened yet: the starting screen is the demo's.</summary>
    public bool DemoPlay { get; set; }

    /// <summary>The last alert the engine raised, which a bug report can start from.</summary>
    public BugError? LastError { get; set; }

    /// <summary>The subtitle style as it stood when Reset was pressed, for the notice's Undo to put back.</summary>
    public SubtitleStyle? StyleBeforeReset { get; set; }

    /// <summary>Where the built-in demo stands, as the engine last said: on, offered, alone, played.</summary>
    public DemoState Demo { get; private set; } = new();

    /// <summary>Whether trailers may start by themselves now, as the engine last said; null before it has.</summary>
    public AutoplayView? Autoplay { get; private set; }

    /// <summary>What the search filters choose from, once the engine has said.</summary>
    public SearchFacets? Facets { get; private set; }
    /// <summary>Search's Trending rail by period, as the engine last sent it; a failed period is not here.</summary>
    public Dictionary<TrendPeriod, Entry[]> TrendingKept { get; } = [];
    /// <summary>The period the engine says was last picked on this device, when it is not today.</summary>
    public TrendPeriod? TrendingRemembered { get; private set; }
    /// <summary>The periods whose last ask failed.</summary>
    public HashSet<TrendPeriod> TrendingFailed { get; } = [];
    public Dictionary<string, SyncState> Sync { get; } = [];

    /// <summary>
    /// Sign-ins started here and not over yet: the tracker, with the engine's job for the step
    /// under way, or none while the person is in the browser. Signed in, or the job ending
    /// without it, takes the tracker out.
    /// </summary>
    public Dictionary<string, ulong?> SigningIn { get; } = [];

    /// <summary>When each signed-in tracker was last seen synced, with nothing waiting and nothing failed.</summary>
    public Dictionary<string, DateTimeOffset> SyncedAt { get; } = [];

    /// <summary>A sign-in to <paramref name="tracker"/> was asked for, as <paramref name="job"/>.</summary>
    public void SignInStarted(string tracker, ulong? job) => SigningIn[tracker] = job;

    /// <summary>Sync between the person's own devices, as the engine last said; null before it has.</summary>
    public SyncStatus? SyncGroup { get; set; }

    /// <summary>The recovery key the engine just said, until its dialog closes.</summary>
    public string? RecoveryKey { get; set; }

    /// <summary>The pairing code this PC shows, its port, and when it runs out in Unix seconds.</summary>
    public (string Code, ushort Port, long Until)? PairInvite { get; set; }

    /// <summary>The problem sync last raised that a dialog may show, until it is dealt with.</summary>
    public SyncProblem? SyncTrouble { get; set; }

    /// <summary>The profiles on this PC, empty for one person, who never sees a picker or a name.</summary>
    public ProfileView[] Profiles { get; set; } = [];

    /// <summary>A cloud sign-in under way, and its step.</summary>
    public (SyncFolderKind Service, SyncSignInStep Step)? SyncSigningIn { get; set; }
    public DownloadRow[] Downloads => Queue.Rows;
    public SeedRow[] Seeding { get; private set; } = [];

    /// <summary>The download queue, by title, with each running download's speed.</summary>
    public DownloadQueue Queue { get; } = new();

    /// <summary>The source picker's listings per title, and which pins have been said to be unavailable.</summary>
    public SourceChoices SourceChoices { get; } = new();

    /// <summary>The match picker's state per title: the questions sources asked, searches by hand and what each title is matched to.</summary>
    public MatchBook Matches { get; } = new();

    /// <summary>The sources, as the engine last said them, in the order it lists them.</summary>
    public ProviderRow[] Providers { get; private set; } = [];
    public bool ProvidersLoaded { get; private set; }

    /// <summary>Why no sources are available, when none are.</summary>
    public string? ProviderNote { get; private set; }

    /// <summary>The sources are being rebuilt, from <c>SourcesChanged</c> until the rebuilt list.</summary>
    public bool Rebuilding { get; private set; }

    /// <summary>The providers in the order failover tries them, as the config now has it.</summary>
    public ProviderRow[] OrderedProviders => [.. SourceOrder.Sorted(Providers, Settings?.Sources.Order ?? []).OrderBy(r => r.IsDemo)];

    /// <summary>Every joined plugin store.</summary>
    /// <summary>The Licensed services page: the services chosen, the region, and the services AniList lists.</summary>
    public LicensedView? Licensed { get; private set; }

    /// <summary>What each title's page says about licensed services, as the engine last worked it out.</summary>
    public Dictionary<uint, TitleAvailability> Availability { get; } = [];

    /// <summary>The Jellyfin servers saved, with their libraries and how far matching has got.</summary>
    public ServerRow[] Servers { get; private set; } = [];

    /// <summary>Whether the engine has listed the saved servers yet, even as none.</summary>
    public bool ServersLoaded { get; private set; }

    /// <summary>The folders on this computer and the groups of files found in them.</summary>
    public LocalView? Local { get; private set; }

    /// <summary>The titles Fix this match last offered, with the group and search they answer.</summary>
    public LocalEvent.Matches? LocalMatches { get; private set; }

    readonly Dictionary<string, DateTimeOffset> waits = [];

    /// <summary>
    /// Seconds left of AniList's limit holding matching back, for a server or folder, or null. The
    /// engine says it once as the wait starts and the app counts it down.
    /// </summary>
    public int? WaitingSeconds(string key, DateTimeOffset now) =>
        waits.TryGetValue(key, out var until) && until > now ? (int)Math.Ceiling((until - now).TotalSeconds) : null;

    /// <summary>Whether any matching is waiting on AniList's limit, so a page keeps counting.</summary>
    public bool AnyWaiting(DateTimeOffset now) => waits.Values.Any(until => until > now);

    void Wait(string key, MatchProgress? progress)
    {
        if (progress?.WaitSeconds is { } seconds)
        {
            if (!waits.TryGetValue(key, out var until) || until <= DateTimeOffset.UtcNow)
                waits[key] = DateTimeOffset.UtcNow.AddSeconds(seconds);
        }
        else
            waits.Remove(key);
    }

    public StoreRow[] Stores { get; private set; } = [];
    public bool StoresLoaded { get; private set; }

    /// <summary>What a pasted invite link leads to, once checked.</summary>
    public InvitePreview? Preview { get; set; }

    /// <summary>The store just joined, until the page has shown it.</summary>
    public string? Joined { get; set; }

    /// <summary>An invite link made, to show and send.</summary>
    public InviteLinkView? Invite { get; set; }

    /// <summary>A store's members and unused invites, for its owner.</summary>
    public Dictionary<string, (StoreMemberRow[] Members, StoreInviteRow[] Invites, StoreInviteRow[] Ended, bool InviteChoices)> Members { get; } = [];

    /// <summary>The search asked last, which a change of title language asks again.</summary>
    ContentFor.Search? lastSearch;

    /// <summary>The calendar's broadcasts count their <c>AiringIn</c> from this Unix time.</summary>
    public long? CalendarAnchor { get; private set; }

    /// <summary>
    /// "Up next for you", as the engine last sent it: the engine decides which titles, in which
    /// order and how many, and sends a fresh group whenever the queue changes.
    /// </summary>
    public UpNextItem[] UpNext { get; private set; } = [];

    /// <summary>Home as the engine last sent it, whole, or null before the first send.</summary>
    public HomeView? Home { get; private set; }

    /// <summary>The titles hidden with Not interested, newest first, once asked for.</summary>
    public HiddenTitle[] Hidden { get; private set; } = [];

    /// <summary>Whether the engine has sent the hidden titles yet, even as none.</summary>
    public bool HiddenLoaded { get; private set; }

    /// <summary>Titles restored from Settings in this run, until the engine sends the list again.</summary>
    public HashSet<uint> RestoredHere { get; } = [];

    /// <summary>
    /// Titles dismissed from Home in this run, until a Home from the engine without them: so a
    /// send that crossed the dismissal cannot bring one back. Home is drawn without them.
    /// </summary>
    public HashSet<uint> DismissedHere { get; } = [];

    /// <summary>
    /// A title put on a list here, ahead of the tracker's word: the engine queues the change for
    /// the tracker and answers with the outbox's depth only, so the title's status and the
    /// Library's lists take it now, as the Mac's title cache does. The title moves to the front
    /// of its new list, where the tracker's last-updated order puts it.
    /// </summary>
    public void SetStatus(uint id, LibrarySegment segment)
    {
        var entry = Entries.GetValueOrDefault(id);
        if (entry is not null)
            Entries[id] = entry = entry with { ListStatus = segment };
        foreach (var (view, list) in Lists.ToList())
        {
            if (view is not ContentFor.Library { V1: var s } || list is not (ContentList.Entries or ContentList.Empty))
                continue;
            var rows = list is ContentList.Entries some ? some.V1 : [];
            var without = rows.Where(e => e.Id != id).ToArray();
            if (s == segment && entry is not null)
                Lists[view] = new ContentList.Entries([entry, .. without]);
            else if (without.Length != rows.Length)
                Lists[view] = without.Length == 0 ? new ContentList.Empty() : new ContentList.Entries(without);
        }
    }

    /// <summary>A suggestion was put on Planning here: its button is gone until the engine says otherwise.</summary>
    public void PlannedFromUpNext(uint id) =>
        UpNext = [.. UpNext.Select(i => i.Entry.Id == id ? i with { Action = UpNextAction.Nothing } : i)];

    /// <summary>Whether the engine has sent <c>UpNext</c> since the page asked, even an empty group.</summary>
    public bool UpNextSent { get; set; }

    /// <summary>
    /// Titles hidden from Continue in this run, until the engine's next list or a Home whose
    /// Continue no longer holds them. Home's Continue and spotlight are drawn without them.
    /// </summary>
    public HashSet<uint> HiddenFromContinue { get; } = [];

    /// <summary>The engine has no network path: from <c>Connectivity</c>, fed by the window's network watch.</summary>
    public bool Offline { get; private set; }

    /// <summary>
    /// The torrent proxy check's last status and when it came, or null while torrents are off or
    /// before the first word.
    /// </summary>
    public (ProxyStatus Status, DateTimeOffset At)? ProxyCheck { get; private set; }

    /// <summary>What the update check last said, or null before it has said anything.</summary>
    public UpdateStatus? Update { get; private set; }

    /// <summary>The watch party this device is in, or null when it is in none.</summary>
    public PartyView? Party { get; private set; }

    /// <summary>The party's members, this device's first, once the room has listed them.</summary>
    public PartyMember[]? PartyMembers { get; private set; }

    /// <summary>What the form that starts a party opens with, once the engine has said.</summary>
    public PartyForm? PartyForm { get; set; }

    /// <summary>The last ten rooms, kept on this device, newest first.</summary>
    public PartyRecentRoom[] PartyRecent { get; private set; } = [];

    /// <summary>Whether the engine has sent the last rooms yet, even as none.</summary>
    public bool PartyRecentLoaded { get; private set; }

    /// <summary>What looking into a room found, or why it could not, for the room that was opened last.</summary>
    public PartyGlimpse? PartyGlimpse { get; private set; }

    public string? PartyLookFailure { get; private set; }

    /// <summary>
    /// A room opened from an invite, a code or Rejoin, looked into and not joined yet: the lobby
    /// shows what it watches and who is there, with Join. Null once joined or let go.
    /// </summary>
    public (string Server, string Room)? PartyLooking { get; set; }

    /// <summary>The room finished its last episode out (and when the next airs): Ready would replay it. Gone when the room moves on.</summary>
    public Event.PartyLastOut? PartyLastOut { get; private set; }

    /// <summary>The party's conversation as the engine last sent it, whole: lines, reaction marks and a lost send.</summary>
    public PartyConversation? PartyChat { get; private set; }

    /// <summary>What this device may do in the conversation now, once the room's server has said.</summary>
    public PartyChatState? PartyChatState { get; private set; }

    /// <summary>The AniList account, when signed in.</summary>
    public SyncState? Anilist => Sync.TryGetValue("anilist", out var s) && s.Connected ? s : null;

    /// <summary>Episodes downloading or waiting to, and those seeding: what closing the window stops.</summary>
    public (int Downloading, int Seeding) Running() =>
        (Downloads.Count(d => d.State is DownloadState.Active or DownloadState.Queued), Seeding.Length);

    /// <summary>The list's titles, each as it now stands, or none when it has not loaded.</summary>
    public Entry[] ListEntries(ContentFor view) =>
        List(view) is ContentList.Entries e
            ? [.. e.V1.Select(x => Entries.GetValueOrDefault(x.Id, x))]
            : [];

    /// <summary>
    /// The list's rows as the engine sent them. The calendar's rows are broadcasts, one title
    /// several times over, each with its own episode and air time, which the titles as they now
    /// stand would lose.
    /// </summary>
    public Entry[] ListRows(ContentFor view) =>
        List(view) is ContentList.Entries e ? e.V1 : [];

    /// <summary>
    /// The list as the engine last sent it, or null. A search is found by what it asks: the query
    /// the engine sends back carries arrays of its own, which a record compares by reference.
    /// </summary>
    public ContentList? List(ContentFor view) =>
        Lists.TryGetValue(view, out var list) ? list
        : view is ContentFor.Search ? Lists.FirstOrDefault(p => Searches.Same(p.Key, view)).Value
        : null;

    public Entry? Entry(uint id) => Entries.GetValueOrDefault(id);

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.Content c:
                // One key per search, whichever instance of its query asked it.
                var key = c.View is ContentFor.Search && !Lists.ContainsKey(c.View)
                    ? Lists.Keys.FirstOrDefault(k => Searches.Same(k, c.View)) ?? c.View
                    : c.View;
                Lists[key] = c.ContentValue;
                if (key is ContentFor.Search search)
                    lastSearch = search;
                if (c.ContentValue is ContentList.Entries list)
                {
                    // Continue's list carries this device's watch history. Another view's rows may
                    // carry a tracker's progress, or none, so they refresh a known title's details
                    // and leave what was watched here alone.
                    var local = c.View is ContentFor.Continue;
                    foreach (var entry in list.V1)
                        Entries[entry.Id] = Merge(Fixed(entry), local);
                    if (c.View is ContentFor.Continue)
                        HiddenFromContinue.Clear();
                }
                break;
            case Event.Demo demo:
                Demo = demo.V1;
                break;
            case Event.UpNext up:
                UpNext = up.V1;
                UpNextSent = true;
                foreach (var item in up.V1)
                    Entries[item.Entry.Id] = Merge(Fixed(item.Entry), local: false);
                break;
            case Event.Home home:
                Home = home.V1;
                // The engine has the removal once a Home comes without the title; until then a
                // send that crossed it still holds the title, and Home leaves it out.
                var held = home.V1.Sections.SelectMany(s => s.Items).Select(i => i.Entry.Id).ToHashSet();
                if (home.V1.Spotlight is { } shown)
                    held.Add(shown.Entry.Id);
                DismissedHere.RemoveWhere(id => !held.Contains(id));
                var continuing = home.V1.Sections.Where(s => s.Kind == SectionKind.Continue).SelectMany(s => s.Items).Select(i => i.Entry.Id).ToHashSet();
                if (home.V1.Spotlight is { Case: SpotlightCase.Continuing } going)
                    continuing.Add(going.Entry.Id);
                HiddenFromContinue.RemoveWhere(id => !continuing.Contains(id));
                if (home.V1.Spotlight is { } spot)
                    Entries[spot.Entry.Id] = Merge(Fixed(spot.Entry), local: false);
                foreach (var section in home.V1.Sections)
                    foreach (var item in section.Items)
                        Entries[item.Entry.Id] = Merge(Fixed(item.Entry), local: section.Kind == SectionKind.Continue);
                break;
            case Event.HiddenTitles hidden:
                Hidden = hidden.V1;
                HiddenLoaded = true;
                RestoredHere.Clear();
                break;
            case Event.Detail d:
                Entries[d.V1.Id] = Fixed(d.V1);
                break;
            case Event.Cards c:
                // Card data for covers the lists did not bring: a title already known keeps its row.
                foreach (var card in c.V1)
                    Entries.TryAdd(card.Id, Fixed(card));
                break;
            case Event.Episodes ep:
                Episodes[ep.Id] = ep.Rows;
                break;
            case Event.SourceOptions:
                SourceChoices.Take(e);
                break;
            case Event.MatchChoices or Event.MatchSearchFailed or Event.MatchFixFailed or Event.TitleMatch:
                Matches.Take(e);
                break;
            case Event.DiscussionFound found:
                Discussions[(found.Media, found.Episode)] = found.Comments;
                break;
            case Event.TitleDub td:
                Dubbed[td.Id] = td.Dub is not null;
                break;
            case Event.Progress p:
                if (Entries.TryGetValue(p.Id, out var known))
                    Entries[p.Id] = known with { Progress = p.ProgressValue, Resume = p.Resume };
                // The episodes' watched state comes with it, as the Mac's title cache takes it:
                // the table is patched, with no new listing.
                if (p.EpisodesValue.Length > 0 && Episodes.TryGetValue(p.Id, out var table))
                {
                    var marks = p.EpisodesValue.GroupBy(e => e.Number).ToDictionary(g => g.Key, g => g.Last());
                    Episodes[p.Id] = [.. table.Select(row => marks.TryGetValue(row.Number, out var mark)
                        ? row with { Watched = mark.Watched, Completed = mark.Completed, SpoilerAhead = mark.SpoilerAhead }
                        : row)];
                }
                break;
            case Event.LastAired aired:
                foreach (var title in aired.V1)
                    if (Entries.TryGetValue(title.Id, out var was))
                        Entries[title.Id] = was with { LastAired = title.Aired };
                break;
            case Event.CalendarWindow w:
                CalendarAnchor = w.Anchor;
                break;
            case Event.Settings s:
                Settings = s.V1;
                break;
            case Event.ShowSettings sheet:
                ShowSheets[sheet.V1.Id] = sheet.V1;
                break;
            case Event.Autoplay a:
                Autoplay = a.V1;
                break;
            case Event.Facets f:
                Facets = f.V1;
                break;
            case Event.Browse browse:
                // Today is the first rail of the browse.
                if (browse.V1.FirstOrDefault(r => r.Query.Sort == SearchSort.Trending) is { } rail && rail.Entries.Length > 0)
                    KeepTrending(TrendPeriod.Today, rail.Entries);
                break;
            case Event.Trending trending:
                if (trending.Failed)
                    TrendingFailed.Add(trending.Period);
                else
                {
                    TrendingFailed.Remove(trending.Period);
                    KeepTrending(trending.Period, trending.Entries);
                    if (trending.Period != TrendPeriod.Today)
                        TrendingRemembered = trending.Period;
                }
                break;
            case Event.Sync s:
                Sync[s.V1.Tracker] = s.V1;
                if (s.V1 is { Connected: true, NeedsReauth: false })
                    SigningIn.Remove(s.V1.Tracker);
                // Synced as the Mac counts it: an outcome, nothing waiting and no failed pull.
                if (!s.V1.Connected)
                    SyncedAt.Remove(s.V1.Tracker);
                else if (s.V1.Last is not null && s.V1.Outbox == 0 && s.V1.PullError is null)
                    SyncedAt[s.V1.Tracker] = DateTimeOffset.UtcNow;
                break;
            case Event.Authorize authorize when SigningIn.ContainsKey(authorize.Tracker):
                // In the browser now: the job that opened it ends, the sign-in goes on.
                SigningIn[authorize.Tracker] = null;
                break;
            case Event.DeviceCode { V1: null }:
                // A code's sign-in is over, however it ended.
                SigningIn.Remove("simkl");
                SigningIn.Remove("trakt");
                break;
            case Event.JobDone done:
                foreach (var tracker in SigningIn.Where(p => p.Value == done.V1).Select(p => p.Key).ToList())
                    SigningIn.Remove(tracker);
                break;
            case Event.SyncStatus group:
                // A pass that works clears the problem, except those only a dialog can answer.
                if (group.V1.Problem is null && SyncTrouble is not (null or SyncProblem.WrongRecoveryKey or SyncProblem.GroupExists or SyncProblem.NoGroup))
                    SyncTrouble = null;
                SyncGroup = group.V1;
                break;
            case Event.SyncInvite invite:
                PairInvite = (invite.Code, invite.Port, DateTimeOffset.UtcNow.ToUnixTimeSeconds() + invite.Seconds);
                break;
            case Event.SyncRecoveryKey recovery:
                RecoveryKey = recovery.V1;
                SyncSigningIn = null;
                break;
            case Event.SyncSignIn signIn:
                SyncSigningIn = signIn.Step is { } step ? (signIn.Service, step) : null;
                break;
            case Event.Profiles profiles:
                Profiles = profiles.V1.Profiles ?? [];
                break;
            case Event.SyncProblem problem:
                SyncTrouble = problem.V1;
                SyncSigningIn = null;
                break;
            case Event.Downloads d:
                Queue.Apply(d.V1, DateTimeOffset.UtcNow);
                break;
            case Event.Providers p:
                Providers = p.V1;
                ProvidersLoaded = true;
                Rebuilding = false;
                if (p.V1.Length > 0)
                    ProviderNote = null;
                break;
            case Event.ProviderNote n:
                ProviderNote = n.V1.Text();
                break;
            case Event.SourcesChanged:
                Rebuilding = true;
                break;
            case Event.Licensed l:
                Licensed = l.V1;
                break;
            case Event.Availability a:
                Availability[a.V1.Id] = a.V1;
                break;
            case Event.Server { V1: ServerEvent.Servers rows }:
                Servers = rows.V1;
                ServersLoaded = true;
                foreach (var row in Servers)
                    Wait("s:" + row.Id, row.Matching);
                break;
            case Event.Local { V1: LocalEvent.View localView }:
                Local = localView.V1;
                foreach (var folder in Local.Folders)
                    Wait("f:" + folder.Path, folder.Scan is FolderScan.Matching m ? m.Progress : null);
                break;
            case Event.Local { V1: LocalEvent.Matches found }:
                LocalMatches = found;
                break;
            case Event.Stores st:
                Stores = st.V1;
                StoresLoaded = true;
                break;
            case Event.StorePreview preview:
                Preview = preview.V1;
                break;
            case Event.StoreJoined joined:
                Joined = joined.Store;
                Preview = null;
                break;
            case Event.InviteLink link:
                Invite = link.V1;
                break;
            case Event.StoreMembers members:
                Members[members.Store] = (members.Members, members.Invites, members.Ended ?? [], members.InviteChoices);
                break;
            case Event.Seeding s:
                Seeding = s.V1;
                break;
            case Event.TorrentProxy p:
                ProxyCheck = p.V1 is { } status ? (status, DateTimeOffset.UtcNow) : null;
                break;
            case Event.Connectivity c:
                Offline = c.Offline;
                break;
            case Event.PartyForm f:
                PartyForm = f.V1;
                break;
            case Event.PartyJoined j:
                // A room joined afresh lists its members afresh.
                if (Party is null || Party.Server != j.V1.Server || Party.Room != j.V1.Room)
                {
                    PartyMembers = null;
                    PartyChat = null;
                    PartyChatState = null;
                }
                Party = j.V1;
                if (PartyLooking is { } looked && looked.Server == j.V1.Server && looked.Room == j.V1.Room)
                    PartyLooking = null;
                break;
            case Event.PartyState p:
                if (Party?.RoomEpisode is { } before && p.V1.RoomEpisode is { } after && (before.Episode != after.Episode || before.Id != after.Id))
                    PartyLastOut = null;
                Party = p.V1;
                break;
            case Event.PartyLastOut last:
                PartyLastOut = last;
                break;
            case Event.PartyStarted:
                PartyLastOut = null;
                break;
            case Event.PartyMembers m:
                PartyMembers = m.V1;
                break;
            case Event.PartyRecent r:
                PartyRecent = r.V1;
                PartyRecentLoaded = true;
                break;
            case Event.PartyGlimpse g:
                PartyGlimpse = g.V1;
                PartyLookFailure = null;
                break;
            case Event.PartyLookFailed f:
                PartyGlimpse = null;
                PartyLookFailure = f.Reason.Text();
                break;
            case Event.PartyLeft:
                Party = null;
                PartyLastOut = null;
                PartyMembers = null;
                PartyChat = null;
                PartyChatState = null;
                break;
            case Event.PartyChat c:
                PartyChat = c.V1;
                break;
            case Event.PartyChatState c:
                PartyChatState = c.V1;
                break;
            case Event.Update u:
                // A check that found nothing new leaves a release found before on offer only
                // while it is still the newest; the engine says UpToDate when it is not.
                Update = u.V1;
                break;
        }
    }

    /// <summary>The testing build's fixture runs give most titles a made-up trailer.</summary>
    public bool FixtureTrailers { get; set; }

    void KeepTrending(TrendPeriod period, Entry[] entries)
    {
        foreach (var entry in entries)
            Entries[entry.Id] = Merge(Fixed(entry), false);
        TrendingKept[period] = entries;
    }

    Entry Fixed(Entry entry) =>
#if KAERI_TESTING
        FixtureTrailers ? Trailers.WithFixtureTrailer(entry) :
#endif
        entry;

    /// <summary>
    /// A list's row for a title, keeping what the title's own page knew that a list row leaves
    /// out (its synopsis, relations and banner when the row has none). A row from any list but
    /// Continue (<paramref name="local"/> false) also keeps the progress, resume point and last
    /// aired episode already known, so a tracker's 12 of 12 can't turn "Resume E7" into E1.
    /// </summary>
    Entry Merge(Entry row, bool local)
    {
        if (!Entries.TryGetValue(row.Id, out var page))
            return row;
        if (!local)
            row = row with
            {
                Progress = page.Progress ?? row.Progress,
                Resume = page.Resume ?? row.Resume,
                LastAired = page.LastAired ?? row.LastAired,
            };
        return row with
        {
            Synopsis = row.Synopsis.Length > 0 ? row.Synopsis : page.Synopsis,
            BannerUrl = row.BannerUrl ?? page.BannerUrl,
            Trailer = row.Trailer ?? page.Trailer,
            TrailerUrl = row.TrailerUrl ?? page.TrailerUrl,
            Related = row.Related is { Length: > 0 } ? row.Related : page.Related,
            Recommended = row.Recommended is { Length: > 0 } ? row.Recommended : page.Recommended,
        };
    }

    /// <summary>
    /// What to ask for again once every title is named another way (<c>TitlesChanged</c>): each
    /// list that has been shown and the search asked last, so what is on screen names its titles
    /// the new way. A title page asks for its own title again.
    /// </summary>
    public IEnumerable<Command> AskAgain()
    {
        foreach (var view in Lists.Keys)
        {
            switch (view)
            {
                case ContentFor.Continue:
                    yield return new Command.LoadContinue();
                    break;
                case ContentFor.Seasonal:
                    yield return new Command.LoadSeasonal();
                    break;
                case ContentFor.Calendar:
                    yield return new Command.LoadCalendar();
                    break;
                case ContentFor.Library library:
                    yield return new Command.LoadLibrary(library.V1);
                    break;
            }
        }
        if (lastSearch is { } search)
            yield return new Command.Search(search.Query);
    }

    /// <summary>A setting's row as it stands.</summary>
    public SettingRow? Setting(SettingKey key) =>
        Settings is { } view ? view.Rows.Concat(view.NativeRows).FirstOrDefault(r => r.Key == key) : null;
}
