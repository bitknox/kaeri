using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Foundation;

namespace Kaeri.Windows.Pages;

/// <summary>One leaf: its parts laid out where the engine put them in a box.</summary>
sealed class LeafView : Canvas
{
    readonly List<(PartView View, LeafPart Part)> parts = [];
    readonly Leaf leaf;

    public LeafView(ReaderSession reader, Leaf leaf, string chapter, ReaderPalette palette, string fit, int total, bool atOnce = false)
    {
        this.leaf = leaf;
        var single = leaf.Parts.Length == 1;
        foreach (var part in leaf.Parts)
        {
            var align = single ? HorizontalAlignment.Center : part.Place.X < 0.5 ? HorizontalAlignment.Right : HorizontalAlignment.Left;
            var label = T.WindowsReaderPageLabel((int)part.Page + 1, Math.Max(total, (int)part.Page + 1));
            var view = new PartView(reader, part, chapter, palette, align, single && fit == "width", single && fit == "height", label, atOnce);
            parts.Add((view, part));
            Children.Add(view);
        }
        SizeChanged += (_, _) => Place();
    }

    public Leaf Leaf => leaf;

    void Place()
    {
        foreach (var (view, part) in parts)
        {
            view.Width = Math.Max(0, part.Place.Width * ActualWidth);
            view.Height = Math.Max(0, part.Place.Height * ActualHeight);
            SetLeft(view, part.Place.X * ActualWidth);
            SetTop(view, part.Place.Y * ActualHeight);
        }
    }

    public void Refresh(double scale)
    {
        foreach (var (view, _) in parts)
            view.Refresh(scale);
    }
}

/// <summary>
/// Pages one at a time, or two as a spread. A turn slides the next leaf in on 250 ms
/// fast-out-slow-in; with animation effects off it crossfades, and Paper mode turns at once.
/// A touch or pen drag turns the way the book reads: the leaf follows the finger with its
/// neighbour beside it, and past a quarter of the width, or with a flick, both settle on the
/// standard spring; otherwise they spring back. The mouse keeps the click zones and the wheel.
/// The leaves either side stay mounted off screen so their images are decoded before the turn.
/// Pinch and Ctrl+wheel zoom; a turn or a double click puts the zoom back.
/// </summary>
sealed class PagedView : Grid
{
    readonly ReaderOverlay owner;
    readonly ScrollViewer zoom;
    readonly Grid stage = new();
    readonly Dictionary<int, FrameworkElement> mounted = [];
    /// <summary>A shown credit page, over the leaf it stands before or after.</summary>
    readonly Grid credit = new() { Visibility = Visibility.Collapsed };
    /// <summary>Two, so one credit page crossfades over the one before rather than through the ground.</summary>
    readonly PageImageView[] creditImages = [new(), new()];
    int creditFront;
    ulong? creditShown;
    int current = -1;
    string? chapter;
    Storyboard? turning;
    bool swiping;
    double swipeX, swipeVelocity;
    long lastWheel;
    double lastWheelOffset = -1;

    static readonly KeySpline FastOutSlowIn = new() { ControlPoint1 = new Point(0.4, 0), ControlPoint2 = new Point(0.2, 1) };
    static readonly TimeSpan TurnTime = TimeSpan.FromMilliseconds(250);
    const string MoveX = "(UIElement.RenderTransform).(TranslateTransform.X)";

    public PagedView(ReaderOverlay owner)
    {
        this.owner = owner;
        zoom = new ScrollViewer
        {
            ZoomMode = ZoomMode.Enabled,
            MinZoomFactor = 1,
            MaxZoomFactor = 4,
            HorizontalScrollMode = ScrollMode.Auto,
            VerticalScrollMode = ScrollMode.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollBarVisibility = ScrollBarVisibility.Hidden,
            IsTabStop = false,
            Content = stage,
        };
        stage.HorizontalAlignment = HorizontalAlignment.Center;
        // Hit-testable everywhere, so a drag can start between a spread's pages.
        stage.Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        stage.VerticalAlignment = VerticalAlignment.Center;
        Children.Add(zoom);
        foreach (var image in creditImages)
            credit.Children.Add(image);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(credit, T.ReaderCreditPageLabel);
        Children.Add(credit);
        Clip = new RectangleGeometry();
        SizeChanged += (_, e) =>
        {
            Clip = new RectangleGeometry { Rect = new Rect(0, 0, e.NewSize.Width, e.NewSize.Height) };
            Layout();
        };
        zoom.AddHandler(PointerWheelChangedEvent, new PointerEventHandler(OnWheel), true);
        zoom.DoubleTapped += (_, e) =>
        {
            if (zoom.ZoomFactor > 1.01f)
            {
                zoom.ChangeView(0, 0, 1);
                e.Handled = true;
            }
        };

        // Swipes: the scroll view keeps pinch and the vertical scroll (System), the stage takes
        // a sideways drag while the page is not zoomed or wider than the window.
        stage.ManipulationStarted += OnSwipeStarted;
        stage.ManipulationDelta += OnSwipeDelta;
        stage.ManipulationCompleted += OnSwipeCompleted;
        stage.SizeChanged += (_, _) => Rails();
        zoom.ViewChanged += (_, _) => Rails();
        Rails();
    }

