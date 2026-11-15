using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A title page's muted trailer, to the trailers board. A title that may play one opens with the
/// band tall enough for it, decided once per visit, so nothing on the page moves. It plays in its
/// own 448 by 252 place at the trailing end of the band once the page has settled, with the
/// ground around it; the art fades away entirely when a frame is playing and comes back half a
/// second before the end, and the player is let go of under it. Scrolling the page by more than
/// 8 px fades the art back and pauses the trailer under it; scrolling back to the top within 30 s
/// plays it again and fades the art away once it plays, and after 30 s away it is let go of for
/// this visit. Leaving the page, starting an episode, opening a sheet or the window losing focus
/// lets it go at once.
/// </summary>
sealed class PageTrailer
{
    /// <summary>The page has settled: its transition has ended and half a second has passed.</summary>
    static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(900);
    static readonly TimeSpan Away = TimeSpan.FromSeconds(30);
    /// <summary>Scrolling further than this pauses it.</summary>
    const double Scrolled = 8;

    /// <summary>The band under the title bar for a title that may play a trailer: the title bar's 48 to 328.</summary>
    public const double Band = 280;

    /// <summary>The video's place in the band: 448 by 252, 12 below its top and 36 from its trailing edge.</summary>
    public const double VideoWidth = 448, VideoHeight = 252, VideoTop = 12, VideoMargin = 36;

    /// <summary>The sound button: 40 square, 8 under the video's trailing corner.</summary>
    public const double SoundSide = 40, SoundGap = 8;

    /// <summary>The page is wide enough for the video beside the cover and title.</summary>
    const double WideEnough = 760;

    readonly Ctx ctx = Ctx.Current;
    readonly TitlePage page;
    readonly Grid banner;
    readonly UIElement art;
    readonly ScrollViewer scroll;
    readonly Button sound;
    readonly Func<bool> reserved;
    readonly Func<Control> focusAfter;
    readonly TrailerStage stage;
    readonly DispatcherQueueTimer settle, away, watch;
    bool soundOn;
    uint id;
    bool settled, abandoned, released;

    /// <param name="focusAfter">Where keyboard focus goes when the sound button hides under it.</param>
    public PageTrailer(TitlePage page, Grid banner, UIElement art, ScrollViewer scroll, Button sound, Func<bool> reserved, Func<Control> focusAfter)
    {
        this.page = page;
        this.banner = banner;
        this.art = art;
        this.scroll = scroll;
        this.sound = sound;
        this.reserved = reserved;
        this.focusAfter = focusAfter;
        stage = new TrailerStage(banner, EmbedStyle.Banner, art, Trailers.SubtitleLanguage(ctx.Known));
        stage.Place(VideoWidth, VideoHeight, new Thickness(0, VideoTop, VideoMargin, 0), HorizontalAlignment.Right);
        stage.Showing += Showing;
        settle = Timers.Weak(this, Settle, false, static t =>
        {
            t.settled = true;
            t.MaybeStart();
        });
        // Not again on this visit.
        away = Timers.Weak(this, Away, false, static t =>
        {
            t.abandoned = true;
            t.stage.Stop();
        });
        // A sheet over the page lets the trailer go; the window's own popups are told apart by what they hold.
        watch = Timers.Weak(this, TimeSpan.FromMilliseconds(400), true, static t =>
        {
            if (t.SheetOpen())
                t.stage.Stop();
        });
        sound.Click += OnSound;
        scroll.ViewChanged += OnScroll;
        ctx.Window.Activated += OnActivated;
    }

    public bool Playing => stage.Playing;

    /// <summary>
    /// The title may play a trailer here: the engine allows it on this device and network (not on a
    /// metered connection, in energy saver or with animation effects off), the title's trailer is
    /// one the app plays and the spoiler rule lets it, motion is on, the web view is there and the
    /// page is <paramref name="width"/> wide, enough for the video. The page asks once per visit and
    /// opens with the band tall enough when the answer is yes.
    /// </summary>
    public bool Wanted(Entry entry, double width) =>
        TrailerHub.Available && Motion.Enabled && width >= WideEnough && Trailers.MayAutoplay(ctx.Known, entry);

