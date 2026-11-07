using Medium = Kaeri.Engine.Core.Medium;
using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;
using Windows.System;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;
using DispatcherQueuePriority = Microsoft.UI.Dispatching.DispatcherQueuePriority;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The reader, over the whole window: manga and webtoons as pages or a strip, light novels in a
/// web view, with the chrome that comes and goes and the screens for opening, failing and the
/// end of a chapter. It drives <see cref="ReaderSession"/> and draws what it says; the window
/// places it (<c>ReaderLayer</c>) and calls <see cref="Open"/> and <see cref="Close"/>.
/// </summary>
public sealed partial class ReaderOverlay : UserControl
{
    enum Kind { None, Paged, Strip, Novel }

    readonly Grid root = new();
    readonly Grid stage = new();
    readonly Grid states = new();
    readonly TextBlock announcer = new() { Opacity = 0, IsHitTestVisible = false, Width = 1, Height = 1, HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top };
    readonly ReaderChrome chrome;
    readonly Flyout settingsFlyout = new() { Placement = FlyoutPlacementMode.Top };
    readonly Flyout chaptersFlyout = new() { Placement = FlyoutPlacementMode.Top };
    readonly DispatcherQueueTimer idle;

    Kind kind;
    PagedView? paged;
    StripView? strip;
    NovelView? novel;
    FrameworkElement? stateView;
    string? stateKey;
    bool showing;
    bool chromeShown = true;
    bool pointerHidden;
    bool overChrome;
    bool flyoutOpen;
    Point lastPointer = new(-1, -1);
    long lastNovelWheel;
    string? lastFit, lastBackground;
    bool lastPaper;
    string? work;

    public ReaderOverlay()
    {
        chrome = new ReaderChrome(this);
        root.Children.Add(stage);
        root.Children.Add(states);
        root.Children.Add(chrome.Notice);
        root.Children.Add(chrome.BackTo);
        root.Children.Add(chrome.Group);
        root.Children.Add(chrome.PreviewLayer);
        root.Children.Add(chrome.Top);
        root.Children.Add(announcer);
        AutomationProperties.SetLiveSetting(announcer, AutomationLiveSetting.Polite);
        Content = root;
        IsTabStop = true;
        UseSystemFocusVisuals = false;
        RequestedTheme = ElementTheme.Dark;

        chrome.Settings.Flyout = settingsFlyout;
        chrome.Chapters.Flyout = chaptersFlyout;
        settingsFlyout.Opening += (_, _) =>
        {
            settingsFlyout.Content = SettingsFlyout.Build(this, settingsFlyout);
            FlyoutTheme(settingsFlyout);
        };
        chaptersFlyout.Opening += (_, _) =>
        {
            chaptersFlyout.Content = ChaptersFlyout.Build(this, chaptersFlyout);
            FlyoutTheme(chaptersFlyout);
        };
        foreach (var flyout in new[] { settingsFlyout, chaptersFlyout })
        {
            flyout.Opened += (_, _) => flyoutOpen = true;
            flyout.Closed += (_, _) =>
            {
                flyoutOpen = false;
                Idle();
            };
        }

        idle = Timers.Weak(this, TimeSpan.FromSeconds(2), false, o => o.IdleTick());
        stage.Tapped += OnTapped;
        AddHandler(PointerMovedEvent, new PointerEventHandler(OnPointerMoved), true);
        AddHandler(PointerPressedEvent, new PointerEventHandler((_, e) =>
        {
            // The mouse's back button leaves, as everywhere in the app.
            if (e.GetCurrentPoint(this).Properties.IsXButton1Pressed)
            {
                Close();
                e.Handled = true;
            }
        }), true);
        PreviewKeyDown += OnKey;
        SizeChanged += (_, _) =>
        {
            chrome.Fit(ActualWidth);
            ReportSurface();
        };
        Loaded += (_, _) =>
        {
            if (XamlRoot is { } xr)
                xr.Changed += (_, _) => ReportSurface();
        };
    }

    Ctx Ctx => Ctx.Current;

    internal ReaderSession Reader => Ctx.Known.Reader;

    /// <summary>The reader is up over the window.</summary>
    public bool Showing => showing;

    /// <summary>The top row's title, which drags the window while the reader is up.</summary>
    public UIElement DragArea => chrome.DragArea;

    // Opening and closing