    /// <summary>A zoomed page, or one wider than the window, pans sideways; otherwise a drag turns.</summary>
    void Rails()
    {
        var swipes = !Zoomed && zoom.ScrollableWidth <= 1;
        zoom.HorizontalScrollMode = swipes ? ScrollMode.Disabled : ScrollMode.Auto;
        stage.ManipulationMode = swipes ? ManipulationModes.System | ManipulationModes.TranslateX : ManipulationModes.System;
    }

    ReaderSession Reader => owner.Reader;

    public bool Zoomed => zoom.ZoomFactor > 1.01f;

    /// <summary>The leaf on screen.</summary>
    public int Current => current;

    // Building

    /// <summary>The leaves changed (a new chapter, or laid out again): build round the session's leaf.</summary>
    public void Rebuild()
    {
        StopTurn();
        foreach (var element in mounted.Values)
            stage.Children.Remove(element);
        mounted.Clear();
        chapter = Reader.View?.Chapter.Id;
        current = Reader.Leaves.Length == 0 ? -1 : Math.Clamp(Reader.Current, 0, Reader.Leaves.Length - 1);
        zoom.ChangeView(0, 0, 1, true);
        Mount();
        SyncCredit();
    }

    /// <summary>The engine's files changed: every mounted leaf shows what there is now.</summary>
    public void RefreshPages()
    {
        foreach (var element in mounted.Values)
            if (element is LeafView leaf)
                leaf.Refresh(owner.Scale);
        Layout();
    }

    /// <summary>The end page's words changed (read, next chapter, server line).</summary>
    public void RefreshEnd()
    {
        foreach (var (index, element) in mounted.ToList())
        {
            if (element is not EndPageView)
                continue;
            stage.Children.Remove(element);
            var fresh = Build(index);
            mounted[index] = fresh;
            stage.Children.Add(fresh);
            Position(index, fresh);
        }
        Layout();
    }