    /// <summary>The page is shown for title <paramref name="title"/>: wait for it to settle, then see whether the trailer may play.</summary>
    public void Open(uint title)
    {
        id = title;
        settled = false;
        abandoned = false;
        settle.Stop();
        settle.Start();
    }

    /// <summary>Another season picked in place: what played goes at once, under the page's crossfade.</summary>
    public void Switch(uint title)
    {
        settle.Stop();
        away.Stop();
        stage.Stop(fade: false);
        Motion.Set(art, true);
        Open(title);
    }

    /// <summary>Everything the page holds is let go of: the trailer, its timers and its hold on the window.</summary>
    public void Release()
    {
        if (released)
            return;
        released = true;
        settle.Stop();
        away.Stop();
        watch.Stop();
        sound.Click -= OnSound;
        scroll.ViewChanged -= OnScroll;
        ctx.Window.Activated -= OnActivated;
        stage.Stop(fade: false);
        TrailerHub.PageShowing = false;
    }

    void OnActivated(object sender, WindowActivatedEventArgs e)
    {
        if (released)
            return;
        if (e.WindowActivationState == WindowActivationState.Deactivated)
        {
            // The window lets every trailer go (TrailerHub.StopAll); this one waits to settle again.
            settle.Stop();
        }
        else if (settled)
        {
            // Back in front: settle again before it plays.
            settle.Start();
        }
    }

    bool SheetOpen() =>
        page.XamlRoot is { } root && VisualTreeHelper.GetOpenPopupsForXamlRoot(root).Any(p => p.Child is ContentDialog);

    /// <summary>Start the trailer if everything allows it now. Called as the title's facts arrive, as the engine's word on autoplay changes, and when the page settles.</summary>
    public void MaybeStart()
    {
        if (released || !settled || abandoned || stage.Active || scroll.VerticalOffset > Scrolled
            || !reserved() || !Motion.Enabled || !page.IsLoaded || ctx.Window.PlayerUp || !ctx.Window.InFront || SheetOpen())
            return;
        if (ctx.Known.Entry(id) is not { } entry || !Trailers.MayAutoplay(ctx.Known, entry))
            return;
        if (!stage.Fits(banner.ActualWidth, banner.ActualHeight))
            return;
        if (stage.Start(entry))
            watch.Start();
    }

    void OnScroll(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        if (scroll.VerticalOffset > Scrolled)
        {
            // The art comes back over the video first, and the player pauses once it is covered.
            if (stage.Playing)
            {
                stage.Pause();
                away.Start();
            }
        }
        else if (away.IsRunning)
        {
            // Play, then the art fades away once a frame plays again.
            away.Stop();
            stage.Resume();
        }
    }

    /// <summary>The video shows or goes: the sound button fades in or away with the art, and the page does not move.</summary>
    void Showing(bool shown)
    {
        TrailerHub.PageShowing = shown;
        if (shown)
        {
            HoverPreview.Close();
            Sound(false);
            Motion.FadeIn(sound);
        }
        else
        {
            // The button leaves the Tab order with the video; focus on it stays on the page.
            if (sound.FocusState != FocusState.Unfocused)
                focusAfter().Focus(sound.FocusState);
            Motion.Hide(sound);
        }
        if (!stage.Active)
            watch.Stop();
    }

    void OnSound(object sender, RoutedEventArgs e) => Sound(!soundOn);

    /// <summary>Sound on or off for this trailer only; every trailer starts muted.</summary>
    void Sound(bool on)
    {
        soundOn = on;
        stage.SetSound(on);
        if (sound.Content is FontIcon icon)
            icon.Glyph = on ? "" : "";
    }
}
