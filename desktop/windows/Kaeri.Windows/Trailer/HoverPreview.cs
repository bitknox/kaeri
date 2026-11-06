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
/// The pointer must rest: less than 4 px of movement for 450 ms, or for 150 ms when a preview
/// closed less than a second ago (scanning a row). The preview closes 150 ms after
/// the pointer leaves both the cover and the preview, and at once on a scroll, a key press, a
/// click elsewhere or the window losing focus. Keyboard focus never opens one.
/// </summary>
static class HoverPreview
{
    static readonly TimeSpan Dwell = TimeSpan.FromMilliseconds(450);
    /// <summary>The dwell for the next cover when a preview closed less than <see cref="Scanning"/> ago.</summary>
    static readonly TimeSpan NextDwell = TimeSpan.FromMilliseconds(150);
    static readonly TimeSpan Scanning = TimeSpan.FromSeconds(1);
    static DateTime closedAt = DateTime.MinValue;
    static readonly TimeSpan Leave = TimeSpan.FromMilliseconds(150);
    /// <summary>The pointer may drift this far and still be resting.</summary>
    const double Rest = 4;
    /// <summary>The video's width: 384 by 216, over YouTube's 356 by 200 floor.</summary>
    internal const double Width = 384;

    /// <summary>
    /// How long after the wheel no preview starts. Covers pass under a still pointer while the page
    /// scrolls, and each would otherwise open a preview at the next pause, for the next notch to close.
    /// </summary>
    static readonly TimeSpan Scrolled = TimeSpan.FromMilliseconds(700);
    static DateTime wheeledAt = DateTime.MinValue;
    static UIElement? heard;

    static readonly ConditionalWeakTable<FrameworkElement, Resting> targets = [];
    static Open? open;

    /// <summary>What one cover keeps while the pointer is on it.</summary>
    sealed class Resting
    {
        public Point Anchor;
        public DispatcherQueueTimer? Timer;
        public Func<uint> Id = () => 0;
    }

    /// <summary>Lets <paramref name="target"/> open a preview of title <paramref name="id"/> when the pointer rests on it.</summary>
    public static void Attach(FrameworkElement target, uint id) => Attach(target, () => id);

