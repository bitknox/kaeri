using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows;

/// <summary>
/// A trailer in one place (a hover preview, the Play trailer window). The art lies over the web
/// view and holds the video's place; it fades away only when the page says a frame is playing,
/// so the wait is never seen, and it comes back over the video before the trailer is held or let
/// go of, so the player's own pause and end screens are never seen. Nothing is ever drawn over
/// the video, and the view takes no pointer or keyboard input of its own.
/// A preview's trailer plays in one of <see cref="TrailerPool"/>'s kept pages, which only switches
/// video, and may load held: silent and unseen in the pool's dock until a card takes it
/// (<see cref="Adopt"/>) and shows it (<see cref="Unhold"/>).
/// </summary>
sealed class TrailerStage
{
    /// <summary>YouTube's smallest player: 200 by 200 px.</summary>
    public const double MinSide = 200;

    static readonly TimeSpan GiveUp = TimeSpan.FromSeconds(8);

    Panel host;
    UIElement? art;
    readonly EmbedStyle style;
    readonly TrailerFlow flow = new();
    readonly string subtitles;
    /// <summary>A page of the caller's own, never one of the pool's.</summary>
    readonly Func<Entry, string?>? pageFor;
    TrailerPage? page;
    DispatcherQueueTimer? timeout, ending;
    string id = "";
    long started;
    int generation;
    bool warm;
    double width, height;
    (double Width, double Height, Thickness Margin, HorizontalAlignment Side)? fixedPlace;

    /// <summary>The art is this far from the end when it starts coming back.</summary>
    static readonly TimeSpan BeforeTheEnd = TimeSpan.FromMilliseconds(500);

    /// <summary>The video is on screen (true) or has gone (false).</summary>
    public event Action<bool>? Showing;

    /// <summary>The window's trailer has not played within 8 s and has been let go of.</summary>
    public event Action? TimedOut;

    /// <summary>The page refused it or it could not start, with YouTube's code (0 for the page's own).</summary>
    public event Action<uint>? Refused;

    /// <param name="art">What lies over the video's place and fades away when a frame plays (banner and preview only).</param>
    public TrailerStage(Panel host, EmbedStyle style, UIElement? art = null, string subtitles = "", Func<Entry, string?>? pageFor = null)
    {
        this.host = host;
        this.style = style;
        this.art = art;
        this.subtitles = subtitles;
        this.pageFor = pageFor;
    }

    WebView2? View => page?.View;

    public TrailerFlow Flow => flow;
    public bool Active => flow.Active;
    public bool Playing => flow.Showing;

    /// <summary>
    /// The size of the area the video may fill. The player letterboxes inside it and crops
    /// nothing; the view is the largest 16:9 that fits, centred.
    /// </summary>
    public void Size(double w, double h)
    {
        (width, height) = (w, h);
        Fit();
    }

    /// <summary>The video sits in this exact place: the title page's band gives it a 16:9 box of its own.</summary>
    public void Place(double w, double h, Thickness margin, HorizontalAlignment side)
    {
        fixedPlace = (w, h, margin, side);
        Fit();
    }

    void Fit()
    {
        if (View is not { } view)
            return;
        if (fixedPlace is { } place)
        {
            view.Width = place.Width;
            view.Height = place.Height;
            view.Margin = place.Margin;
            view.HorizontalAlignment = place.Side;
            view.VerticalAlignment = VerticalAlignment.Top;
            return;
        }
        view.Margin = default;
        if (style == EmbedStyle.Full)
        {
            // The window's own: the view fills it, and YouTube's player letterboxes inside.
            view.Width = view.Height = double.NaN;
            view.HorizontalAlignment = HorizontalAlignment.Stretch;
            view.VerticalAlignment = VerticalAlignment.Stretch;
            return;
        }
        if (width <= 0 || height <= 0)
            return;
        view.Height = height;
        view.Width = Math.Min(width, height * 16 / 9);
        view.HorizontalAlignment = HorizontalAlignment.Center;
        view.VerticalAlignment = VerticalAlignment.Top;
    }

    /// <summary>This stage's area would give the player at least 200 by 200.</summary>
    public bool Fits(double w, double h) =>
        flow.Phase != TrailerPhase.Done
        && (fixedPlace is { } place ? w >= place.Width + place.Margin.Right + 260 : Math.Min(w, h * 16 / 9) >= 356 && h >= MinSide);

