using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows;

/// <summary>
/// The preview's trailer pages for the whole run, as on the Mac. The first loads YouTube's player
/// with no video at a still moment, then plays the first cover's trailer a moment, hidden and
/// muted, and goes back paused, so the first hover starts as fast as later ones. A page handed
/// back is paused and kept, its player loaded, and plays the next trailer through the engine's
/// load script. While one waits it lies in the dock: in the main window, so it may play, at the
/// preview's size, unseen and taking no input. The web views are private and share one session,
/// which these pages keep alive for the run, so what YouTube's player fetched stays cached
/// between hovers and nothing is written to disk.
/// </summary>
static class TrailerPool
{
    static readonly TrailerShelf<TrailerPage> shelf = new();
    static readonly TimeSpan PrimeLimit = TimeSpan.FromSeconds(8);
    static Grid? dock;
    static TrailerPage? priming;
    static bool primed;
    static DispatcherQueueTimer? primeTimeout;

    static double Height => HoverPreview.Width * 9 / 16;

    /// <summary>The run's page with no video has been asked for.</summary>
    public static bool Warmed => shelf.Warmed;

    /// <summary>Where kept pages wait, made in the main window the first time it is asked for.</summary>
    public static Panel? Dock()
    {
        if (dock is not null)
            return dock;
        if (Ctx.Current?.Window.Content is not Panel root)
            return null;
        dock = new Unseen
        {
            Width = HoverPreview.Width,
            Height = Height,
            Opacity = 0,
            IsHitTestVisible = false,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
        };
        AutomationProperties.SetAccessibilityView(dock, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        Grid.SetRowSpan(dock, 8);
        // Drawn under everything else in the window, and last in its tree: a web view there
        // stopped UI Automation's walk of the window at the dock.
        Canvas.SetZIndex(dock, -1);
        root.Children.Add(dock);
        return dock;
    }

    /// <summary>The dock: what waits in it is not there for UI Automation, as it is never seen.</summary>
    sealed partial class Unseen : Grid
    {
        protected override AutomationPeer OnCreateAutomationPeer() => new Peer(this);

        sealed partial class Peer(Unseen owner) : FrameworkElementAutomationPeer(owner)
        {
            protected override IList<AutomationPeer> GetChildrenCore() => [];
        }
    }

    /// <summary>
    /// Load the preview's player with no video, once a run and nothing after the first call. The
    /// run's first web view starts WebView2's processes, so the caller picks a still moment.
    /// </summary>
    public static void Prewarm(string subtitles)
    {
        if (!TrailerHub.Available || shelf.Warmed || Dock() is not { } at || Trailers.WarmPage(subtitles) is not { } html)
            return;
        shelf.Warm();
        var page = TrailerPage.Open(EmbedStyle.Banner, html, subtitles, pooled: true, at, at.Children.Count);
        Rest(page);
        shelf.Give(page);
        Log("the preview's player is loading ahead of the first hover");
    }

    /// <summary>
    /// Play <paramref name="first"/>'s trailer in the warm page a moment, hidden and muted, until it
    /// plays, then pause it back onto the shelf: a page that has played nothing starts its first
    /// trailer slower than its next. Once a run.
    /// </summary>
    public static void Prime(Entry first, string subtitles)
    {
        if (primed || priming is not null || Trailers.Switch(first) is not { } script || Dock() is not { } at)
            return;
        var drop = new List<TrailerPage>();
        var page = shelf.Take(EmbedStyle.Banner, subtitles, drop);
        Close(drop);
        if (page is null)
            return;
        primed = true;
        priming = page;
        var began = Environment.TickCount64;
        var id = first.Trailer?.Id ?? "";
        page.Hear = text =>
        {
            var word = text?.Trim() ?? "";
            if (word == "playing" || word.StartsWith("error:", StringComparison.Ordinal))
            {
                Log($"primed with {id}: {word} after {Environment.TickCount64 - began} ms");
                EndPrime();
            }
        };
        page.Control(TrailerControl.Mute);
        page.Run(script);
        primeTimeout = at.DispatcherQueue.CreateTimer();
        primeTimeout.Interval = PrimeLimit;
        primeTimeout.IsRepeating = false;
        primeTimeout.Tick += (_, _) =>
        {
            Log($"priming with {id} did not play in {PrimeLimit.TotalSeconds:0} s");
            EndPrime();
        };
        primeTimeout.Start();
    }

    /// <summary>The priming is over, or a trailer wants the page: it goes back onto the shelf, paused.</summary>
    public static void EndPrime()
    {
        primeTimeout?.Stop();
        primeTimeout = null;
        if (priming is not { } page)
            return;
        priming = null;
        page.Hear = null;
        page.Control(TrailerControl.Pause);
        // Ends the chrome script's count of what YouTube's frame fetched, which has no one to tell here.
        page.Run(KaeriFfiMethods.TrailerBytesScript());
        if (!shelf.Give(page))
            page.Close();
    }

    /// <summary>
    /// A page playing <paramref name="entry"/>'s trailer, put into <paramref name="host"/> at
    /// <paramref name="index"/>: a kept one when there is one, which only switches video
    /// (<paramref name="warm"/>), or a new one, kept once handed back. Null for a trailer that plays
    /// in a page of its own (the fixture scene's), which the caller makes.
    /// </summary>
    public static TrailerPage? Take(Entry entry, string subtitles, Panel host, int index, out bool warm)
    {
        warm = false;
        if (Trailers.Switch(entry) is not { } script)
            return null;
        EndPrime();
        var drop = new List<TrailerPage>();
        var page = shelf.Take(EmbedStyle.Banner, subtitles, drop);
        Close(drop);
        if (page is not null)
        {
            page.MoveTo(host, index);
            page.Run(script);
            warm = true;
            return page;
        }
        return Trailers.Page(entry, EmbedStyle.Banner, subtitles) is { } html
            ? TrailerPage.Open(EmbedStyle.Banner, html, subtitles, pooled: true, host, index)
            : null;
    }

    /// <summary>
    /// Take a page back: out of sight into the dock first, then muted and paused, so neither the
    /// player's pause screen nor its end screen is ever seen. Kept while there is room.
    /// </summary>
    public static void Give(TrailerPage page)
    {
        page.Hear = null;
        if (!page.Pooled || page.Broken || Dock() is not { } at)
        {
            page.Close();
            return;
        }
        page.MoveTo(at, at.Children.Count);
        Rest(page);
        page.Control(TrailerControl.Mute);
        page.Control(TrailerControl.Pause);
        if (!shelf.Give(page))
            page.Close();
    }

    /// <summary>The view as it waits in the dock: the preview's size, seen whole once it is moved under a card's art.</summary>
    static void Rest(TrailerPage page)
    {
        var view = page.View;
        view.Width = HoverPreview.Width;
        view.Height = Height;
        view.Margin = default;
        view.HorizontalAlignment = HorizontalAlignment.Left;
        view.VerticalAlignment = VerticalAlignment.Top;
        view.Opacity = 1;
    }

    static void Close(List<TrailerPage> pages)
    {
        foreach (var page in pages)
            page.Close();
    }

    static void Log(string line) => KaeriFfiMethods.LogLine(FfiLogLevel.Debug, "trailer", line);
}