    /// <summary>As above, for a container that may be given another title: <paramref name="id"/> is asked when the preview opens.</summary>
    public static void Attach(FrameworkElement target, Func<uint> id)
    {
        var state = targets.GetOrCreateValue(target);
        state.Id = id;
        // A container reused for another title keeps its handlers, and takes the new id.
        if (state.Timer is not null)
            return;
        // The timer reaches its cover weakly: a handler holding it would keep the page alive.
        state.Timer = Timers.Weak(target, Dwell, false, static at =>
        {
            if (targets.TryGetValue(at, out var resting))
                Show(at, resting.Id());
        });
        target.Loaded += (_, _) => HearWheel(target);
        // Heard even when the target has handled them: a grid's item container and a button
        // take their own pointer events for their hover and press states, and a plain handler
        // never heard them, so most covers never opened a card.
        target.AddHandler(UIElement.PointerEnteredEvent, new PointerEventHandler((_, e) => Resume(target, state, e)), true);
        target.AddHandler(UIElement.PointerMovedEvent, new PointerEventHandler((_, e) =>
        {
            var at = e.GetCurrentPoint(target).Position;
            if (Math.Abs(at.X - state.Anchor.X) >= Rest || Math.Abs(at.Y - state.Anchor.Y) >= Rest)
                Resume(target, state, e);
        }), true);
        target.AddHandler(UIElement.PointerExitedEvent, new PointerEventHandler((_, _) =>
        {
            state.Timer.Stop();
            if (open is { } o && o.Target == target)
                o.Leaving();
        }), true);
        // A press on the cover opens the title: no preview then.
        target.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) =>
        {
            state.Timer.Stop();
            Close();
        }), true);
        target.Unloaded += (_, _) => state.Timer.Stop();
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

    static void OnWheel(object sender, PointerRoutedEventArgs e) => wheeledAt = DateTime.UtcNow;

    /// <summary>The pointer is at a new resting place: wait out the dwell again.</summary>
    static void Resume(FrameworkElement target, Resting state, PointerRoutedEventArgs e)
    {
        state.Anchor = e.GetCurrentPoint(target).Position;
        state.Timer!.Stop();
        // The page is scrolling under the pointer: a cover arriving there is not one pointed at.
        // The pointer moving once the page is still starts the dwell, as PointerMoved calls here.
        if (DateTime.UtcNow - wheeledAt < Scrolled)
            return;
        state.Timer.Interval = DateTime.UtcNow - closedAt < Scanning ? NextDwell : Dwell;
        // A touch has no hover; a pen or a mouse does.
        if (e.Pointer.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Touch)
            return;
        if (open is { } o && o.Target == target)
        {
            o.Stay();
            return;
        }
        state.Timer.Start();
    }

    /// <summary>Close the preview on screen, if there is one.</summary>
    public static void Close()
    {
        var was = open;
        open = null;
        if (was is not null)
            closedAt = DateTime.UtcNow;
        was?.Dismiss();
    }

    /// <summary>Open a preview of the title now, for a screenshot or a key.</summary>
    public static void Show(FrameworkElement target, uint id)
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
        var preview = new Open(ctx, target, entry);
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
        bool closing;
        readonly TrailerStage? stage;
        readonly DispatcherQueueTimer leave;
        readonly Button sound = new();
        bool soundOn;

        public FrameworkElement Target { get; }

        public Open(Ctx ctx, FrameworkElement target, Entry entry)
        {
            this.ctx = ctx;
            Target = target;
            // The video is flush at the top of the flyout, clipped only by the flyout's own corners. The
            // art lies over it and holds its place until a frame plays.
            var video = new Grid { Width = Width, Height = Width * 9 / 16 };
            var banner = new Image { Stretch = Stretch.UniformToFill };
            var art = new Grid { Background = (Brush)Application.Current.Resources["RaisedBrush"] };
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
                stage?.Stop();
                if (open == this)
                {
                    open = null;
                    closedAt = DateTime.UtcNow;
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

            if (TrailerHub.Available && Trailers.Embeddable(entry))
            {
                stage = new TrailerStage(video, EmbedStyle.Banner, art, Trailers.SubtitleLanguage(ctx.Known));
                stage.Size(video.Width, video.Height);
                stage.Showing += shown =>
                {
                    soundOn = false;
                    sound.IsHitTestVisible = shown;
                    if (shown)
                    {
                        (sound.Content as FontIcon)!.Glyph = "\uE74F";
                        Motion.Fade(sound, 1);
                    }
                    else
                        Motion.Fade(sound, 0);
                };
                sound.Click += (_, _) =>
                {
                    soundOn = !soundOn;
                    stage.SetSound(soundOn);
                    (sound.Content as FontIcon)!.Glyph = soundOn ? "\uE767" : "\uE74F";
                };
            }
            entryToPlay = entry;
        }

        readonly Entry entryToPlay;

        UIElement? root;
        readonly KeyEventHandler onKey = (_, _) => Close();
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
            Grow(cover, x, y, height);
        }

        /// <summary>
        /// From the cover to the card on the standard token: the card scales up from where the
        /// cover is, the banner shows from the start and the words come in from 100 ms. The
        /// trailer starts once the card has grown. With animation effects off, a crossfade.
        /// </summary>
        void Grow(Rect cover, double x, double y, double height)
        {
            var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(card);
            var compositor = visual.Compositor;
            var fade = compositor.CreateScalarKeyFrameAnimation();
            fade.InsertKeyFrame(0, 0);
            fade.InsertKeyFrame(1, 1);
            if (!Motion.Enabled)
            {
                fade.Duration = Motion.Standard;
                visual.StartAnimation("Opacity", fade);
                return;
            }
            var ease = compositor.CreateCubicBezierEasingFunction(new System.Numerics.Vector2(0.2f, 0), new System.Numerics.Vector2(0, 1));
            // The point of the card over the cover's centre stays put while it grows.
            visual.CenterPoint = new System.Numerics.Vector3((float)(cover.X + cover.Width / 2 - x), (float)(cover.Y + cover.Height / 2 - y), 0);
            var from = (float)Math.Clamp(cover.Width / card.Width, 0.2, 1);
            var scale = compositor.CreateVector3KeyFrameAnimation();
            scale.InsertKeyFrame(0, new System.Numerics.Vector3(from, from, 1));
            scale.InsertKeyFrame(1, System.Numerics.Vector3.One, ease);
            scale.Duration = Motion.Standard;
            fade.Duration = Motion.Quick;
            visual.StartAnimation("Scale", scale);
            visual.StartAnimation("Opacity", fade);
            var text = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(words);
            var later = compositor.CreateScalarKeyFrameAnimation();
            later.InsertKeyFrame(0, 0);
            later.InsertKeyFrame(1, 1);
            later.DelayTime = TimeSpan.FromMilliseconds(100);
            later.DelayBehavior = Microsoft.UI.Composition.AnimationDelayBehavior.SetInitialValueBeforeDelay;
            later.Duration = Motion.Standard - TimeSpan.FromMilliseconds(100);
            text.StartAnimation("Opacity", later);
            // Banner first; the trailer loads under it once the card has grown, and fades in
            // only when a frame plays.
            var grown = Target.DispatcherQueue.CreateTimer();
            grown.Interval = Motion.Standard;
            grown.IsRepeating = false;
            grown.Tick += (_, _) =>
            {
                grown.Stop();
                if (!closing && popup.IsOpen && stage is not null && Trailers.MayAutoplay(ctx.Known, entryToPlay))
                    stage.Start(entryToPlay);
            };
            grown.Start();
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

        /// <summary>Leaves on the quick token: the trailer stops, the card fades, and it is gone.</summary>
        public void Dismiss()
        {
            if (closing)
                return;
            closing = true;
            leave.Stop();
            root?.RemoveHandler(UIElement.KeyDownEvent, onKey);
            root?.RemoveHandler(UIElement.PointerWheelChangedEvent, onWheel);
            stage?.Stop();
            if (!popup.IsOpen)
                return;
            card.IsHitTestVisible = false;
            var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(card);
            var fade = visual.Compositor.CreateScalarKeyFrameAnimation();
            fade.InsertKeyFrame(1, 0);
            fade.Duration = Motion.Quick;
            var batch = visual.Compositor.CreateScopedBatch(Microsoft.UI.Composition.CompositionBatchTypes.Animation);
            visual.StartAnimation("Opacity", fade);
            batch.End();
            batch.Completed += (_, _) => popup.IsOpen = false;
        }
    }
}