    /// <summary>Load it unseen and silent, until <see cref="Unhold"/>: call before <see cref="Start"/>.</summary>
    public void Hold() => flow.Hold();

    /// <summary>Load the title's trailer. False when it cannot: no runtime, nothing the player plays, or one already held.</summary>
    public bool Start(Entry entry)
    {
        if (!TrailerHub.Available || page is not null || !flow.CanStart)
            return false;
        var index = art is null ? host.Children.Count : Math.Max(0, host.Children.IndexOf(art));
        // Under the art, which holds the place until a frame plays.
        var taken = style == EmbedStyle.Banner && pageFor is null ? TrailerPool.Take(entry, subtitles, host, index, out warm) : null;
        if (taken is null)
        {
            warm = false;
            if ((pageFor ?? (e => Trailers.Page(e, style, subtitles)))(entry) is not { } html)
                return false;
            taken = TrailerPage.Open(style, html, subtitles, pooled: false, host, index);
        }
        flow.Start();
        page = taken;
        id = entry.Trailer?.Id ?? "";
        started = Environment.TickCount64;
        var mine = ++generation;
        page.Hear = text => Heard(mine, text);
        page.View.Opacity = style == EmbedStyle.Full || art is not null || flow.Held ? 1 : 0;
        Fit();
        Log(FfiLogLevel.Debug, $"loading {id}, {Kind}");
        timeout = host.DispatcherQueue.CreateTimer();
        timeout.Interval = GiveUp;
        timeout.IsRepeating = false;
        timeout.Tick += (_, _) =>
        {
            Log(FfiLogLevel.Warn, $"{id} gave up after {Environment.TickCount64 - started} ms, never playing, {Kind}");
            var effect = flow.GiveUp();
            if (effect != TrailerEffect.None && style == EmbedStyle.Full)
                TimedOut?.Invoke();
            Apply(effect);
        };
        timeout.Start();
        if (style == EmbedStyle.Banner)
            TrailerHub.Claim(this);
        return true;
    }

    string Kind => (warm ? "warm" : "cold") + (flow.Held ? ", held" : "");

    /// <summary>
    /// A held trailer goes under <paramref name="cover"/> in <paramref name="place"/>, an area of
    /// <paramref name="w"/> by <paramref name="h"/>: a hover card taking the trailer cued for it.
    /// It stays held, and plays on where it is.
    /// </summary>
    public void Adopt(Panel place, UIElement cover, double w, double h)
    {
        host = place;
        art = cover;
        (width, height) = (w, h);
        if (page is null)
            return;
        page.MoveTo(place, Math.Max(0, place.Children.IndexOf(cover)));
        page.View.Opacity = 1;
        Fit();
    }

    /// <summary>It is being seen now: the art fades from a frame already playing, or once one plays.</summary>
    public void Unhold() => Apply(flow.Unhold());

    void Heard(int token, string? text)
    {
        if (token != generation)
            return;
        var message = text is null ? null : KaeriFfiMethods.TrailerMessage(text);
        var ms = Environment.TickCount64 - started;
        if (message is TrailerMessage.Error error)
        {
            Log(FfiLogLevel.Warn, $"{id} failed after {ms} ms: error {error.Code}, {Trailers.Meaning(error.Code)}");
            Refused?.Invoke(error.Code);
        }
        else if (message is TrailerMessage.Playing && flow.Phase is TrailerPhase.Loading or TrailerPhase.Paused)
            Log(FfiLogLevel.Debug, flow.Held ? $"{id} playing after {ms} ms, {Kind}" : $"{id} playing, shown after {ms} ms, {Kind}");
        Apply(flow.Heard(message));
        // It played: no giving up on it now, held or not.
        if (flow.Phase == TrailerPhase.Playing)
            timeout?.Stop();
    }

    static void Log(FfiLogLevel level, string line) => KaeriFfiMethods.LogLine(level, "trailer", line);