    FrameworkElement Build(int index)
    {
        var leaf = Reader.Leaves[index];
        FrameworkElement element = leaf.EndPage
            ? owner.BuildEndPage()
            // The first leaf after the opening plate keeps its plate up at once: no blank between them.
            : new LeafView(Reader, leaf, chapter ?? "", owner.PagePalette, owner.Fit, Reader.PageCount, index == current && owner.TakeCarry());
        element.RenderTransform = new TranslateTransform();
        if (element is LeafView view)
            view.Refresh(owner.Scale);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(element,
            index == current ? Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Content : Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return element;
    }

    /// <summary>The leaf on screen and one either side, nothing else.</summary>
    void Mount()
    {
        if (current < 0)
            return;
        var want = new HashSet<int> { current };
        if (current + 1 < Reader.Leaves.Length)
            want.Add(current + 1);
        if (current - 1 >= 0)
            want.Add(current - 1);
        foreach (var gone in mounted.Keys.Where(k => !want.Contains(k)).ToList())
        {
            stage.Children.Remove(mounted[gone]);
            mounted.Remove(gone);
        }
        foreach (var index in want)
        {
            if (!mounted.TryGetValue(index, out var element))
            {
                element = Build(index);
                mounted[index] = element;
                stage.Children.Add(element);
            }
            Position(index, element);
        }
        Layout();
    }

    /// <summary>A neighbour waits off screen on the side it comes in from.</summary>
    void Position(int index, FrameworkElement element)
    {
        var shift = index == current ? 0 : (index > current ? ForwardFrom : -ForwardFrom) * ActualWidth;
        if (element.RenderTransform is TranslateTransform move)
            move.X = shift;
        element.Opacity = 1;
        Canvas.SetZIndex(element, index == current ? 1 : 0);
        element.IsHitTestVisible = index == current;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(element,
            index == current && creditShown is null ? Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Content : Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
    }

    /// <summary>The side a forward leaf comes in from: the left in a right-to-left book.</summary>
    double ForwardFrom => owner.Rtl ? -1 : 1;

    /// <summary>The stage takes the window, or more when a page fits its width or height.</summary>
    void Layout()
    {
        double w = ActualWidth, h = ActualHeight;
        if (w <= 0 || h <= 0)
            return;
        double cw = w, ch = h;
        if (current >= 0 && current < Reader.Leaves.Length && Reader.Leaves[current] is { EndPage: false, Parts.Length: 1 } leaf
            && Reader.FileOf(leaf.Parts[0], chapter ?? "") is { } file)
        {
            var fit = owner.Fit;
            var shown = PageImageView.Fitted(file, new global::Windows.Foundation.Size(w, h), fit == "width", fit == "height");
            if (fit == "width")
                ch = Math.Max(h, shown.Height);
            else if (fit == "height")
                cw = Math.Max(w, shown.Width);
        }
        stage.Width = cw;
        stage.Height = ch;
        foreach (var image in creditImages)
        {
            image.Width = w;
            image.Height = h;
        }
        Rails();
        foreach (var element in mounted.Values)
        {
            element.Width = cw;
            element.Height = ch;
        }
        foreach (var (index, element) in mounted)
            if (turning is null)
                Position(index, element);
    }

    // Turning

    /// <summary>
    /// A turn forward or back. Forward from the end page opens the next chapter; past either end
    /// of the chapter nothing happens.
    /// </summary>
    public void Turn(bool forward, TurnHow how)
    {
        if (current < 0 || current >= Reader.Leaves.Length)
            return;
        if (owner.CreditTurn(forward))
            return;
        if (forward && Reader.Leaves[current].EndPage)
        {
            if ((Reader.End?.Next ?? Reader.View?.Next) is { } next)
                Reader.OpenChapter(next.Id);
            return;
        }
        var target = current + (forward ? 1 : -1);
        if (target < 0 || target >= Reader.Leaves.Length)
            return;
        Reader.Shown(target, how);
        Go(target, animate: true);
        owner.Turned();
    }

    /// <summary>A jump the person asked for (the scrubber, the page grid): Back to remembers where they were.</summary>
    public void Jump(int target)
    {
        if (target < 0 || target >= Reader.Leaves.Length)
            return;
        if (Reader.CreditOn is not null)
        {
            // A jump leaves a credit page, to the leaf under it too.
            Reader.LeaveCredit();
            if (target == current)
            {
                owner.Turned();
                return;
            }
        }
        if (target == current)
            return;
        Reader.Shown(target, TurnHow.Jump);
        Go(target, animate: false);
        owner.Turned();
    }

    /// <summary>The engine moved the reader (Back to, Start over, another device's place).</summary>
    public void GoTo(int target)
    {
        if (target < 0 || target >= Reader.Leaves.Length)
            return;
        Reader.LeaveCredit();
        Reader.Shown(target, TurnHow.Placed);
        if (target != current)
        {
            Go(target, animate: false);
            owner.Turned();
        }
    }

    /// <summary>From a credit page to a leaf: the leaf crossfades in as the page fades away.</summary>
    public void Land(int target)
    {
        if (target < 0 || target >= Reader.Leaves.Length)
            return;
        Reader.LeaveCredit();
        if (target == current)
            return;
        Reader.Shown(target, TurnHow.Turn);
        Go(target, animate: false);
    }

    /// <summary>
    /// The session's credit page on screen, or none: a page fades in over the leaf on the
    /// fade-in token (one credit page to the next crossfades), and fades away on the fade-out
    /// token. Paper mode cuts.
    /// </summary>
    public void SyncCredit()
    {
        var on = Reader.CreditOn;
        if (on is null)
        {
            if (creditShown is null)
                return;
            creditShown = null;
            credit.IsHitTestVisible = false;
            if (owner.PaperMode)
                Motion.Set(credit, false);
            else
                Motion.Hide(credit);
            SetLeavesSeen(true);
            return;
        }
        if (creditShown == on.Fingerprint)
            return;
        var fresh = creditShown is null;
        creditShown = on.Fingerprint;
        StopTurn();
        if (zoom.ZoomFactor > 1.01f || zoom.VerticalOffset > 0 || zoom.HorizontalOffset > 0)
            zoom.ChangeView(0, 0, 1, true);
        credit.Background = owner.StageGround;
        var back = creditImages[creditFront];
        creditFront = fresh ? creditFront : 1 - creditFront;
        var front = creditImages[creditFront];
        front.Dim = owner.PagePalette.Paper;
        // The layer or the page fades as a whole; the image itself shows as it decodes.
        front.Cut = true;
        front.FitWidth = owner.Fit == "width";
        front.Width = ActualWidth;
        front.Height = ActualHeight;
        front.Show(on.File, owner.Scale);
        Canvas.SetZIndex(front, 1);
        Canvas.SetZIndex(back, 0);
        credit.IsHitTestVisible = true;
        SetLeavesSeen(false);
        if (owner.PaperMode)
        {
            Motion.Set(credit, true);
            Motion.Set(front, true);
            back.Visibility = fresh ? Visibility.Visible : Visibility.Collapsed;
        }
        else if (fresh)
        {
            Motion.Set(front, true);
            Motion.FadeIn(credit, Motion.FadeInTime);
        }
        else
            // One credit page to the next: the new one fades in over the old, which goes after.
            Motion.FadeInThen(front, () =>
            {
                if (creditImages[creditFront] == front)
                    back.Visibility = Visibility.Collapsed;
            });
    }

    /// <summary>Narrator reads the credit page, not the leaf under it.</summary>
    void SetLeavesSeen(bool seen)
    {
        if (mounted.TryGetValue(current, out var element))
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(element,
                seen ? Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Content : Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
    }

    void Go(int target, bool animate)
    {
        StopTurn();
        var from = current;
        var forward = target > from;
        current = target;
        if (zoom.ZoomFactor > 1.01f || zoom.VerticalOffset > 0 || zoom.HorizontalOffset > 0)
            zoom.ChangeView(0, 0, 1, true);
        if (!mounted.ContainsKey(from))
        {
            Mount();
            return;
        }
        var old = mounted[from];
        if (!mounted.TryGetValue(target, out var incoming))
        {
            incoming = Build(target);
            mounted[target] = incoming;
            stage.Children.Add(incoming);
        }
        incoming.Width = old.Width;
        incoming.Height = old.Height;
        Canvas.SetZIndex(incoming, 1);
        Canvas.SetZIndex(old, 0);
        incoming.IsHitTestVisible = true;
        old.IsHitTestVisible = false;
        // Paper mode cuts; reduced motion keeps the crossfade, which moves nothing.
        if (owner.PaperMode)
        {
            Mount();
            return;
        }
        if (!animate || !Motion.Enabled)
        {
            // A jump, or animation effects off: the new leaf crossfades in over the old.
            ((TranslateTransform)incoming.RenderTransform).X = 0;
            incoming.Opacity = 0;
            turning = Board(incoming, "Opacity", 0, 1, null, Motion.Quick);
            turning.Children.Add(Animation(old, "Opacity", 1, 0, null, Motion.Quick));
            Run(target);
            return;
        }
        var width = ActualWidth;
        var sign = forward ? ForwardFrom : -ForwardFrom;
        turning = Board(incoming, MoveX, sign * width, 0, FastOutSlowIn);
        turning.Children.Add(Animation(old, MoveX, 0, -sign * width, FastOutSlowIn));
        Run(target);
    }

    void Run(int target)
    {
        var board = turning!;
        board.Completed += (_, _) =>
        {
            if (turning != board)
                return;
            turning = null;
            // Let go of the held values, so the next drag moves the leaves itself.
            board.Stop();
            if (current == target)
                Mount();
        };
        board.Begin();
    }

    static Storyboard Board(UIElement element, string property, double from, double to, KeySpline? spline, TimeSpan? time = null)
    {
        var board = new Storyboard();
        board.Children.Add(Animation(element, property, from, to, spline, time));
        return board;
    }

    static DoubleAnimationUsingKeyFrames Animation(UIElement element, string property, double from, double to, KeySpline? spline = null, TimeSpan? time = null)
    {
        var animation = new DoubleAnimationUsingKeyFrames();
        animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = time ?? TurnTime, Value = to, KeySpline = spline ?? FastOutSlowIn });
        Storyboard.SetTarget(animation, element);
        Storyboard.SetTargetProperty(animation, property);
        return animation;
    }

    void StopTurn()
    {
        if (turning is not { } board)
            return;
        turning = null;
        board.Stop();
        Mount();
    }

    // Swiping

    /// <summary>Whether a drag of <paramref name="x"/> goes forward: right in a right-to-left book, left otherwise.</summary>
    bool Forward(double x) => x * -ForwardFrom > 0;

    /// <summary>Whether a drag that way has somewhere to go: a leaf, or the next chapter past the end page.</summary>
    bool CanGo(bool forward)
    {
        if (forward && Reader.Leaves[current].EndPage)
            return (Reader.End?.Next ?? Reader.View?.Next) is not null;
        var target = current + (forward ? 1 : -1);
        return target >= 0 && target < Reader.Leaves.Length;
    }

    /// <summary>A finger or a pen starts a drag; the mouse keeps its click zones.</summary>
    void OnSwipeStarted(object sender, ManipulationStartedRoutedEventArgs e)
    {
        if (e.PointerDeviceType == PointerDeviceType.Mouse || Zoomed || current < 0 || current >= Reader.Leaves.Length)
        {
            e.Complete();
            return;
        }
        // A turn under way lands first, so the drag starts from a still page.
        StopTurn();
        swiping = true;
        swipeX = 0;
        swipeVelocity = 0;
        e.Handled = true;
    }

    void OnSwipeDelta(object sender, ManipulationDeltaRoutedEventArgs e)
    {
        if (!swiping)
            return;
        e.Handled = true;
        var x = e.Cumulative.Translation.X;
        swipeX = x;
        swipeVelocity = e.Velocities.Linear.X * 1000;
        // Paper mode and reduced motion: nothing follows the finger; the turn comes on release.
        // A credit page over the leaves stays still too, and crossfades on release.
        if (owner.PaperMode || !Motion.Enabled || Reader.CreditOn is not null)
            return;
        // Past either end the page gives a quarter of the way, and comes back.
        if (!CanGo(Forward(x)))
            x /= 4;
        Follow(x);
    }

    void OnSwipeCompleted(object sender, ManipulationCompletedRoutedEventArgs e)
    {
        if (!swiping)
            return;
        swiping = false;
        e.Handled = true;
        if (current < 0 || current >= Reader.Leaves.Length)
            return;
        var x = swipeX;
        var forward = Forward(x);
        var flick = Math.Abs(swipeVelocity) > 450 && swipeVelocity * x > 0;
        var commit = Math.Abs(x) > Math.Max(ActualWidth, 1) * 0.25 || flick;
        if (commit && (Reader.CreditOn is not null || owner.CreditTurnWaits(forward)))
        {
            // Onto, between or off credit pages: the leaves go back and the pages crossfade.
            Mount();
            Turn(forward, TurnHow.Swipe);
            return;
        }
        if (!commit || !CanGo(forward))
        {
            SpringBack();
            return;
        }
        if (Reader.Leaves[current].EndPage && forward)
        {
            // One more swipe past the end page opens the next chapter.
            Mount();
            Turn(true, TurnHow.Swipe);
            return;
        }
        if (owner.PaperMode || !Motion.Enabled)
        {
            // Nothing followed the finger: the turn is the keys' (at once on paper, a crossfade otherwise).
            Turn(forward, TurnHow.Swipe);
            return;
        }
        var target = current + (forward ? 1 : -1);
        Reader.Shown(target, TurnHow.Swipe);
        Settle(target);
        owner.Turned();
    }

    /// <summary>The leaf on screen at <paramref name="x"/>, each neighbour a width away on its side.</summary>
    void Follow(double x)
    {
        var width = ActualWidth;
        foreach (var (index, element) in mounted)
            if (element.RenderTransform is TranslateTransform move)
                move.X = x + (index == current ? 0 : (index > current ? ForwardFrom : -ForwardFrom) * width);
    }

    /// <summary>A drag let go past the line: the two leaves finish the way on the standard spring.</summary>
    void Settle(int target)
    {
        var from = current;
        if (!mounted.TryGetValue(from, out var old) || !mounted.TryGetValue(target, out var incoming)
            || old.RenderTransform is not TranslateTransform oldMove || incoming.RenderTransform is not TranslateTransform inMove)
        {
            current = target;
            Mount();
            return;
        }
        current = target;
        Canvas.SetZIndex(incoming, 1);
        Canvas.SetZIndex(old, 0);
        incoming.IsHitTestVisible = true;
        old.IsHitTestVisible = false;
        // The old leaf leaves on the side opposite the one the new leaf waited on.
        var side = target > from ? ForwardFrom : -ForwardFrom;
        turning = Board(incoming, MoveX, inMove.X, 0, Motion.SpringSpline(), Motion.Standard);
        turning.Children.Add(Animation(old, MoveX, oldMove.X, -side * ActualWidth, Motion.SpringSpline(), Motion.Standard));
        Run(target);
    }

    /// <summary>A drag let go short of the line: everything goes back where it was.</summary>
    void SpringBack()
    {
        if (owner.PaperMode || !Motion.Enabled || mounted.Count == 0)
        {
            Mount();
            return;
        }
        var width = ActualWidth;
        Storyboard? board = null;
        foreach (var (index, element) in mounted)
        {
            if (element.RenderTransform is not TranslateTransform move)
                continue;
            var home = index == current ? 0 : (index > current ? ForwardFrom : -ForwardFrom) * width;
            var animation = Animation(element, MoveX, move.X, home, Motion.SpringSpline(), Motion.Standard);
            board ??= new Storyboard();
            board.Children.Add(animation);
        }
        if (board is null)
        {
            Mount();
            return;
        }
        turning = board;
        Run(current);
    }

    // Input

    /// <summary>
    /// The wheel turns the page, a notch at a time; a page taller than the window scrolls first
    /// and turns at its foot. Ctrl+wheel is the scroll view's zoom.
    /// </summary>
    void OnWheel(object sender, PointerRoutedEventArgs e)
    {
        var point = e.GetCurrentPoint(zoom);
        if ((e.KeyModifiers & global::Windows.System.VirtualKeyModifiers.Control) != 0)
            return;
        var delta = point.Properties.MouseWheelDelta;
        if (delta == 0 || point.Properties.IsHorizontalMouseWheel)
            return;
        var down = delta < 0;
        var scrollable = zoom.ScrollableHeight > 1;
        if (scrollable)
        {
            var atEdge = down ? zoom.VerticalOffset >= zoom.ScrollableHeight - 1 : zoom.VerticalOffset <= 1;
            // Only a notch that finds the page already at its edge turns: the one that got it there doesn't.
            var stayed = Math.Abs(lastWheelOffset - zoom.VerticalOffset) < 1;
            lastWheelOffset = zoom.VerticalOffset;
            if (!atEdge || !stayed)
                return;
        }
        else if (Zoomed)
            return;
        var now = Environment.TickCount64;
        if (now - lastWheel < 280)
        {
            e.Handled = true;
            return;
        }
        lastWheel = now;
        lastWheelOffset = -1;
        e.Handled = true;
        Turn(down, TurnHow.Scroll);
    }

    /// <summary>A tap or click, as a fraction of the width, from the overlay.</summary>
    public void Tap(double x)
    {
        var layout = owner.Reader.Choice(ReaderSettingKey.TapLayout) ?? "edges";
        if (owner.ChromeShown || layout == "off")
        {
            owner.ToggleChrome();
            return;
        }
        // Edges: the outer thirds turn. Sides: the outer 40 percent, and the middle still brings the chrome up.
        var edge = layout == "sides" ? 0.4 : 0.33;
        if (x < edge || x > 1 - edge)
        {
            var leftSide = x < 0.5;
            Turn(leftSide == owner.Rtl, TurnHow.Turn);
        }
        else
            owner.ToggleChrome();
    }

    /// <summary>Home and End: the first leaf and the last.</summary>
    public void Ends(bool last)
    {
        if (Reader.Leaves.Length == 0)
            return;
        var target = last ? Math.Max(0, Reader.ReadingLeaves - 1) : 0;
        Jump(target);
    }

    /// <summary>The scroll view takes the keyboard's arrows when zoomed; otherwise they turn.</summary>
    public bool TakesArrows => Zoomed;

    /// <summary>Ctrl+plus and Ctrl+minus step the zoom.</summary>
    public void ZoomBy(double factor)
    {
        var to = (float)Math.Clamp(zoom.ZoomFactor * factor, 1, 4);
        zoom.ChangeView(null, null, to);
    }

    public void ResetZoom() => zoom.ChangeView(0, 0, 1);
}
