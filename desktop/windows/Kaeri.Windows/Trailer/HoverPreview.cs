using System.Runtime.CompilerServices;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Foundation;
using Size = Windows.Foundation.Size;

namespace Kaeri.Windows;

/// <summary>
/// The card a cover grows into when the pointer rests on it, as on the Mac: the banner, the
/// title, a facts line, two lines of synopsis and the title's main action, 384 wide, centred on
/// the cover and kept inside the window. It grows from the cover on the standard token, the
/// banner from the start and the words from 100 ms; with animation effects off it crossfades in
/// place. A click on it opens the title. Where the title has a trailer and trailers may play, it
/// plays muted over the banner once the card has grown.
/// When it opens is <see cref="HoverRest"/>'s rule, the Mac's: 450 ms after the pointer arrives on
/// the cover, or 150 ms when a preview closed less than a second ago (scanning a row), with the
/// pointer slowed. At 150 ms the trailer is cued: it loads unseen and silent
/// (<see cref="HoverTrailer"/>), so the card opens on a video that is already coming; the pointer
/// leaving the cover first lets it go. A scroll closes the card and starts the wait again from
/// the moment the page stops, so the cover that comes to rest under a still pointer opens too.
/// The preview's player is loaded once a run at a still moment, two seconds after the first
/// covers show, and primed with the first cover's trailer (<see cref="TrailerPool"/>). The
/// preview closes 150 ms after the pointer leaves both the cover and the preview, and at once on
/// a scroll, a key press, a click elsewhere or the window losing focus; after a key press nothing
/// opens until the pointer moves. Keyboard focus never opens one.
/// </summary>
static class HoverPreview
{
    static long closedAt = long.MinValue / 2;
    static readonly TimeSpan Leave = TimeSpan.FromMilliseconds(150);
    /// <summary>The video's width: 384 by 216, over YouTube's 356 by 200 floor.</summary>
    internal const double Width = 384;

    static long wheeledAt = long.MinValue / 2;
    /// <summary>Where the pointer was at the last wheel, in the window's content, for the cover under it once the page stops.</summary>
    static Point? wheelAt;
    static UIElement? heard;

    static readonly ConditionalWeakTable<FrameworkElement, Resting> targets = [];
    static Open? open;

    /// <summary>The pointer's wait on the cover it is on; one cover at a time.</summary>
    static readonly HoverRest rest = new();
    static WeakReference<FrameworkElement>? resting;
    /// <summary>Steps <see cref="rest"/> while the pointer waits on a cover or a scroll is settling.</summary>
    static DispatcherQueueTimer? ticker;
    static bool settling;

    /// <summary>What one cover keeps.</summary>
    sealed class Resting
    {
        public Func<uint> Id = () => 0;
    }

    /// <summary>Lets <paramref name="target"/> open a preview of title <paramref name="id"/> when the pointer rests on it.</summary>
    public static void Attach(FrameworkElement target, uint id) => Attach(target, () => id);