    void Apply(TrailerEffect effect, bool fade = true)
    {
        if (effect.HasFlag(TrailerEffect.PausePlayer))
        {
            if (art is not null && effect.HasFlag(TrailerEffect.HideVideo))
            {
                // Pause once the art covers it, so the paused screen is never seen.
                var mine = page;
                var wait = host.DispatcherQueue.CreateTimer();
                wait.Interval = Motion.FadeOutTime;
                wait.IsRepeating = false;
                wait.Tick += (_, _) =>
                {
                    if (page == mine && flow.Phase == TrailerPhase.Paused)
                        Control(TrailerControl.Pause);
                };
                wait.Start();
            }
            else
                Control(TrailerControl.Pause);
        }
        if (effect.HasFlag(TrailerEffect.PlayPlayer))
            Control(TrailerControl.Play);
        if (effect.HasFlag(TrailerEffect.ShowVideo) && View is { } view)
        {
            timeout?.Stop();
            if (art is not null)
                Motion.Cover(art, false);
            else if (style == EmbedStyle.Banner)
                Motion.Video(view, true);
            Watch(true);
            Showing?.Invoke(true);
        }
        if (effect.HasFlag(TrailerEffect.HideVideo))
        {
            Watch(false);
            if (art is not null)
            {
                if (!effect.HasFlag(TrailerEffect.Release))
                    Motion.Cover(art, true);
            }
            else if (!effect.HasFlag(TrailerEffect.Release) && View is { } shown)
                Motion.Video(shown, false);
            Showing?.Invoke(false);
        }
        if (effect.HasFlag(TrailerEffect.Release))
            Let(fade: fade && effect.HasFlag(TrailerEffect.HideVideo) && style == EmbedStyle.Banner);
    }

    /// <summary>
    /// While the video shows, ask the player how long is left: half a second before the end the
    /// art starts back over it, and the player is let go of under the art.
    /// </summary>
    void Watch(bool on)
    {
        if (art is null || style != EmbedStyle.Banner)
            return;
        if (!on)
        {
            ending?.Stop();
            return;
        }
        ending ??= host.DispatcherQueue.CreateTimer();
        ending.Interval = TimeSpan.FromMilliseconds(100);
        ending.IsRepeating = true;
        ending.Tick -= OnEnding;
        ending.Tick += OnEnding;
        ending.Start();
    }

    async void OnEnding(DispatcherQueueTimer sender, object args)
    {
        var mine = page;
        if (mine is null || !flow.Showing)
            return;
        var text = await mine.Ask("window.trailer && window.trailer.remaining ? window.trailer.remaining() : -1");
        if (page != mine || !flow.Showing || text is null)
            return;
        if (double.TryParse(text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var left)
            && left >= 0 && left <= BeforeTheEnd.TotalSeconds)
            Apply(flow.Heard(new TrailerMessage.Ended()));
    }

    /// <summary>Hold the trailer: it pauses and the banner comes back over it.</summary>
    public void Pause() => Apply(flow.Pause());

    /// <summary>Play it again from where it paused; the video shows when the page says it is playing.</summary>
    public void Resume()
    {
        Apply(flow.Resume());
        timeout?.Stop();
    }

    /// <summary>Sound on or off, for this trailer only.</summary>
    public void SetSound(bool on) => Control(on ? TrailerControl.Unmute : TrailerControl.Mute);

    /// <summary>
    /// Let go of the view and bring the banner back. With <paramref name="fade"/> false the view goes
    /// at once: its card is closing over it.
    /// </summary>
    public void Stop(bool fade = true) => Apply(flow.Stop(), fade);

    void Control(TrailerControl control) => page?.Control(control);

    /// <summary>
    /// The page goes back to the pool, or is closed when it is not the pool's: after the video has
    /// faded away when it was showing.
    /// </summary>
    void Let(bool fade)
    {
        generation++;
        timeout?.Stop();
        timeout = null;
        ending?.Stop();
        TrailerHub.Release(this);
        var gone = page;
        page = null;
        if (gone is null)
            return;
        gone.Hear = null;
        void Done()
        {
            if (gone.Pooled)
                TrailerPool.Give(gone);
            else
                gone.Close();
        }
        if (fade && art is not null)
        {
            // The art comes back over the playing video; the player goes under it.
            Motion.Cover(art, true, () =>
            {
                gone.Control(TrailerControl.Pause);
                Done();
            });
        }
        else if (fade)
        {
            gone.Control(TrailerControl.Pause);
            Motion.Video(gone.View, false, Done);
        }
        else
            Done();
    }
}
