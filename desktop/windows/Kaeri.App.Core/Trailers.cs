using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>What the page's player has done so far, which decides what the banner and the web view do.</summary>
public enum TrailerPhase
{
    /// <summary>No web view.</summary>
    Idle,
    /// <summary>A web view is loading the page; the banner stays as it was.</summary>
    Loading,
    /// <summary>A frame is on screen and the video shows.</summary>
    Playing,
    /// <summary>The view is held, paused, with the banner back over it.</summary>
    Paused,
    /// <summary>It ended or was refused: not again on this visit.</summary>
    Done,
}

/// <summary>What the view does after the flow has moved.</summary>
[Flags]
public enum TrailerEffect
{
    None = 0,
    /// <summary>The video fades in over the banner.</summary>
    ShowVideo = 1,
    /// <summary>The video fades out and the banner is all there is.</summary>
    HideVideo = 2,
    /// <summary>Let go of the web view.</summary>
    Release = 4,
    PausePlayer = 8,
    PlayPlayer = 16,
}

/// <summary>
/// The life of one trailer in one place (a title page, a hover preview): the banner shows at once,
/// the video shows only once the page says a frame is playing, a trailer that has ended or was
/// refused does not come back on this visit. No WinUI in it, so the rules are tested.
/// </summary>
public sealed class TrailerFlow
{
    public TrailerPhase Phase { get; private set; }

    /// <summary>A web view is held: loading, playing or paused.</summary>
    public bool Active => Phase is TrailerPhase.Loading or TrailerPhase.Playing or TrailerPhase.Paused;

    /// <summary>The video is in front of the banner: a frame plays and nothing holds it back.</summary>
    public bool Showing => Phase == TrailerPhase.Playing && !Held;

    /// <summary>
    /// It loads ahead of being seen, as a hover card's cued trailer does: a frame playing shows
    /// nothing until <see cref="Unhold"/>.
    /// </summary>
    public bool Held { get; private set; }

    /// <summary>A trailer may start: nothing is held and it has not already ended or failed.</summary>
    public bool CanStart => Phase == TrailerPhase.Idle;

    public bool Start()
    {
        if (!CanStart)
            return false;
        Phase = TrailerPhase.Loading;
        return true;
    }

    /// <summary>Load it unseen: before it starts, or while it loads.</summary>
    public void Hold()
    {
        if (Phase is TrailerPhase.Idle or TrailerPhase.Loading)
            Held = true;
    }

    /// <summary>It is being seen now: the video shows at once when a frame already plays, or when one does.</summary>
    public TrailerEffect Unhold()
    {
        if (!Held)
            return TrailerEffect.None;
        Held = false;
        return Phase == TrailerPhase.Playing ? TrailerEffect.ShowVideo : TrailerEffect.None;
    }

    /// <summary>The page said something. Only a frame playing shows the video; an end or an error lets go for good.</summary>
    public TrailerEffect Heard(TrailerMessage? message)
    {
        switch (message)
        {
            case TrailerMessage.Playing when Phase is TrailerPhase.Loading or TrailerPhase.Paused:
                Phase = TrailerPhase.Playing;
                return Held ? TrailerEffect.None : TrailerEffect.ShowVideo;
            case TrailerMessage.Ended or TrailerMessage.Error when Active:
                return Let(TrailerPhase.Done);
            default:
                return TrailerEffect.None;
        }
    }

    /// <summary>The view could not be made, or the runtime is missing.</summary>
    public TrailerEffect Failed() => Active ? Let(TrailerPhase.Done) : TrailerEffect.None;

    /// <summary>It has not played in time: let go, and try again on a later visit.</summary>
    public TrailerEffect GiveUp() => Phase == TrailerPhase.Loading ? Let(TrailerPhase.Idle) : TrailerEffect.None;

    /// <summary>Hold the trailer: it pauses and the banner comes back over it.</summary>
    public TrailerEffect Pause()
    {
        if (Phase != TrailerPhase.Playing)
            return TrailerEffect.None;
        var shown = Showing;
        Phase = TrailerPhase.Paused;
        return TrailerEffect.PausePlayer | (shown ? TrailerEffect.HideVideo : TrailerEffect.None);
    }

    /// <summary>Play it again from where it paused; the video shows when the page says it is playing.</summary>
    public TrailerEffect Resume() => Phase == TrailerPhase.Paused ? TrailerEffect.PlayPlayer : TrailerEffect.None;

    /// <summary>Let go of the view, whatever it was doing.</summary>
    public TrailerEffect Stop() => Active ? Let(TrailerPhase.Idle) : TrailerEffect.None;