    /// <summary>As above, for a container that may be given another title: <paramref name="id"/> is asked when the preview opens.</summary>
    public static void Attach(FrameworkElement target, Func<uint> id)
    {
        if (targets.TryGetValue(target, out var known))
        {
            // A container reused for another title keeps its handlers, and takes the new id.
            known.Id = id;
            return;
        }
        targets.Add(target, new Resting { Id = id });
        target.Loaded += (_, _) =>
        {
            HearWheel(target);
            WarmSoon(target);
        };
        // Heard even when the target has handled them: a grid's item container and a button
        // take their own pointer events for their hover and press states, and a plain handler
        // never heard them, so most covers never opened a card.
        target.AddHandler(UIElement.PointerEnteredEvent, new PointerEventHandler((_, e) => Arrive(target, e)), true);
        target.AddHandler(UIElement.PointerMovedEvent, new PointerEventHandler((_, e) =>
        {
            if (!IsResting(target))
            {
                Arrive(target, e);
                return;
            }
            if (Where(target, e) is { } at)
                rest.Move(at.X, at.Y, Environment.TickCount64);
        }), true);
        target.AddHandler(UIElement.PointerExitedEvent, new PointerEventHandler((_, _) =>
        {
            if (IsResting(target))
                Depart(target);
            if (open is { } o && o.Target == target)
                o.Leaving();
        }), true);
        // A press on the cover opens the title: no preview then.
        target.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) =>
        {
            rest.Hold();
            Uncue(target);
            Close();
        }), true);
        target.Unloaded += (_, _) =>
        {
            if (IsResting(target))
                Depart(target);
        };
    }

    static bool IsResting(FrameworkElement target) =>
        resting is not null && resting.TryGetTarget(out var at) && at == target;

    static uint IdOf(FrameworkElement target) => targets.TryGetValue(target, out var r) ? r.Id() : 0;

    /// <summary>The pointer in the window's content, where covers are hit-tested after a scroll.</summary>
    static Point? Where(FrameworkElement target, PointerRoutedEventArgs e) =>
        target.XamlRoot?.Content is UIElement root ? e.GetCurrentPoint(root).Position : null;

    /// <summary>The pointer came onto <paramref name="target"/>: its wait starts.</summary>
    static void Arrive(FrameworkElement target, PointerRoutedEventArgs e)
    {
        // A touch has no hover; a pen or a mouse does.
        if (e.Pointer.PointerDeviceType == PointerDeviceType.Touch || Where(target, e) is not { } at)
            return;
        if (open is { } o && o.Target == target)
        {
            o.Stay();
            return;
        }
        if (resting is not null && resting.TryGetTarget(out var was) && was != target)
            Uncue(was);
        var now = Environment.TickCount64;
        resting = new WeakReference<FrameworkElement>(target);
        // A card a scroll just closed is not a row being scanned: a cover the page brings under
        // the pointer waits the full dwell.
        var dwell = now - wheeledAt < HoverRest.Dwell.TotalMilliseconds ? HoverRest.Dwell : HoverRest.DwellAfter(closedAt, now);
        rest.Enter(at.X, at.Y, now, dwell);
        Tick(target);
    }

    /// <summary>The pointer left <paramref name="target"/> before or after its card opened.</summary>
    static void Depart(FrameworkElement target)
    {
        rest.Leave();
        resting = null;
        Uncue(target);
    }

    static void Uncue(FrameworkElement target) => HoverTrailer.Uncue(IdOf(target));

    /// <summary>Steps the wait on the dispatcher, every 25 ms while there is something to wait for.</summary>
    static void Tick(FrameworkElement from)
    {
        if (ticker is null)
        {
            ticker = from.DispatcherQueue.CreateTimer();
            ticker.Interval = TimeSpan.FromMilliseconds(25);
            ticker.IsRepeating = true;
            ticker.Tick += (_, _) => Step();
        }
        if (!ticker.IsRunning)
            ticker.Start();
    }

    static void Step()
    {
        var now = Environment.TickCount64;
        if (settling && now - wheeledAt >= HoverCue.Wait.TotalMilliseconds)
        {
            settling = false;
            CoverUnderPointer();
        }
        FrameworkElement? target = null;
        var waiting = resting is not null && resting.TryGetTarget(out target) && rest.Waiting;
        if (!waiting)
        {
            if (!settling)
                ticker?.Stop();
            return;
        }
        switch (rest.Step(now))
        {
            case HoverRestStep.Cue:
                CueAt(target!, IdOf(target!));
                break;
            case HoverRestStep.Open:
                Show(target!, IdOf(target!), rest.From);
                break;
        }
    }

    /// <summary>
    /// The page has stopped under a still pointer: the cover now there, if any, is the one the
    /// pointer rests on, waiting from the scroll. The layout is already where the wheel's glide
    /// ends, so the hit is the cover that comes to rest there.
    /// </summary>
    static void CoverUnderPointer()
    {
        if (wheelAt is not { } at || heard is null)
            return;
        FrameworkElement? found = null;
        foreach (var element in VisualTreeHelper.FindElementsInHostCoordinates(at, heard))
            if (element is FrameworkElement fe && targets.TryGetValue(fe, out _))
            {
                found = fe;
                break;
            }
        if (resting is not null && resting.TryGetTarget(out var was) && was != found)
            Depart(was);
        if (found is not null && !IsResting(found))
        {
            resting = new WeakReference<FrameworkElement>(found);
            rest.Enter(at.X, at.Y, Environment.TickCount64, HoverRest.Dwell, start: wheeledAt);
        }
    }

    /// <summary>
    /// The pointer has rested on <paramref name="target"/> 150 ms: the title's trailer starts loading,
    /// held, when its card could play one.
    /// </summary>
    static void CueAt(FrameworkElement target, uint id)
    {
        var ctx = Ctx.Current;
        if (target.XamlRoot is null || !ctx.Window.InFront || open is { } o && o.Target == target)
            return;
        if (ctx.Known.Setting(SettingKey.HoverCards) is { } cards && !SettingsCatalog.IsOn(cards))
            return;
        if (!TrailerHub.Available || ctx.Known.Entry(id) is not { } entry || !Trailers.MayAutoplay(ctx.Known, entry))
            return;
        HoverTrailer.Cue(entry, Trailers.SubtitleLanguage(ctx.Known));
    }

    static DispatcherQueueTimer? warming;
    static long warmingFrom;

    /// <summary>
    /// Covers are on screen: two seconds on, at a still moment (no card, nothing scrolled for half
    /// a second), the preview's player loads and is primed with the first cover's trailer, as on the
    /// Mac. Once a run, and only while cards are on and trailers may play; never in a fixture scene,
    /// where nothing may be fetched.
    /// </summary>
    static void WarmSoon(FrameworkElement target)
    {
        if (warming is not null || TrailerPool.Warmed || !TrailerHub.Available || Ctx.Current.Options.Fixture)
            return;
        warmingFrom = Environment.TickCount64;
        warming = target.DispatcherQueue.CreateTimer();
        warming.Interval = TimeSpan.FromMilliseconds(250);
        warming.IsRepeating = true;
        warming.Tick += (timer, _) =>
        {
            if (Environment.TickCount64 - warmingFrom < 2000)
                return;
            const long still = 500;
            if (open is not null || Environment.TickCount64 - wheeledAt < still || Environment.TickCount64 - closedAt < still)
                return;
            // The engine has not said yet whether trailers may play: ask again at the next tick.
            if (Ctx.Current.Known.Autoplay is null)
                return;
            timer.Stop();
            warming = null;
            WarmUp();
        };
        warming.Start();
    }

    /// <summary>Load and prime the preview's player, when cards are on and trailers may play now; a later page's covers try again otherwise.</summary>
    static void WarmUp()
    {
        var ctx = Ctx.Current;
        if (ctx.Known.Setting(SettingKey.HoverCards) is { } cards && !SettingsCatalog.IsOn(cards))
            return;
        if (ctx.Known.Autoplay is not { Trailers: true })
            return;
        var subtitles = Trailers.SubtitleLanguage(ctx.Known);
        TrailerPool.Prewarm(subtitles);
        if (FirstOnScreen(ctx) is { } first)
            TrailerPool.Prime(first, subtitles);
    }

    /// <summary>The first cover on screen, top row first, whose trailer may play.</summary>
    static Entry? FirstOnScreen(Ctx ctx)
    {
        var shown = new List<(Rect At, uint Id)>();
        foreach (var (target, resting) in targets)
        {
            if (!target.IsLoaded || target.ActualWidth <= 0 || target.XamlRoot?.Content is not FrameworkElement root)
                continue;
            try
            {
                var at = target.TransformToVisual(root).TransformBounds(new Rect(0, 0, target.ActualWidth, target.ActualHeight));
                if (at.Right > 0 && at.Bottom > 0 && at.Left < root.ActualWidth && at.Top < root.ActualHeight)
                    shown.Add((at, resting.Id()));
            }
            catch (ArgumentException)
            {
                // Not in the window's tree any more.
            }
        }
        return shown
            .OrderBy(s => Math.Round(s.At.Top))
            .ThenBy(s => s.At.Left)
            .Select(s => ctx.Known.Entry(s.Id))
            .FirstOrDefault(e => e is not null && Trailers.MayAutoplay(ctx.Known, e));
    }

    /// <summary>Every wheel in the window, handled or not, marks the page as scrolling.</summary>
    static void HearWheel(FrameworkElement target)
    {
        if (target.XamlRoot?.Content is not UIElement root || root == heard)
            return;
        heard?.RemoveHandler(UIElement.PointerWheelChangedEvent, (PointerEventHandler)OnWheel);
        heard = root;
        root.AddHandler(UIElement.PointerWheelChangedEvent, (PointerEventHandler)OnWheel, true);
    }

    /// <summary>A scroll: the wait starts again once the page stops, for the cover then under the pointer.</summary>
    static void OnWheel(object sender, PointerRoutedEventArgs e)
    {
        wheeledAt = Environment.TickCount64;
        wheelAt = heard is null ? null : e.GetCurrentPoint(heard).Position;
        rest.Scrolled(wheeledAt);
        HoverTrailer.Scrolled();
        if (e.Pointer.PointerDeviceType == PointerDeviceType.Touch || heard is not FrameworkElement root)
            return;
        settling = true;
        Tick(root);
    }

    /// <summary>A key press closed the card: nothing opens again until the pointer moves.</summary>
    static void KeyPressed()
    {
        rest.Hold();
        Close();
    }

    /// <summary>Close the preview on screen, if there is one.</summary>
    public static void Close()
    {
        var was = open;
        open = null;
        if (was is not null)
            closedAt = Environment.TickCount64;
        was?.Dismiss();
    }

    /// <summary>Open a preview of the title now, for a screenshot or a key.</summary>
    public static void Show(FrameworkElement target, uint id) => Show(target, id, 0);

    static void Show(FrameworkElement target, uint id, long rested)
    {
        var ctx = Ctx.Current;
        if (target.XamlRoot is null || !ctx.Window.InFront)
            return;
        if (ctx.Known.Entry(id) is not { } entry)
            return;
        // Expand covers on hover turned off: the cover keeps its hover and its click, and no card.
        if (ctx.Known.Setting(SettingKey.HoverCards) is { } cards && !SettingsCatalog.IsOn(cards))
            return;
        // Interest shown: the sources are asked whether they have the title, as the spec ties to a
        // hover card. The engine asks once a minute at most, and Home marks a miss.
        ctx.Send(new Command.ProbeTitle(id));
        Close();
        var preview = new Open(ctx, target, entry, rested);
        open = preview;
        preview.Show();
    }

    /// <summary>The card on screen.</summary>
    sealed class Open
    {
        readonly Ctx ctx;
        readonly Popup popup = new() { IsLightDismissEnabled = false, ShouldConstrainToRootBounds = true };
        readonly Border card;
        readonly StackPanel words;
        bool closing, playing;
        /// <summary>The card's scale where it covers the cover: it grows from this and goes back to it.</summary>
        float coverScale = 1;
        TrailerStage? stage;
        readonly DispatcherQueueTimer leave;
        readonly Button sound = new();
        bool soundOn;
        readonly Grid video, art;
        readonly long rested;

        public FrameworkElement Target { get; }

        public Open(Ctx ctx, FrameworkElement target, Entry entry, long rested)
        {
            this.ctx = ctx;
            this.rested = rested;
            Target = target;
            // The video is flush at the top of the flyout, clipped only by the flyout's own corners. The
            // art lies over it and holds its place until a frame plays.
            video = new Grid { Width = Width, Height = Width * 9 / 16 };
            var banner = new Image { Stretch = Stretch.UniformToFill };
            art = new Grid { Background = (Brush)Application.Current.Resources["RaisedBrush"] };
            art.Children.Add(banner);
            video.Children.Add(art);
            ctx.Art.Bind(entry.BannerUrl ?? entry.CoverUrl, s => banner.Source = s);

            var column = new StackPanel { Width = Width, Spacing = 0 };
            column.Children.Add(video);
            var text = new StackPanel { Spacing = 0, Padding = new Thickness(16, 12, 16, 16) };
            words = text;

            var heading = new Grid { ColumnSpacing = 8, MinHeight = 32 };
            heading.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            heading.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            heading.Children.Add(new TextBlock
            {
                Text = entry.Title,
                Style = (Style)Application.Current.Resources["ShowTitleStyle"],
                MaxLines = 2,
                TextWrapping = TextWrapping.WrapWholeWords,
                TextTrimming = TextTrimming.CharacterEllipsis,
            });
            sound.Content = new FontIcon { Glyph = "\uE74F", FontSize = 14 };
            sound.Width = 32;
            sound.Height = 32;
            sound.Padding = new Thickness(0);
            sound.CornerRadius = new CornerRadius(4);
            sound.Style = (Style)Application.Current.Resources["SubtleButtonStyle"];
            // Hidden until a frame plays, but its place stays, so the card is the same size with or without a trailer.
            sound.Opacity = 0;
            sound.IsHitTestVisible = false;
            ToolTipService.SetToolTip(sound, T.TitleTrailerSound);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(sound, T.TitleTrailerSound);
            Grid.SetColumn(sound, 1);
            heading.Children.Add(sound);
            text.Children.Add(heading);

            if (Wording.MetaLine(entry) is { Length: > 0 } facts)
            {
                var line = Ui.Text(facts, "HazeCaptionStyle");
                line.Margin = new Thickness(0, 4, 0, 0);
                text.Children.Add(line);
            }
            if (Trailers.SpoilerFree(entry) is { Length: > 0 } synopsis)
            {
                var lines = Ui.Text(synopsis, "BodyTextBlockStyle");
                lines.Margin = new Thickness(0, 8, 0, 0);
                lines.MaxLines = 2;
                lines.TextTrimming = TextTrimming.CharacterEllipsis;
                text.Children.Add(lines);
            }
            var queue = PlayableNext.Item(entry, ctx.Known.Episodes.GetValueOrDefault(entry.Id), Casing.Sentence);
            if (queue.Button is { } label)
            {
                var play = new Button
                {
                    Content = label,
                    Style = (Style)Application.Current.Resources["PrimaryActionStyle"],
                    Margin = new Thickness(0, 14, 0, 0),
                    HorizontalAlignment = HorizontalAlignment.Left,
                    IsEnabled = queue.Playable,
                };
                play.Click += (_, _) =>
                {
                    Close();
                    ctx.Play(entry.Id, queue.Episode.ToString());
                };
                text.Children.Add(play);
            }
            column.Children.Add(text);

            // The canvas's Raised with the system's flyout hairline, over a shadow.
            card = new Border
            {
                Child = column,
                Width = Width + 2,
                CornerRadius = new CornerRadius(8),
                Background = (Brush)Application.Current.Resources["RaisedBrush"],
                BorderBrush = (Brush)Application.Current.Resources["SurfaceStrokeColorFlyoutBrush"],
                BorderThickness = new Thickness(1),
                Shadow = new ThemeShadow(),
                Translation = new System.Numerics.Vector3(0, 0, 32),
            };
            popup.Child = card;
            popup.Closed += (_, _) =>
            {
                HoverTrailer.Closed(stage);
                if (open == this)
                {
                    open = null;
                    closedAt = Environment.TickCount64;
                }
            };

            column.PointerEntered += (_, _) => Stay();
            column.PointerExited += (_, _) => Leaving();
            // A click anywhere on the card but its buttons opens the title, as on the Mac.
            card.Tapped += (_, e) =>
            {
                if (e.OriginalSource is DependencyObject source && Within<ButtonBase>(source, card))
                    return;
                Close();
                ctx.OpenTitle(entry.Id);
            };
            leave = target.DispatcherQueue.CreateTimer();
            leave.Interval = Leave;
            leave.IsRepeating = false;
            leave.Tick += (_, _) => Close();

            sound.Click += (_, _) =>
            {
                if (stage is null)
                    return;
                soundOn = !soundOn;
                stage.SetSound(soundOn);
                (sound.Content as FontIcon)!.Glyph = soundOn ? "\uE767" : "\uE74F";
            };
            entryToPlay = entry;
        }

        readonly Entry entryToPlay;

        /// <summary>
        /// The trailer cued for this card, or one begun now, goes under the art, held until the
        /// card has grown. None where the title has no trailer or trailers may not play now.
        /// </summary>
        void TakeTrailer()
        {
            var plays = TrailerHub.Available && Trailers.MayAutoplay(ctx.Known, entryToPlay);
            stage = HoverTrailer.Open(entryToPlay, plays, Trailers.SubtitleLanguage(ctx.Known), video, art, video.Width, video.Height);
            if (stage is null)
                return;
            var id = entryToPlay.Id;
            stage.Showing += shown =>
            {
                if (closing)
                    return;
                playing = shown;
                soundOn = false;
                sound.IsHitTestVisible = shown;
                if (shown)
                {
                    // The time from the pointer resting on the cover to the first frame, for the log.
                    if (rested > 0)
                        KaeriFfiMethods.LogLine(FfiLogLevel.Debug, "trailer", $"hover card {id}: video shown {Environment.TickCount64 - rested} ms after the pointer rested");
                    (sound.Content as FontIcon)!.Glyph = "\uE74F";
                    Motion.Fade(sound, 1);
                }
                else
                    Motion.Fade(sound, 0);
            };
        }

        UIElement? root;
        readonly KeyEventHandler onKey = (_, _) => KeyPressed();
        readonly PointerEventHandler onWheel = (_, _) => Close();

        static bool Within<T>(DependencyObject element, DependencyObject stop) where T : DependencyObject
        {
            for (var at = element; at is not null && at != stop; at = VisualTreeHelper.GetParent(at))
                if (at is T)
                    return true;
            return false;
        }

        public void Show()
        {
            // A key press or a scroll closes it at once.
            root = Target.XamlRoot?.Content;
            if (root is not FrameworkElement window || Target.ActualWidth <= 0)
                return;
            root.AddHandler(UIElement.KeyDownEvent, onKey, true);
            root.AddHandler(UIElement.PointerWheelChangedEvent, onWheel, true);
            // Centred on the cover, its top a little above the cover's, inside the page it is on,
            // so it never lies over the sidebar.
            card.Measure(new Size(card.Width, double.PositiveInfinity));
            var height = card.DesiredSize.Height;
            var cover = Target.TransformToVisual(root).TransformBounds(new Rect(0, 0, Target.ActualWidth, Target.ActualHeight));
            var area = new Rect(0, 0, window.ActualWidth, window.ActualHeight);
            for (var up = VisualTreeHelper.GetParent(Target); up is not null; up = VisualTreeHelper.GetParent(up))
                if (up is Page page && page.ActualWidth > card.Width + 16)
                {
                    area = page.TransformToVisual(root).TransformBounds(new Rect(0, 0, page.ActualWidth, page.ActualHeight));
                    break;
                }
            var x = Math.Clamp(cover.X + cover.Width / 2 - card.Width / 2, area.X + 8, Math.Max(area.X + 8, area.Right - card.Width - 8));
            var y = Math.Clamp(cover.Y - 24, 8, Math.Max(8, window.ActualHeight - height - 8));
            popup.XamlRoot = Target.XamlRoot;
            popup.HorizontalOffset = x;
            popup.VerticalOffset = y;
            popup.IsOpen = true;
            TakeTrailer();
            if (!Grow(cover, x, y, height))
                stage?.Unhold();
        }

        /// <summary>
        /// From the cover to the card on the standard spring, as on the Mac: the card grows from where
        /// the cover is, the banner shows from the start and the words come in from 100 ms. The
        /// trailer shows once the card has grown, from the first frame then. With animation effects
        /// off, a crossfade, and false.
        /// </summary>
        bool Grow(Rect cover, double x, double y, double height)
        {
            var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(card);
            var compositor = visual.Compositor;
            var fade = compositor.CreateScalarKeyFrameAnimation();
            fade.InsertKeyFrame(0, 0);
            fade.InsertKeyFrame(1, 1, Motion.FadeInEasing(compositor));
            if (!Motion.Enabled)
            {
                fade.Duration = Motion.Standard;
                visual.StartAnimation("Opacity", fade);
                return false;
            }
            // The point of the card over the cover's centre stays put while it grows.
            visual.CenterPoint = new System.Numerics.Vector3((float)(cover.X + cover.Width / 2 - x), (float)(cover.Y + cover.Height / 2 - y), 0);
            coverScale = (float)Math.Clamp(cover.Width / card.Width, 0.2, 1);
            Motion.Spring(visual, "Scale", System.Numerics.Vector3.One, new System.Numerics.Vector3(coverScale, coverScale, 1));
            fade.Duration = Motion.Quick;
            visual.StartAnimation("Opacity", fade);
            var text = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(words);
            var later = compositor.CreateScalarKeyFrameAnimation();
            later.InsertKeyFrame(0, 0);
            later.InsertKeyFrame(1, 1, Motion.FadeInEasing(compositor));
            later.DelayTime = TimeSpan.FromMilliseconds(100);
            later.DelayBehavior = Microsoft.UI.Composition.AnimationDelayBehavior.SetInitialValueBeforeDelay;
            later.Duration = Motion.Standard - TimeSpan.FromMilliseconds(100);
            text.StartAnimation("Opacity", later);
            // Banner first; the trailer, already loading under it, shows once the card has grown,
            // and only when a frame plays.
            var grown = Target.DispatcherQueue.CreateTimer();
            grown.Interval = Motion.Standard;
            grown.IsRepeating = false;
            grown.Tick += (_, _) =>
            {
                grown.Stop();
                if (!closing && popup.IsOpen)
                    stage?.Unhold();
            };
            grown.Start();
            return true;
        }

        /// <summary>The pointer is on the cover or the preview: stay open.</summary>
        public void Stay()
        {
            leave.Stop();
        }

        /// <summary>The pointer left the cover or the preview: close in 150 ms unless it comes back to either.</summary>
        public void Leaving()
        {
            leave.Stop();
            leave.Start();
        }

        /// <summary>Leaves on the quick token: the trailer stops, the card shrinks back to the cover and fades, and it is gone.</summary>
        public void Dismiss()
        {
            if (closing)
                return;
            closing = true;
            leave.Stop();
            root?.RemoveHandler(UIElement.KeyDownEvent, onKey);
            root?.RemoveHandler(UIElement.PointerWheelChangedEvent, onWheel);
            // The trailer's page goes back to the pool at once: the card closes over it.
            HoverTrailer.Closed(stage);
            if (!popup.IsOpen)
                return;
            // A trailer's video does not fade with the card and would stand alone as it went:
            // a card that played one closes at once.
            if (playing)
            {
                popup.IsOpen = false;
                return;
            }
            card.IsHitTestVisible = false;
            var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(card);
            // Back into the cover on the quick spring as it fades, as on the Mac.
            if (Motion.Enabled)
                Motion.Spring(visual, "Scale", new System.Numerics.Vector3(coverScale, coverScale, 1), quick: true);
            var fade = visual.Compositor.CreateScalarKeyFrameAnimation();
            fade.InsertKeyFrame(1, 0, visual.Compositor.CreateCubicBezierEasingFunction(new(0.42f, 0), new(1, 1)));
            fade.Duration = Motion.Quick;
            var batch = visual.Compositor.CreateScopedBatch(Microsoft.UI.Composition.CompositionBatchTypes.Animation);
            visual.StartAnimation("Opacity", fade);
            batch.End();
            batch.Completed += (_, _) => popup.IsOpen = false;
        }
    }
}