    /// <summary>Opens <paramref name="work"/> at <paramref name="chapter"/>, or where the person stopped.</summary>
    public void Open(string work, string? chapter, OpenAt? at)
    {
        this.work = work;
        if (!showing)
        {
            showing = true;
            Reader.Changed += OnChanged;
            Visibility = Visibility.Visible;
            Ctx.Window.ReaderShowing(true);
            if (Motion.Enabled)
                Motion.FadeIn(this);
        }
        Clear();
        // The end page's server line needs Your sources to know a send is under way.
        if (Ctx.Known.Books.View is null)
            Ctx.Known.Books.Load();
        Reader.Open(work, chapter, at);
        Repaint(force: true);
        chrome.FullscreenShown(Ctx.Window.ReaderFullscreen);
        ShowChrome(stay: true);
        Render();
        ReportSurface();
        DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => Focus(FocusState.Programmatic));
    }

    /// <summary>Closes the reader and gives the window back.</summary>
    public void Close()
    {
        if (!showing)
            return;
        showing = false;
        Reader.Changed -= OnChanged;
        settingsFlyout.Hide();
        chaptersFlyout.Hide();
        Reader.Close();
        idle.Stop();
        ShowPointer();
        Clear();
        chrome.ShowNotice(null);
        chrome.ShowBackTo(null);
        Visibility = Visibility.Collapsed;
        Ctx.Window.ReaderShowing(false);
    }

    void Clear()
    {
        novel?.Close();
        stage.Children.Clear();
        paged = null;
        strip = null;
        novel = null;
        kind = Kind.None;
        ClearState();
        lastFit = null;
    }

    // What the session says

    void OnChanged(ReaderChange change)
    {
        if (!DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(() => OnChanged(change));
            return;
        }
        if (!showing)
            return;
        switch (change)
        {
            case ReaderChange.Phase:
            case ReaderChange.Opening:
                Render();
                break;
            case ReaderChange.Leaves:
                Render();
                if (kind == Kind.Paged)
                    paged?.Rebuild();
                else if (kind == Kind.Strip)
                    strip?.Rebuild();
                PositionChanged();
                break;
            case ReaderChange.Page:
                paged?.RefreshPages();
                strip?.RefreshPages();
                chrome.RefreshPreview();
                break;
            case ReaderChange.Novel:
                Render();
                if (Reader.Novel is { } book && novel is not null)
                {
                    novel.Ground(NovelGround);
                    novel.Open(book);
                }
                PositionChanged();
                break;
            case ReaderChange.NovelStyle:
                novel?.ApplyStyle(Reader.NovelStyle);
                Repaint();
                break;
            case ReaderChange.End:
            case ReaderChange.Read:
            case ReaderChange.Sent:
            case ReaderChange.Preview:
                paged?.RefreshEnd();
                strip?.RefreshFoot();
                novel?.RefreshEnd();
                break;
            case ReaderChange.Settings:
                Repaint();
                ReportSurface();
                break;
            case ReaderChange.Notice:
                chrome.ShowNotice(Reader.Notice);
                break;
            case ReaderChange.BackTo:
                chrome.ShowBackTo(Reader.BackTo);
                break;
            case ReaderChange.GoTo:
                if (Reader.GoTo is { } go)
                {
                    if (kind == Kind.Paged)
                        paged?.GoTo(go.Leaf);
                    else if (kind == Kind.Strip)
                        strip?.Restored();
                }
                break;
            case ReaderChange.NovelGoTo:
                if (Reader.NovelGoTo is { } place)
                    novel?.GoTo(place.Pos);
                break;
            case ReaderChange.Chapters:
                break;
        }
    }

    /// <summary>The view for what is open, and the opening or failed screen over it.</summary>
    void Render()
    {
        var view = Reader.View;
        var want = view is null ? Kind.None
            : view.Medium == Medium.Novel ? Kind.Novel
            : view.Mode is ReaderMode.Strip or ReaderMode.Vertical ? Kind.Strip
            : Kind.Paged;
        if (want != kind)
        {
            novel?.Close();
            stage.Children.Clear();
            paged = null;
            strip = null;
            novel = null;
            kind = want;
            FrameworkElement? made = want switch
            {
                Kind.Paged => paged = new PagedView(this),
                Kind.Strip => strip = new StripView(this),
                Kind.Novel => novel = new NovelView(this),
                _ => null,
            };
            if (made is not null)
                stage.Children.Add(made);
            if (want == Kind.Novel && Reader.Novel is { } book)
                novel!.Open(book);
            paged?.Rebuild();
            strip?.Rebuild();
            Repaint(force: true);
        }
        if (view is not null)
            chrome.Titles(view.Title, view.DemoCredit is { } demo ? T.ReaderDemoCredit(demo.Series, demo.Author, demo.Licence ?? "") : null);
        AutomationProperties.SetName(this, view?.Title ?? "");

        var hasContent = Reader.Leaves.Length > 0 || (Reader.Novel is not null && Reader.NovelPainted);
        if (Reader.Phase == ReaderPhase.Failed)
            ShowState("failed", () => new ChapterFailedView(this, ChapterLabel(), Reader.Failure));
        else if (Reader.Phase == ReaderPhase.Opening || (Reader.Phase == ReaderPhase.Open && !hasContent))
            ShowState("opening:" + ChapterLabel(), () => new OpeningView(Ctx, Reader, Palette, work ?? Reader.View?.Work ?? "", ChapterLabel()));
        else
            ClearState();
    }

    string ChapterLabel()
    {
        if (Reader.Opening?.Chapter is { } opening)
            return opening.Label;
        if (Reader.Phase == ReaderPhase.Opening && Reader.Asked?.Chapter is { } id)
            foreach (var group in Reader.Chapters)
                foreach (var row in group.Chapters)
                    if (row.Id == id)
                        return row.Label;
        return Reader.View?.Chapter.Label ?? "";
    }

    void ShowState(string key, Func<FrameworkElement> make)
    {
        if (stateKey == key && stateView is not null)
            return;
        stateKey = key;
        states.Children.Clear();
        stateView = make();
        states.Children.Add(stateView);
        states.Visibility = Visibility.Visible;
        // Over a screen with buttons the chrome's Back stays and the group waits.
        idle.Stop();
        ShowPointer();
        Motion.Set(chrome.Top, true);
        chromeShown = true;
        Motion.Set(chrome.Group, false);
        Ctx.Window.ReaderChrome(true);
    }

    void ClearState()
    {
        if (stateView is null)
            return;
        stateKey = null;
        stateView = null;
        states.Children.Clear();
        states.Visibility = Visibility.Collapsed;
        if (showing)
            ShowChrome();
    }

    bool StateShown => stateView is not null;

    /// <summary>The novel's web view could not start (no WebView2 runtime): the failed screen.</summary>
    internal void NovelFailed() => ShowState("failed", () => new ChapterFailedView(this, ChapterLabel(), null));

    internal void NovelPaintedNow() => ClearState();

    // The surface

    /// <summary>The reader's size in pixels, for the engine's layout of leaves and novels.</summary>
    void ReportSurface()
    {
        if (!showing || ActualWidth <= 0 || ActualHeight <= 0)
            return;
        var scale = XamlRoot?.RasterizationScale ?? 1;
        var two = ActualWidth > ActualHeight && ActualWidth >= 700;
        Reader.SurfaceChanged(new ReaderSurface(
            (uint)Math.Round(ActualWidth * scale), (uint)Math.Round(ActualHeight * scale), (float)scale,
            false, two, !PaperMode));
    }

    internal double Scale => XamlRoot?.RasterizationScale ?? 1;

    // Looks

    internal ReaderPalette Palette { get; private set; } = ReaderPalette.Dark;

    internal bool Rtl => kind == Kind.Novel ? novel?.Rtl ?? false : Reader.View?.Direction == ReadDirection.RightToLeft;

    internal string Fit => Reader.Choice(ReaderSettingKey.Fit) ?? "screen";

    /// <summary>
    /// Paper mode, the engine's setting: it is offered only on the screens the engine names
    /// (handhelds), and stays on the device as the engine keeps it. Elsewhere it is off, and the
    /// Paper look is the reader's paper.
    /// </summary>
    internal bool PaperMode => Reader.Row(ReaderSettingKey.PaperMode) is not null && Reader.IsOn(ReaderSettingKey.PaperMode);

    internal void SetPaperMode(bool on)
    {
        if (Reader.Row(ReaderSettingKey.PaperMode) is not null)
            Reader.Set(ReaderSettingKey.PaperMode, new ReaderValue.Toggle(on));
    }

    internal global::Windows.UI.Color NovelGround => ReaderPalette.NovelGround(Reader.Choice(ReaderSettingKey.Look), PaperMode);

    /// <summary>The colours, the backdrop and the fit, after a setting changed: what changed is built again.</summary>
    void Repaint(bool force = false)
    {
        var paper = PaperMode;
        var fit = Fit;
        var background = Reader.Choice(ReaderSettingKey.Background);
        var paletteChanged = paper != lastPaper || force;
        Palette = paper ? ReaderPalette.PaperSet : ReaderPalette.Dark;
        root.Background = new SolidColorBrush(kind == Kind.Novel ? NovelGround : Palette.Backdrop(background));
        if (paletteChanged)
        {
            chrome.Paint(Palette);
            // Paper's flyouts and controls are light; the reader is dark otherwise.
            RequestedTheme = paper ? ElementTheme.Light : ElementTheme.Dark;
        }
        novel?.Ground(NovelGround);
        if (!force && (paletteChanged || fit != lastFit || background != lastBackground))
        {
            paged?.Rebuild();
            strip?.Rebuild();
            novel?.RefreshEnd();
        }
        else if (!force && strip is not null)
            strip.RefreshPages();
        lastPaper = paper;
        lastFit = fit;
        lastBackground = background;
        PositionChanged();
    }

    void FlyoutTheme(Flyout flyout)
    {
        var style = new Style(typeof(FlyoutPresenter));
        style.Setters.Add(new Setter(Control.BackgroundProperty, new SolidColorBrush(ReaderPalette.WithAlpha(Palette.Raised, 0.98))));
        style.Setters.Add(new Setter(Control.CornerRadiusProperty, new CornerRadius(8)));
        style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(12)));
        style.Setters.Add(new Setter(FrameworkElement.MaxWidthProperty, 420.0));
        style.Setters.Add(new Setter(FrameworkElement.MaxHeightProperty, 620.0));
        style.Setters.Add(new Setter(FrameworkElement.RequestedThemeProperty, PaperMode ? ElementTheme.Light : ElementTheme.Dark));
        flyout.FlyoutPresenterStyle = style;
    }

    // The chrome

    internal bool ChromeShown => chromeShown;

    /// <summary>The group and the title bar come up; after 2 s without use they go again.</summary>
    void ShowChrome(bool stay = false)
    {
        ShowPointer();
        if (!chromeShown)
        {
            chromeShown = true;
            Appear(chrome.Top, true);
            Ctx.Window.ReaderChrome(true);
        }
        if (!StateShown)
            Appear(chrome.Group, true);
        if (!stay)
            Idle();
        else
            idle.Stop();
    }

    /// <summary>Reading: the chrome and the pointer go.</summary>
    internal void HideChrome()
    {
        idle.Stop();
        if (StateShown || flyoutOpen || chrome.Holds(XamlRoot) || DeviceWatch.ScreenReaderRunning())
            return;
        if (chromeShown)
        {
            chromeShown = false;
            Appear(chrome.Top, false);
            Appear(chrome.Group, false);
            Ctx.Window.ReaderChrome(false);
        }
        HidePointer();
    }

    internal void ToggleChrome()
    {
        if (chromeShown && !StateShown)
            HideChrome();
        else
            ShowChrome();
    }

    void Appear(UIElement element, bool shown)
    {
        if (Motion.Enabled && !PaperMode)
        {
            if (shown)
                Motion.Show(element);
            else
                Motion.Hide(element);
        }
        else
            Motion.Set(element, shown);
    }

    void Idle()
    {
        idle.Stop();
        if (chromeShown && !StateShown)
            idle.Start();
    }

    void IdleTick()
    {
        if (overChrome || flyoutOpen || chrome.Holds(XamlRoot))
        {
            Idle();
            return;
        }
        HideChrome();
    }

    void OnPointerMoved(object sender, PointerRoutedEventArgs e)
    {
        var at = e.GetCurrentPoint(this).Position;
        if (Math.Abs(at.X - lastPointer.X) < 2 && Math.Abs(at.Y - lastPointer.Y) < 2)
            return;
        lastPointer = at;
        overChrome = Within(chrome.Group, e) || Within(chrome.Top, e);
        PointerActive();
    }

    static bool Within(FrameworkElement element, PointerRoutedEventArgs e)
    {
        if (element.Visibility != Visibility.Visible)
            return false;
        var p = e.GetCurrentPoint(element).Position;
        return p.X >= 0 && p.Y >= 0 && p.X <= element.ActualWidth && p.Y <= element.ActualHeight;
    }

    /// <summary>The pointer moved, here or inside the novel's page.</summary>
    internal void PointerActive()
    {
        if (!showing)
            return;
        ShowChrome();
    }

    [LibraryImport("user32.dll")]
    private static partial int ShowCursor([MarshalAs(UnmanagedType.Bool)] bool show);

    /// <summary>The pointer goes with the chrome, but never over a screen with buttons to reach.</summary>
    void HidePointer()
    {
        if (pointerHidden || !showing || StateShown || OnEndPage)
            return;
        pointerHidden = true;
        while (ShowCursor(false) >= 0)
        {
            // The count falls below zero and the pointer is gone.
        }
    }

    void ShowPointer()
    {
        if (!pointerHidden)
            return;
        pointerHidden = false;
        while (ShowCursor(true) < 0)
        {
            // Back to zero or above: the pointer is shown again.
        }
    }

    bool OnEndPage => kind switch
    {
        Kind.Paged => paged is { Current: >= 0 } p && p.Current < Reader.Leaves.Length && Reader.Leaves[p.Current].EndPage,
        Kind.Novel => novel?.EndShown ?? false,
        _ => false,
    };

    /// <summary>A turn or a jump happened: the chrome goes and a screen reader hears the page.</summary>
    internal void Turned()
    {
        HideChrome();
        PositionChanged();
        Announce();
    }

    void Announce()
    {
        if (!DeviceWatch.ScreenReaderRunning() || kind != Kind.Paged || paged is null)
            return;
        var leaf = paged.Current;
        if (leaf < 0 || leaf >= Reader.Leaves.Length)
            return;
        var pages = Reader.Leaves[leaf].Parts.Select(p => (int)p.Page + 1).Distinct().Order().ToList();
        var total = Reader.PageCount;
        announcer.Text = Reader.Leaves[leaf].EndPage ? T.WindowsReaderEndOfChapter(Reader.View?.Chapter.Label ?? "")
            : pages.Count >= 2 ? T.WindowsReaderPagesLabel(pages[0], pages[1], total)
            : T.WindowsReaderPageLabel(pages.FirstOrDefault(1), total);
        var peer = FrameworkElementAutomationPeer.FromElement(announcer) ?? FrameworkElementAutomationPeer.CreatePeerForElement(announcer);
        peer?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
    }

    /// <summary>The place changed: the group's line and the scrubber follow.</summary>
    internal void PositionChanged()
    {
        var label = Reader.View?.Chapter.Label ?? "";
        switch (kind)
        {
            case Kind.Paged when paged is not null:
            {
                var leaf = paged.Current;
                var total = Reader.PageCount;
                if (leaf < 0 || leaf >= Reader.Leaves.Length)
                {
                    chrome.Line(label);
                    chrome.Scrub(1, total, Rtl, null);
                    break;
                }
                var pages = Reader.Leaves[leaf].Parts.Select(p => (int)p.Page + 1).Distinct().Order().ToList();
                if (Reader.Leaves[leaf].EndPage)
                    chrome.Line(T.WindowsReaderEndOfChapter(label));
                else if (pages.Count >= 2)
                    chrome.Line(T.WindowsReaderGroupLineSpread(label, pages[0], pages[1]));
                else
                    chrome.Line(T.WindowsReaderGroupLine(label, pages.FirstOrDefault(1), Math.Max(total, 1)));
                var at = Reader.Leaves[leaf].EndPage ? total : Reader.PageNumber(leaf);
                int? ringAt = Reader.Ring is { } r && r >= 0 && r < Reader.Leaves.Length ? Reader.PageNumber(r) : null;
                chrome.Scrub(at, total, Rtl, ringAt);
                break;
            }
            case Kind.Strip when strip is not null:
            {
                var (image, count) = strip.Position;
                chrome.Line(T.WindowsReaderGroupLineStrip(label, Math.Min(image + 1, Math.Max(count, 1)), Math.Max(count, 1)));
                chrome.Scrub(image + 1, count, false, null);
                break;
            }
            case Kind.Novel when novel is not null:
            {
                var scrolled = Reader.Choice(ReaderSettingKey.Mode) == "scrolled";
                if (novel.EndShown)
                    chrome.Line(T.WindowsReaderEndOfChapter(label));
                else if (scrolled)
                    chrome.Line(T.WindowsReaderGroupLineScroll(label, novel.Pages > 1 ? novel.Page * 100 / (novel.Pages - 1) : 0));
                else
                    chrome.Line(T.WindowsReaderGroupLine(label, novel.Page + 1, novel.Pages));
                chrome.Scrub(novel.Page + 1, novel.Pages, Rtl, null);
                break;
            }
            default:
                chrome.Line(label);
                chrome.Scrub(1, 1, false, null);
                break;
        }
    }

    internal void ScrubTo(int value)
    {
        switch (kind)
        {
            case Kind.Paged:
                JumpToLeaf(Reader.LeafOfPage(value));
                break;
            case Kind.Strip:
                strip?.JumpTo(value - 1);
                break;
            case Kind.Novel:
                novel?.JumpTo(value - 1);
                break;
        }
    }

    internal int ShownLeaf => paged?.Current ?? Reader.Current;

    (string Chapter, uint Page)? askedForPreview;

    /// <summary>
    /// The scrub preview's page for a scrubber value: the file once it has landed, and whether
    /// there is a page to picture at all (a novel's preview is its number alone). A page not
    /// asked for yet is asked for, as the Mac's preview does.
    /// </summary>
    internal (PageFile? File, bool Pictured) PreviewOf(int value)
    {
        var chapter = Reader.View?.Chapter.Id ?? "";
        LeafPart? part = null;
        switch (kind)
        {
            case Kind.Paged:
            {
                var leaf = Reader.LeafOfPage(value);
                if (leaf >= 0 && leaf < Reader.Leaves.Length && !Reader.Leaves[leaf].EndPage)
                {
                    var parts = Reader.Leaves[leaf].Parts;
                    part = parts.FirstOrDefault(p => p.Page == (uint)Math.Max(value - 1, 0)) ?? parts.FirstOrDefault();
                }
                break;
            }
            case Kind.Strip:
            {
                var index = value - 1;
                foreach (var leaf in Reader.Leaves)
                {
                    if (leaf.EndPage)
                        continue;
                    if (index < leaf.Parts.Length)
                    {
                        part = leaf.Parts[index];
                        break;
                    }
                    index -= leaf.Parts.Length;
                }
                break;
            }
            default:
                return (null, false);
        }
        if (part is null)
            return (null, true);
        var file = Reader.FileOf(part, chapter);
        if (file is null && Reader.FailureOf(part, chapter) is null && askedForPreview != (chapter, part.Page))
        {
            askedForPreview = (chapter, part.Page);
            Reader.NeedPage(part.Page);
        }
        return (file, true);
    }

    internal void JumpToLeaf(int leaf) => paged?.Jump(leaf);

    internal NovelTocEntry[] NovelContents => novel?.Contents ?? [];

    internal void NovelGoToUrl(string url) => novel?.GoToUrl(url);

    internal void OpenSettings()
    {
        ShowChrome(stay: true);
        settingsFlyout.ShowAt(chrome.Settings);
    }

    internal void ToggleFullscreen()
    {
        Ctx.Window.ReaderSetFullscreen(null);
        chrome.FullscreenShown(Ctx.Window.ReaderFullscreen);
    }

    // Ways out

    /// <summary>Back to the series page, where another source can be chosen.</summary>
    internal void ToSeries()
    {
        var at = work ?? Reader.View?.Work;
        Close();
        if (at is not null)
            Ctx.Window.OpenSeries(at);
    }

    /// <summary>The reading demo's way on: the place sources are added.</summary>
    internal void AddSource()
    {
        Close();
        Ctx.Window.Go("providers");
    }

    /// <summary>The reading demo's Back to page 1.</summary>
    internal void FromStart()
    {
        switch (kind)
        {
            case Kind.Paged:
                paged?.Jump(0);
                break;
            case Kind.Strip:
                strip?.JumpTo(0);
                break;
            case Kind.Novel when Reader.Novel is { Spine.Length: > 0 } book:
                novel?.GoToUrl(book.Spine[0].Url);
                break;
        }
    }

    internal void MoreSettings()
    {
        Close();
        Ctx.Window.ShowSettings("reading");
    }

    internal void RetryProgress(string server) => Ctx.Known.Books.RetryProgress(server);

    internal FrameworkElement BuildEndPage() => new EndPageView(this, ActualWidth);

    ServerLineView? serverLine;

    /// <summary>The book server's line, one for the reader, which each end page built takes.</summary>
    internal ServerLineView ServerLine => serverLine ??= new ServerLineView(this);

    // Input

    void OnTapped(object sender, TappedRoutedEventArgs e)
    {
        for (var at = e.OriginalSource as DependencyObject; at is not null && at != stage; at = VisualTreeHelper.GetParent(at))
            if (at is ButtonBase or ToggleSwitch or Slider)
                return;
        var width = stage.ActualWidth;
        if (width <= 0)
            return;
        var x = e.GetPosition(stage).X / width;
        switch (kind)
        {
            case Kind.Paged:
                paged?.Tap(x);
                break;
            case Kind.Strip:
                strip?.Tap(x);
                break;
            default:
                ToggleChrome();
                break;
        }
        Focus(FocusState.Programmatic);
    }

    void OnKey(object sender, KeyRoutedEventArgs e)
    {
        if (!showing)
            return;
        var ctrl = (InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Control) & global::Windows.UI.Core.CoreVirtualKeyStates.Down) != 0;
        var shift = (InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Shift) & global::Windows.UI.Core.CoreVirtualKeyStates.Down) != 0;
        var alt = (InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Menu) & global::Windows.UI.Core.CoreVirtualKeyStates.Down) != 0;
        var focused = XamlRoot is { } xr ? FocusManager.GetFocusedElement(xr) : null;
        var key = e.Key;
        // A control in the chrome keeps its own keys: the scrubber its arrows, a button Space and Enter.
        if (focused is Slider && key is VirtualKey.Left or VirtualKey.Right or VirtualKey.Up or VirtualKey.Down or VirtualKey.Home or VirtualKey.End or VirtualKey.PageUp or VirtualKey.PageDown)
            return;
        if (focused is ButtonBase or ToggleSwitch && key is VirtualKey.Space or VirtualKey.Enter)
            return;
        e.Handled = Key(key, ctrl, shift, alt);
    }

    /// <summary>A key from the window or from inside the novel's page. True when the reader used it.</summary>
    bool Key(VirtualKey key, bool ctrl, bool shift, bool alt)
    {
        switch (key)
        {
            case VirtualKey.Escape:
                Escape();
                return true;
            case VirtualKey.Left when alt:
                Close();
                return true;
            case VirtualKey.F11:
                ToggleFullscreen();
                return true;
        }
        if (alt)
            return false;
        if (ctrl)
            return CtrlKey(key);
        if (StateShown)
            return false;
        switch (kind)
        {
            case Kind.Paged when paged is not null:
                if (paged.TakesArrows && key is VirtualKey.Left or VirtualKey.Right or VirtualKey.Up or VirtualKey.Down)
                    return false;
                switch (key)
                {
                    case VirtualKey.Right:
                        paged.Turn(!Rtl, TurnHow.Key);
                        return true;
                    case VirtualKey.Left:
                        paged.Turn(Rtl, TurnHow.Key);
                        return true;
                    case VirtualKey.Down or VirtualKey.PageDown:
                        paged.Turn(true, TurnHow.Key);
                        return true;
                    case VirtualKey.Space:
                        paged.Turn(!shift, TurnHow.Key);
                        return true;
                    case VirtualKey.Up or VirtualKey.PageUp:
                        paged.Turn(false, TurnHow.Key);
                        return true;
                    case VirtualKey.Home:
                        paged.Ends(false);
                        return true;
                    case VirtualKey.End:
                        paged.Ends(true);
                        return true;
                }
                return false;
            case Kind.Strip when strip is not null:
                switch (key)
                {
                    case VirtualKey.Down:
                        strip.Step(true, line: true);
                        break;
                    case VirtualKey.Up:
                        strip.Step(false, line: true);
                        break;
                    case VirtualKey.PageDown or VirtualKey.Right:
                        strip.Step(true, line: false);
                        break;
                    case VirtualKey.PageUp or VirtualKey.Left:
                        strip.Step(false, line: false);
                        break;
                    case VirtualKey.Space:
                        strip.Step(!shift, line: false);
                        break;
                    case VirtualKey.Home:
                        strip.Ends(false);
                        break;
                    case VirtualKey.End:
                        strip.Ends(true);
                        break;
                    default:
                        return false;
                }
                HideChrome();
                return true;
            case Kind.Novel when novel is not null:
                return NovelKey(key switch
                {
                    VirtualKey.Left => "ArrowLeft",
                    VirtualKey.Right => "ArrowRight",
                    VirtualKey.Up => "ArrowUp",
                    VirtualKey.Down => "ArrowDown",
                    VirtualKey.PageUp => "PageUp",
                    VirtualKey.PageDown => "PageDown",
                    VirtualKey.Space => shift ? "PageUp" : " ",
                    VirtualKey.Home => "Home",
                    VirtualKey.End => "End",
                    _ => "",
                });
        }
        return false;
    }

    /// <summary>Ctrl in place of the Mac's Command: zoom for pages, text size for novels.</summary>
    bool CtrlKey(VirtualKey key)
    {
        var plus = key is VirtualKey.Add || (int)key == 187;
        var minus = key is VirtualKey.Subtract || (int)key == 189;
        var zero = key is VirtualKey.Number0 or VirtualKey.NumberPad0;
        if (!plus && !minus && !zero)
            return false;
        if (kind == Kind.Paged && paged is not null)
        {
            if (zero)
                paged.ResetZoom();
            else
                paged.ZoomBy(plus ? 1.25 : 0.8);
            return true;
        }
        if (kind == Kind.Novel)
        {
            var now = Reader.Amount(ReaderSettingKey.TextSize) ?? 1.0;
            var next = zero ? 1.0 : Math.Clamp(Math.Round((now + (plus ? 0.05 : -0.05)) * 20) / 20, 0.7, 2.0);
            Reader.Set(ReaderSettingKey.TextSize, new ReaderValue.Amount((float)next));
            return true;
        }
        return false;
    }

    /// <summary>Esc steps out: the chrome first, then full screen, then the reader.</summary>
    void Escape()
    {
        if (chromeShown && !StateShown && !DeviceWatch.ScreenReaderRunning())
        {
            HideChrome();
            if (!chromeShown)
                return;
        }
        if (Ctx.Window.ReaderFullscreen)
        {
            ToggleFullscreen();
            return;
        }
        Close();
    }

    /// <summary>A turning key the novel's page received.</summary>
    internal void PageKey(string key) => NovelKey(key);

    bool NovelKey(string key)
    {
        if (novel is null)
            return false;
        var rtl = novel.Rtl;
        switch (key)
        {
            case "ArrowRight":
                novel.Turn(!rtl, "key");
                return true;
            case "ArrowLeft":
                novel.Turn(rtl, "key");
                return true;
            case "ArrowDown" or "PageDown" or " ":
                novel.Turn(true, "key");
                return true;
            case "ArrowUp" or "PageUp":
                novel.Turn(false, "key");
                return true;
            case "Home":
                novel.Ends(false);
                return true;
            case "End":
                novel.Ends(true);
                return true;
        }
        return false;
    }

    /// <summary>A key the novel's page passed on: Esc, F11, Alt+Left, Ctrl and the zoom keys.</summary>
    internal void HostKey(string key, bool alt, bool ctrl, bool shift)
    {
        var mapped = key switch
        {
            "Escape" => VirtualKey.Escape,
            "F11" => VirtualKey.F11,
            "ArrowLeft" => VirtualKey.Left,
            "+" or "=" => VirtualKey.Add,
            "-" => VirtualKey.Subtract,
            "0" => VirtualKey.Number0,
            _ => VirtualKey.None,
        };
        if (mapped != VirtualKey.None)
            Key(mapped, ctrl, shift, alt);
    }

    /// <summary>The wheel inside a paged novel turns a page a notch at a time; a scrolled one scrolls.</summary>
    internal void NovelWheel(double dy)
    {
        if (novel is null || Reader.Choice(ReaderSettingKey.Mode) == "scrolled" || Math.Abs(dy) < 1)
            return;
        var now = Environment.TickCount64;
        if (now - lastNovelWheel < 280)
            return;
        lastNovelWheel = now;
        novel.Turn(dy > 0, "scroll");
    }
}