    TrailerEffect Let(TrailerPhase then)
    {
        var shown = Showing;
        Phase = then;
        Held = false;
        return TrailerEffect.Release | (shown ? TrailerEffect.HideVideo : TrailerEffect.None);
    }
}

/// <summary>A trailer page the shelf keeps between trailers, as it sees one.</summary>
public interface IWarmPage
{
    EmbedStyle Style { get; }
    /// <summary>The subtitle language its captions were set up for, as it loaded.</summary>
    string Subtitles { get; }
    /// <summary>Its page failed to load or its process ended: not kept.</summary>
    bool Broken { get; }
}

/// <summary>
/// The trailer pages kept for the run, as on the Mac: a page handed back is paused and kept, its
/// player loaded, and the next trailer only switches video in it rather than loading YouTube's
/// player again. Two of a style at most, as one can be on its way out while the next comes.
/// No WinUI in it, so the rules are tested.
/// </summary>
public sealed class TrailerShelf<T>(int keep = 2) where T : class, IWarmPage
{
    readonly List<T> idle = [];

    /// <summary>The pages waiting, for the tests and the log.</summary>
    public IReadOnlyList<T> Idle => idle;

    /// <summary>Whether the run's page with no video has been asked for.</summary>
    public bool Warmed { get; private set; }

    /// <summary>True the first time only: the page with no video loads once a run.</summary>
    public bool Warm()
    {
        if (Warmed)
            return false;
        Warmed = true;
        return true;
    }

    /// <summary>
    /// A kept page for <paramref name="style"/>, or none. Pages that can serve no trailer now (broken,
    /// or set up for another subtitle language) come off the shelf into <paramref name="drop"/>,
    /// for the caller to close.
    /// </summary>
    public T? Take(EmbedStyle style, string subtitles, List<T> drop)
    {
        foreach (var page in idle.Where(p => p.Broken || p.Subtitles != subtitles).ToList())
        {
            idle.Remove(page);
            drop.Add(page);
        }
        var found = idle.FirstOrDefault(p => p.Style == style);
        if (found is not null)
            idle.Remove(found);
        return found;
    }

    /// <summary>
    /// Every kept page comes off the shelf into <paramref name="drop"/>, for the caller to close,
    /// and the page with no video may load again: the window was minimised, Home was left, or
    /// memory is short.
    /// </summary>
    public void Empty(List<T> drop)
    {
        drop.AddRange(idle);
        idle.Clear();
        Warmed = false;
    }

    /// <summary>Take a page back. False when it is not kept (broken, already there, or no room), for the caller to close.</summary>
    public bool Give(T page)
    {
        if (page.Broken || idle.Any(p => ReferenceEquals(p, page)) || idle.Count(p => p.Style == page.Style) >= keep)
            return false;
        idle.Add(page);
        return true;
    }
}

/// <summary>What the hover card's trailer does when the pointer cues a cover, leaves it, or opens its card.</summary>
public enum HoverTrailerStep
{
    /// <summary>Nothing changes.</summary>
    None,
    /// <summary>Start loading this title's trailer, held: silent and unseen.</summary>
    Start,
    /// <summary>Let go of the trailer loading or playing for the hover.</summary>
    Stop,
    /// <summary>Let go of the one there, then start this title's.</summary>
    Replace,
    /// <summary>The card takes the trailer already loading for it.</summary>
    Adopt,
}

/// <summary>
/// Which title the hover card's trailer is loading for, and whether its card has taken it, as on
/// the Mac: the pointer resting on a cover cues its trailer 300 ms before the card opens, the
/// pointer leaving the cover before then lets it go, and the card that opens takes it. A cue that
/// comes while another card still has its trailer waits for that card to close. No WinUI in it,
/// so the rules are tested.
/// </summary>
public sealed class HoverCue
{
    /// <summary>How long the pointer rests on a cover before its trailer is cued: 300 ms before the card's 450.</summary>
    public static readonly TimeSpan Wait = TimeSpan.FromMilliseconds(150);

    /// <summary>When to cue, for a dwell of <paramref name="dwell"/>; never when the card opens as soon (scanning a row).</summary>
    public static TimeSpan? After(TimeSpan dwell) => dwell > Wait ? Wait : null;

    /// <summary>The title the trailer is for, while there is one.</summary>
    public uint? Id { get; private set; }

    /// <summary>Its card has taken it.</summary>
    public bool Adopted { get; private set; }

    /// <summary>A cue that came while a card had its trailer: it starts once that card closes.</summary>
    public uint? Queued { get; private set; }

    /// <summary>The pointer has rested on title <paramref name="id"/>'s cover.</summary>
    public HoverTrailerStep Cue(uint id)
    {
        if (Id == id)
            return HoverTrailerStep.None;
        // An open card keeps its trailer until it closes, which is moments away: the pointer has left it.
        if (Adopted)
        {
            Queued = id;
            return HoverTrailerStep.None;
        }
        var step = Id is null ? HoverTrailerStep.Start : HoverTrailerStep.Replace;
        Id = id;
        return step;
    }

    /// <summary>The pointer left title <paramref name="id"/>'s cover before its card opened.</summary>
    public HoverTrailerStep Uncue(uint id)
    {
        if (Queued == id)
            Queued = null;
        if (Id != id || Adopted)
            return HoverTrailerStep.None;
        Id = null;
        return HoverTrailerStep.Stop;
    }

    /// <summary>A scroll: covers move from under the pointer, and no cue stands. An open card closes by itself.</summary>
    public HoverTrailerStep Scrolled()
    {
        Queued = null;
        return Id is { } id ? Uncue(id) : HoverTrailerStep.None;
    }

    /// <summary>Every trailer is let go of: nothing is cued or waits.</summary>
    public HoverTrailerStep Reset()
    {
        var had = Id;
        Id = Queued = null;
        Adopted = false;
        return had is null ? HoverTrailerStep.None : HoverTrailerStep.Stop;
    }

    /// <summary>
    /// Title <paramref name="id"/>'s card opened. <paramref name="live"/>: the trailer cued for it is
    /// still loading or playing. With <paramref name="plays"/> false (no trailer, or trailers may
    /// not play now) any trailer there is let go.
    /// </summary>
    public HoverTrailerStep Open(uint id, bool live, bool plays)
    {
        var had = Id;
        Queued = null;
        if (!plays)
        {
            Id = null;
            Adopted = false;
            return had is null ? HoverTrailerStep.None : HoverTrailerStep.Stop;
        }
        Id = id;
        Adopted = true;
        if (had == id && live)
            return HoverTrailerStep.Adopt;
        return had is null ? HoverTrailerStep.Start : HoverTrailerStep.Replace;
    }

    /// <summary>
    /// The card that had the trailer closed: it is let go of, and a queued cue starts
    /// (<see cref="HoverTrailerStep.Replace"/>, or <see cref="HoverTrailerStep.Start"/> when there was none).
    /// </summary>
    public HoverTrailerStep Closed()
    {
        var had = Id;
        Adopted = false;
        Id = Queued;
        Queued = null;
        if (Id is not null)
            return had is null ? HoverTrailerStep.Start : HoverTrailerStep.Replace;
        return had is null ? HoverTrailerStep.None : HoverTrailerStep.Stop;
    }
}

/// <summary>
/// Trailers as the Windows app has them: the engine's page and the identity it is loaded under,
/// whether a title's trailer may start by itself, and the words a preview shows.
/// </summary>
public static class Trailers
{
    /// <summary>
    /// Who the app is to YouTube: the origin the page is loaded under, which the player is told as
    /// its <c>origin</c> and the web view sends as the Referer. An embed without one fails (error 153).
    /// </summary>
    public const string Origin = "https://app.kaeri.windows";

    public static AppIdentity Identity { get; } = new(Origin);

    /// <summary>The page is served from here, at <c>/trailer.html</c>.</summary>
    public static string Host => new Uri(Origin).Host;

    /// <summary>The app's own player plays this title's trailer (YouTube only).</summary>
    public static bool Embeddable(Entry entry) =>
        entry.Trailer is { } trailer && KaeriFfiMethods.TrailerEmbeddable(trailer);

    /// <summary>The playback setting's subtitle language, which the page's captions follow.</summary>
    public static string SubtitleLanguage(Known known) => known.Setting(SettingKey.Subtitles)?.Value ?? "";

    /// <summary>What YouTube's error code says, for the log.</summary>
    public static string Meaning(uint code) => code switch
    {
        0 => "the page or the player API did not load",
        2 => "a bad video id",
        5 => "the HTML5 player failed",
        100 => "the video is gone or private",
        101 or 150 => "embedding not allowed, or YouTube's sign-in check (a flagged network, such as a VPN exit)",
        152 => "the embed's identity was refused",
        153 => "the embed sent no identity (Referer)",
        _ => "an unknown code",
    };

    /// <summary>
    /// The page to load for the title's trailer, or null for one the app does not play. Its
    /// captions follow <paramref name="subtitles"/>, the playback setting's language.
    /// </summary>
    public static string? Page(Entry entry, EmbedStyle style, string subtitles = "")
    {
#if KAERI_TESTING
        // The fixture scene's made-up trailer plays a gradient, with no network.
        if (entry.Trailer?.Id == FixtureId)
            return FixturePage;
        if (entry.Trailer?.Id == FixtureStuckId)
            return FixtureStuckPage;
#endif
        return entry.Trailer is { } trailer ? KaeriFfiMethods.TrailerPage(trailer, style, Identity, subtitles) : null;
    }

    /// <summary>The preview's page with no video in it, loaded ahead of the first hover: a kept page then only switches video.</summary>
    public static string? WarmPage(string subtitles = "") =>
        KaeriFfiMethods.TrailerPage(null, EmbedStyle.Banner, Identity, subtitles);

    /// <summary>
    /// The script that switches a kept page to this title's trailer, or null when the title's
    /// trailer plays in a page of its own: one the app does not play, or the fixture scene's.
    /// </summary>
    public static string? Switch(Entry entry)
    {
#if KAERI_TESTING
        if (entry.Trailer?.Id is FixtureId or FixtureStuckId)
            return null;
#endif
        return entry.Trailer is { } trailer ? KaeriFfiMethods.TrailerLoadScript(trailer) : null;
    }

    /// <summary>
    /// Whether this title's trailer may start by itself now: the engine has said trailers may
    /// play on this device and network, and the title is one the spoiler rule does not hold back.
    /// </summary>
    public static bool MayAutoplay(Known known, Entry entry)
    {
        if (known.Autoplay is not { Trailers: true } || !Embeddable(entry))
            return false;
        return KaeriFfiMethods.TrailerMayAutoplay(entry, Spoilers.On(known, entry.Id));
    }

    /// <summary>The synopsis without the parts a spoiler tag marks, for the preview's two lines.</summary>
    public static string SpoilerFree(Entry entry)
    {
        if (entry.SynopsisSpans is not { Length: > 0 } spans)
            return entry.Synopsis;
        return string.Concat(spans.Where(s => !s.Spoiler).Select(s => s.Text)).Trim();
    }

#if KAERI_TESTING
    /// <summary>
    /// The fixture scene's titles carry no trailer of their own: most are given a made-up one, so
    /// the banner and the preview have a video to play, and one in five none, so that state shows.
    /// </summary>
    public const string FixtureId = "fixtureclip1";

    /// <summary>A trailer that never plays: the window's loading, then its "Couldn't open the trailer".</summary>
    public const string FixtureStuckId = "fixtureclip-stuck";

    public const string FixtureStuckPage = """
        <!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;background:transparent}</style></head><body><script>
        function say(m){try{window.chrome.webview.postMessage(m)}catch(e){}}
        window.trailer={play:function(){},pause:function(){},mute:function(){},unmute:function(){},remaining:function(){return -1}};
        say("ready");
        </script></body></html>
        """;

    /// <summary>
    /// A page that says what the real one would, for the shots and for trying the app offline: a
    /// slow gradient in place of the video, <c>ready</c> at once and <c>playing</c> a moment later.
    /// </summary>
    public const string FixturePage = """
        <!doctype html><html><head><meta charset="utf-8"><style>
        html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}
        [id=clip]{width:100%;height:100%;background:linear-gradient(120deg,#1d3557,#e76f51,#f4a261,#264653);
        background-size:300% 300%;animation:drift 6s linear infinite}
        @keyframes drift{0%{background-position:0% 50%}100%{background-position:100% 50%}}
        </style></head><body><div id="clip"></div><script>
        function say(m){try{window.chrome.webview.postMessage(m)}catch(e){}}
        var t0=Date.now(),len=30;
        window.trailer={play:function(){say("playing")},pause:function(){say("paused")},mute:function(){},unmute:function(){},
        remaining:function(){return Math.max(0,len-(Date.now()-t0)/1000)}};
        say("ready");setTimeout(function(){say("playing")},700);setTimeout(function(){say("ended")},len*1000);
        </script></body></html>
        """;

    public static Entry WithFixtureTrailer(Entry entry) =>
        entry.Trailer is null && entry.Id % 5 != 4
            ? entry with { Trailer = new Trailer(TrailerHost.YouTube, FixtureId), TrailerUrl = "https://video.example/watch?v=" + FixtureId }
            : entry;
#endif
}
