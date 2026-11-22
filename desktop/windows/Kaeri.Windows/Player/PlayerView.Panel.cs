using System.Diagnostics;
using System.Numerics;
using System.Runtime.InteropServices.WindowsRuntime;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Shapes;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;
using SourceGroup = Kaeri.App.SourceGroup;
using SourceSwitch = Kaeri.Engine.Model.SourceSwitch;
using VirtualKey = global::Windows.System.VirtualKey;
using VirtualKeyModifiers = global::Windows.System.VirtualKeyModifiers;

namespace Kaeri.Windows.Player;

/// <summary>
/// The player's Episodes and Sources panel (player panel board, desktop frames): Acrylic on the
/// trailing edge, over a dim of the video. Episodes lists the title's table with a still, the
/// title and one line, and a click reveals a row's synopsis and buttons. Sources says what plays
/// now, switches subtitled and dubbed, and lists every other source and its releases; a pick
/// plays from the playhead for this episode only, the picture held until the new one lands.
/// E opens and closes it, Ctrl+1 and Ctrl+2 choose the tab, Up and Down move the selection and
/// Enter plays, and Esc closes it before anything else in the player.
/// </summary>
public sealed partial class PlayerView
{
    const double PanelWidth = 380;
    /// <summary>Below the caption buttons, as the window's title bar is while the panel is open.</summary>
    const double PanelTop = 32;
    /// <summary>How long a revealed row takes to open before it is scrolled into view.</summary>
    const int RevealSettle = 320;

    readonly PanelState panel = new();
    readonly Stopwatch panelWatch = Stopwatch.StartNew();
    long PanelNow => panelWatch.ElapsedMilliseconds;
    DispatcherQueueTimer? panelTick;
    bool panelReady, settingTabs;
    int panelMotion;

    /// <summary>The episode revealed and selected, by number.</summary>
    string? panelSelected;
    /// <summary>The synopses read whole with More.</summary>
    readonly HashSet<string> panelWhole = [];
    /// <summary>The sources with their releases open, by provider id.</summary>
    readonly HashSet<string> panelOpenSources = [];
    /// <summary>The title's art, for a row with no still of its own.</summary>
    string? panelBanner;
    /// <summary>What the start found, by title and episode: the source the episode plays from.</summary>
    (uint Id, string Episode, string? Source)? panelFound;

    string[] panelNumbers = [];
    readonly Dictionary<string, EpisodeRow> panelRows = [];
    readonly Dictionary<string, PanelEpisodeView> panelRealized = [];
    readonly Stack<PanelEpisodeView> panelPool = new();
    bool panelMetadataOnly, panelProtect;

    /// <summary>The picture copied as a switch began loading, while the engine's word on it has not come yet.</summary>
    WriteableBitmap? heldPending;

    /// <summary>Whether the panel is open: the window keeps its caption buttons at their standard height under it.</summary>
    internal bool PanelOpen => panel.Open;

    void EnsurePanel()
    {
        if (panelReady)
            return;
        panelReady = true;
        PanelEpisodes.ItemTemplate = new PanelRowFactory(this);
        ElementCompositionPreview.SetIsTranslationEnabled(PanelPane, true);
        panelTick = DispatcherQueue.CreateTimer();
        panelTick.Interval = TimeSpan.FromMilliseconds(250);
        panelTick.Tick += (_, _) => PanelTicked();
        // Seven rows the shape of the ones to come, while the table loads.
        double[] widths = [0.72, 0.58, 0.8, 0.64, 0.7, 0.52, 0.76];
        foreach (var w in widths)
        {
            var row = new Grid { Padding = new Thickness(8), ColumnSpacing = 12 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.Children.Add(new Border { Width = 96, Height = 54, CornerRadius = new CornerRadius(4), Background = Faint(0.07) });
            var lines = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
            lines.Children.Add(new Border { Height = 12, Width = 220 * w, HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(4), Background = Faint(0.08) });
            lines.Children.Add(new Border { Height = 9, Width = 220 * 0.34, HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(4), Background = Faint(0.08) });
            Grid.SetColumn(lines, 1);
            row.Children.Add(lines);
            PanelLoading.Children.Add(row);
        }
    }

    static SolidColorBrush Faint(double alpha) => new(global::Windows.UI.Color.FromArgb((byte)Math.Round(alpha * 255), 0xE4, 0xE0, 0xD6));

    static Brush Res(string key) => (Brush)Application.Current.Resources[key];

    // Opening and closing.

    /// <summary>Opens the panel on <paramref name="tab"/>, or shows that tab when it is open already.</summary>
    void OpenPanel(PanelTab tab, bool keyboard)
    {
        if (!Showing || mini || startingShown || still.Asking)
            return;
        EnsurePanel();
        if (panel.Open)
        {
            ShowPanelTab(tab, animated: panel.Tab != tab);
            FocusPanel(keyboard);
            return;
        }
        panel.Open = true;
        panelMotion++;
        CloseFlyouts();
        PanelPane.Margin = new Thickness(0, FullscreenOn?.Invoke() == true ? 0 : PanelTop, 0, 0);
        ShowPanelTab(tab, animated: false);
        RefreshPanelEpisodes(scroll: true);
        // The controls fade under it; the caption buttons stay, at the standard height.
        HideChrome();
        ControlsChanged?.Invoke(true);
        ShowPointer();
        Motion.Set(PanelLayer, true);
        PanelDim.Opacity = 0;
        Motion.FadeTo(PanelDim, 1, Motion.Standard);
        // In from the trailing edge on the standard spring; with reduced motion, a fade.
        var visual = ElementCompositionPreview.GetElementVisual(PanelPane);
        Motion.Spring(visual, "Translation", Vector3.Zero, new Vector3((float)PanelWidth + 2, 0, 0));
        PanelPane.Opacity = 1;
        if (!Motion.Enabled)
        {
            PanelPane.Opacity = 0;
            Motion.FadeTo(PanelPane, 1, Motion.Standard);
        }
        panelTick?.Start();
        // Laid out first, so the row to focus exists.
        DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => FocusPanel(keyboard));
    }

    /// <summary>Closes the panel: out to the trailing edge on the quick spring, and the controls come back.</summary>
    void ClosePanel(bool animated = true)
    {
        if (!panel.Open)
            return;
        panel.Open = false;
        var motion = ++panelMotion;
        var focused = XamlRoot is not null && FocusManager.GetFocusedElement(XamlRoot) is DependencyObject f && Within(f, PanelPane)
            ? (f as Control)?.FocusState : null;
        panelTick?.Stop();
        if (!animated)
        {
            Motion.Set(PanelLayer, false);
        }
        else
        {
            var visual = ElementCompositionPreview.GetElementVisual(PanelPane);
            Motion.Spring(visual, "Translation", new Vector3((float)PanelWidth + 2, 0, 0), quick: true);
            if (!Motion.Enabled)
                Motion.FadeTo(PanelPane, 0, Motion.Quick);
            Motion.FadeTo(PanelDim, 0, Motion.Quick);
            _ = DispatcherQueue.TryEnqueue(async () =>
            {
                await Task.Delay(Motion.Scaled(Motion.Quick) + TimeSpan.FromMilliseconds(40));
                if (motion == panelMotion && !panel.Open)
                    PanelLayer.Visibility = Visibility.Collapsed;
            });
        }
        if (!Showing || !animated)
            return;
        ShowControls();
        // The keyboard goes back to the button that opens it, or to the player.
        if (focused is { } state && state != FocusState.Unfocused)
        {
            if (EpisodesButton.Visibility != Visibility.Visible || !EpisodesButton.Focus(state == FocusState.Keyboard ? FocusState.Keyboard : FocusState.Programmatic))
                Focus(FocusState.Programmatic);
        }
    }

    /// <summary>The menus and flyouts open over the player give way to the panel.</summary>
    void CloseFlyouts()
    {
        if (XamlRoot is null)
            return;
        foreach (var popup in VisualTreeHelper.GetOpenPopupsForXamlRoot(XamlRoot))
            if (popup.Child is FlyoutPresenter or MenuFlyoutPresenter)
                popup.IsOpen = false;
    }

    void FocusPanel(bool keyboard)
    {
        if (!panel.Open)
            return;
        var state = keyboard ? FocusState.Keyboard : FocusState.Programmatic;
        if (panel.Tab == PanelTab.Episodes)
        {
            var number = panelSelected ?? (panelMetadataOnly ? null : playingEpisode);
            if (number is not null && FocusEpisode(number, state))
                return;
        }
        else if (FirstFocusable(PanelSources) is { } first && first.Focus(state))
        {
            return;
        }
        if (PanelTabs.SelectedItem is SelectorBarItem item)
            item.Focus(state);
    }

    static Control? FirstFocusable(DependencyObject root)
    {
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is Control { IsTabStop: true, IsEnabled: true, Visibility: Visibility.Visible } control)
                return control;
            if (child is UIElement { Visibility: Visibility.Collapsed })
                continue;
            if (FirstFocusable(child) is { } found)
                return found;
        }
        return null;
    }

    void OnEpisodesButton(object sender, RoutedEventArgs e)
    {
        if (panel.Open)
            ClosePanel();
        else
            OpenPanel(PanelTab.Episodes, keyboard: false);
    }

    void OnPanelClose(object sender, RoutedEventArgs e) => ClosePanel();

    void OnPanelDimTapped(object sender, TappedRoutedEventArgs e)
    {
        e.Handled = true;
        ClosePanel();
    }

    void OnKeyPanelEpisodes(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => OpenPanel(PanelTab.Episodes, keyboard: true), show: false);
    void OnKeyPanelSources(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => OpenPanel(PanelTab.Sources, keyboard: true), show: false);

    /// <summary>E: the panel on Episodes, or closed again.</summary>
    void TogglePanel()
    {
        if (panel.Open)
            ClosePanel();
        else
            OpenPanel(PanelTab.Episodes, keyboard: true);
    }

    // The tabs.

    void ShowPanelTab(PanelTab tab, bool animated)
    {
        panel.Tab = tab;
        settingTabs = true;
        // Both the bar's selection and each item's own flag, so the bar's first layout keeps the tab asked for.
        PanelEpisodesTab.IsSelected = tab == PanelTab.Episodes;
        PanelSourcesTab.IsSelected = tab == PanelTab.Sources;
        PanelTabs.SelectedItem = tab == PanelTab.Episodes ? PanelEpisodesTab : PanelSourcesTab;
        settingTabs = false;
        var show = tab == PanelTab.Episodes ? PanelEpisodesScroll : PanelSourcesScroll;
        var hide = tab == PanelTab.Episodes ? PanelSourcesScroll : PanelEpisodesScroll;
        if (tab == PanelTab.Sources)
        {
            AskPanelSources();
            BuildSources();
        }
        show.IsHitTestVisible = true;
        hide.IsHitTestVisible = false;
        if (animated && hide.Visibility == Visibility.Visible)
        {
            // A crossfade in place, the old tab going as the new one comes.
            show.Visibility = Visibility.Visible;
            show.Opacity = 0;
            Motion.Crossfade(hide, show, () =>
            {
                if (panel.Tab != tab)
                    return;
                hide.Visibility = Visibility.Collapsed;
                hide.Opacity = 1;
            });
        }
        else
        {
            Motion.Set(show, true);
            Motion.Set(hide, false);
        }
    }

    void OnPanelTab(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        if (settingTabs || !panel.Open)
            return;
        if (sender.SelectedItem is null)
        {
            // A tab is always chosen: the one showing stays.
            settingTabs = true;
            sender.SelectedItem = panel.Tab == PanelTab.Episodes ? PanelEpisodesTab : PanelSourcesTab;
            settingTabs = false;
            return;
        }
        var tab = sender.SelectedItem == PanelSourcesTab ? PanelTab.Sources : PanelTab.Episodes;
        if (tab != panel.Tab)
            ShowPanelTab(tab, animated: true);
    }

    // The keys inside the panel.

    /// <summary>
    /// Up and Down move the selection on Episodes, which reveals, and the focus on Sources; Left
    /// and Right move between a row's buttons; Enter does the row's amber action or picks it. Space
    /// is the player's play and pause, Esc its own key that closes the panel first.
    /// </summary>
    void OnPanelKey(object sender, KeyRoutedEventArgs e)
    {
        if (XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not DependencyObject focused)
            return;
        var onTabs = Within(focused, PanelTabs);
        switch (e.Key)
        {
            case VirtualKey.Up or VirtualKey.Down:
            {
                e.Handled = true;
                var by = e.Key == VirtualKey.Down ? 1 : -1;
                if (panel.Tab == PanelTab.Episodes)
                {
                    if (onTabs || focused == PanelClose)
                    {
                        if (by > 0)
                            FocusPanel(keyboard: true);
                        return;
                    }
                    if (!StepEpisode(EpisodeOf(focused), by) && by < 0)
                        (PanelTabs.SelectedItem as Control)?.Focus(FocusState.Keyboard);
                    return;
                }
                if (onTabs || focused == PanelClose)
                {
                    if (by > 0 && FirstFocusable(PanelSources) is { } first)
                        first.Focus(FocusState.Keyboard);
                    return;
                }
                var options = new FindNextElementOptions { SearchRoot = PanelPane, XYFocusNavigationStrategyOverride = XYFocusNavigationStrategyOverride.Projection };
                if (!FocusManager.TryMoveFocus(by > 0 ? FocusNavigationDirection.Down : FocusNavigationDirection.Up, options) && by < 0)
                    (PanelTabs.SelectedItem as Control)?.Focus(FocusState.Keyboard);
                return;
            }
            case VirtualKey.Left or VirtualKey.Right when !onTabs:
            {
                e.Handled = true;
                var options = new FindNextElementOptions { SearchRoot = PanelPane };
                FocusManager.TryMoveFocus(e.Key == VirtualKey.Right ? FocusNavigationDirection.Right : FocusNavigationDirection.Left, options);
                return;
            }
            case VirtualKey.Enter:
            {
                e.Handled = true;
                if (e.KeyStatus.WasKeyDown)
                    return;
                if (focused is Button head && head.Tag is PanelEpisodeView view)
                {
                    EpisodeEnter(view.Number);
                    return;
                }
                if (focused is RadioButton radio)
                {
                    if (radio.IsEnabled)
                        radio.IsChecked = true;
                    return;
                }
                if (focused is ButtonBase button && button.IsEnabled
                    && (FrameworkElementAutomationPeer.FromElement(button) ?? FrameworkElementAutomationPeer.CreatePeerForElement(button)) is IInvokeProvider invoke)
                    invoke.Invoke();
                return;
            }
        }
    }

    // The Episodes tab.

    /// <summary>The table again: rows added or gone are laid out anew, the rest bound to their new state where they show.</summary>
    void RefreshPanelEpisodes(bool scroll = false)
    {
        if (!panelReady)
            return;
        var table = ctx?.Known.Episodes.GetValueOrDefault(playingId);
        if (table is null)
        {
            PanelLoading.Visibility = Visibility.Visible;
            PanelMetadataOnly.Visibility = Visibility.Collapsed;
            panelNumbers = [];
            panelRows.Clear();
            PanelEpisodes.ItemsSource = null;
            return;
        }
        PanelLoading.Visibility = Visibility.Collapsed;
        panelMetadataOnly = PanelWords.MetadataOnly(table);
        panelProtect = ownSpoilers ?? SettingsCatalog.IsOn(ctx?.Known.Setting(SettingKey.SpoilerProtection));
        PanelMetadataOnly.Visibility = panelMetadataOnly ? Visibility.Visible : Visibility.Collapsed;
        panelRows.Clear();
        foreach (var row in table)
            panelRows.TryAdd(row.Number, row);
        string[] numbers = [.. table.Select(r => r.Number)];
        if (!numbers.SequenceEqual(panelNumbers))
        {
            panelNumbers = numbers;
            PanelEpisodes.ItemsSource = numbers;
        }
        else
        {
            foreach (var view in panelRealized.Values)
                BindEpisode(view);
        }
        if (scroll)
        {
            // The playing row near the top, with the one before it above.
            var at = Array.IndexOf(panelNumbers, panelSelected ?? playingEpisode);
            if (at >= 0)
                DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
                {
                    if (PanelEpisodes.GetOrCreateElement(Math.Max(0, at - 1)) is FrameworkElement element)
                    {
                        element.UpdateLayout();
                        element.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 });
                    }
                });
        }
    }

    /// <summary>One episode's row, from the pool or new.</summary>
    internal UIElement PanelRowFor(string number)
    {
        var view = panelPool.Count > 0 ? panelPool.Pop() : NewEpisodeView();
        if (panelRealized.TryGetValue(view.Number, out var was) && was == view)
            panelRealized.Remove(view.Number);
        view.Number = number;
        view.Actions = "";
        panelRealized[number] = view;
        BindEpisode(view);
        return view.Root;
    }

    internal void PanelRowGone(UIElement element)
    {
        if ((element as FrameworkElement)?.Tag is not PanelEpisodeView view)
            return;
        if (panelRealized.TryGetValue(view.Number, out var shown) && shown == view)
            panelRealized.Remove(view.Number);
        panelPool.Push(view);
    }

    PanelEpisodeView NewEpisodeView()
    {
        var view = new PanelEpisodeView();
        view.Root.Tag = view;
        view.Head.Tag = view;
        view.Head.Click += (_, _) =>
        {
            still.Input();
            RevealEpisode(view.Number, toggle: true);
        };
        // A double-click plays at once: the first click has revealed it.
        view.Head.DoubleTapped += (_, e) =>
        {
            e.Handled = true;
            PlayEpisodeNow(view.Number);
        };
        view.More.Click += (_, _) =>
        {
            if (!panelWhole.Remove(view.Number))
                panelWhole.Add(view.Number);
            BindSynopsis(view);
        };
        view.Synopsis.IsTextTrimmedChanged += (_, _) => ShowMore(view);
        return view;
    }

    void BindEpisode(PanelEpisodeView view)
    {
        if (!panelRows.TryGetValue(view.Number, out var row))
            return;
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var current = !panelMetadataOnly && row.Number == playingEpisode;
        var selected = row.Number == panelSelected;
        var notOut = PanelWords.NotOut(row, now);
        var hidden = panelProtect && row.SpoilerAhead && !current;
        view.Root.Background = current ? Res("SelectedFillBrush") : selected ? Res("PlayerPanelSelectedBrush") : null;
        view.Pill.Visibility = selected ? Visibility.Visible : Visibility.Collapsed;

        view.Title.Inlines.Clear();
        if (PanelWords.Title(row) is { } title)
        {
            view.Title.Inlines.Add(new Run { Text = PanelWords.Number(row.Number) + " ", Foreground = Res("HazeBrush"), FontWeight = FontWeights.Medium });
            view.Title.Inlines.Add(new Run { Text = title });
        }
        else
        {
            view.Title.Inlines.Add(new Run { Text = PanelWords.Plain(row.Number) });
        }
        // Cut to two lines, whole once revealed.
        view.Title.MaxLines = selected ? 0 : 2;
        var detail = PanelWords.Detail(row, current, panelMetadataOnly, position, duration, now);
        view.Detail.Text = detail;
        view.Detail.Visibility = detail.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        view.Detail.MaxLines = selected ? 0 : 2;
        AutomationProperties.SetName(view.Head, T.EpisodeRowSpoken(PanelWords.Heading(row), detail));

        var url = hidden || panelMetadataOnly || notOut ? null : row.Thumbnail ?? panelBanner;
        if (url != view.StillUrl)
        {
            view.StillUrl = url;
            view.Still.Source = null;
            if (url is not null)
                ctx?.Art.Bind(url, image =>
                {
                    if (view.StillUrl == url)
                        view.Still.Source = image;
                });
        }
        view.StillFrame.Opacity = row.Completed && !current && !panelMetadataOnly ? 0.62 : 1;
        BindProgress(view, row, current);

        BindSynopsis(view, row, hidden, notOut);
        var actions = PanelWords.Actions(row, current, panelMetadataOnly, notOut);
        var key = string.Join(",", actions);
        if (key != view.Actions)
        {
            view.Actions = key;
            view.Buttons.Children.Clear();
            foreach (var action in actions)
            {
                var button = new Button
                {
                    Content = PanelWords.Title(action),
                    Style = (Style)Application.Current.Resources[PanelWords.Primary(action) ? "PanelPrimaryButtonStyle" : "PanelSecondaryButtonStyle"],
                };
                var number = row.Number;
                var act = action;
                button.Click += (_, _) => RunEpisodeAction(number, act);
                view.Buttons.Children.Add(button);
            }
        }
        view.Buttons.Visibility = actions.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (!view.Moving)
        {
            view.Body.Visibility = selected && (view.Synopsis.Visibility == Visibility.Visible || actions.Length > 0) ? Visibility.Visible : Visibility.Collapsed;
            view.Body.Opacity = 1;
        }
    }

    void BindProgress(PanelEpisodeView view, EpisodeRow row, bool current)
    {
        var progress = PanelWords.Progress(row, current, panelMetadataOnly, position, duration);
        view.Progress.Visibility = progress is > 0 ? Visibility.Visible : Visibility.Collapsed;
        view.ProgressFill.Width = (96 - 8) * (progress ?? 0);
    }

    void BindSynopsis(PanelEpisodeView view)
    {
        if (!panelRows.TryGetValue(view.Number, out var row))
            return;
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var current = !panelMetadataOnly && row.Number == playingEpisode;
        BindSynopsis(view, row, panelProtect && row.SpoilerAhead && !current, PanelWords.NotOut(row, now));
    }

    /// <summary>The synopsis, cut to three lines with More while it is cut; protection's line in its place.</summary>
    void BindSynopsis(PanelEpisodeView view, EpisodeRow row, bool hidden, bool notOut)
    {
        var synopsis = row.Description is { } d && !string.IsNullOrWhiteSpace(d) ? d : null;
        if (hidden && synopsis is not null)
        {
            view.Synopsis.Text = T.EpisodeSpoilerHidden;
            view.Synopsis.FontStyle = global::Windows.UI.Text.FontStyle.Italic;
            view.Synopsis.MaxLines = 0;
            view.Synopsis.Visibility = Visibility.Visible;
        }
        else if (synopsis is not null && !notOut)
        {
            view.Synopsis.Text = synopsis;
            view.Synopsis.FontStyle = global::Windows.UI.Text.FontStyle.Normal;
            view.Synopsis.MaxLines = panelWhole.Contains(row.Number) ? 0 : 3;
            view.Synopsis.Visibility = Visibility.Visible;
        }
        else
        {
            view.Synopsis.Text = "";
            view.Synopsis.Visibility = Visibility.Collapsed;
        }
        view.Readable = synopsis is not null && !hidden && !notOut;
        ShowMore(view);
    }

    void ShowMore(PanelEpisodeView view)
    {
        var whole = panelWhole.Contains(view.Number);
        view.More.Content = whole ? T.PlayerPanelLess : T.PlayerPanelMore;
        view.More.Visibility = view.Readable && (whole || view.Synopsis.IsTextTrimmed) ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>A row selected: the one before closes on the quick spring, this one opens on the standard one, its words fading in 100 ms on.</summary>
    void RevealEpisode(string number, bool toggle)
    {
        var was = panelSelected;
        panelSelected = toggle && was == number ? null : number;
        if (was == panelSelected)
            return;
        if (was is not null && panelRealized.TryGetValue(was, out var old))
        {
            var height = old.Body.ActualHeight;
            BindEpisode(old);
            if (height > 0 && Motion.Enabled)
                Shrink(old, height);
        }
        if (panelSelected is { } now && panelRealized.TryGetValue(now, out var view))
        {
            view.Moving = false;
            BindEpisode(view);
            if (view.Body.Visibility == Visibility.Visible)
                Grow(view);
            // A row opened near the panel's foot scrolls up far enough to show its buttons.
            _ = DispatcherQueue.TryEnqueue(async () =>
            {
                await Task.Delay(Motion.Enabled ? Motion.Scaled(TimeSpan.FromMilliseconds(RevealSettle)) : TimeSpan.Zero);
                if (panelSelected == view.Number && view.Number == now)
                    view.Root.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = Motion.Enabled });
            });
        }
    }

    void Grow(PanelEpisodeView view)
    {
        var width = Math.Max(1, view.Root.ActualWidth - 16);
        view.Body.Measure(new global::Windows.Foundation.Size(width, double.PositiveInfinity));
        var to = view.Body.DesiredSize.Height;
        view.Body.Opacity = 0;
        if (Motion.Enabled)
        {
            Motion.Height(view.Body, 0, to);
            _ = DispatcherQueue.TryEnqueue(async () =>
            {
                await Task.Delay(Motion.Scaled(TimeSpan.FromMilliseconds(100)));
                if (panelSelected == view.Number)
                    Motion.FadeTo(view.Body, 1, Motion.FadeInTime);
            });
        }
        else
        {
            Motion.FadeTo(view.Body, 1, Motion.Standard);
        }
    }

    /// <summary>A row closing: its words go at once, its height on the quick spring.</summary>
    void Shrink(PanelEpisodeView view, double from)
    {
        view.Moving = true;
        view.Body.Visibility = Visibility.Visible;
        view.Body.Opacity = 0;
        var animation = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
        animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = Motion.Scaled(Motion.Quick), Value = 0, KeySpline = Motion.SpringSpline() });
        Storyboard.SetTarget(animation, view.Body);
        Storyboard.SetTargetProperty(animation, "Height");
        var board = new Storyboard();
        board.Children.Add(animation);
        var number = view.Number;
        board.Completed += (_, _) =>
        {
            board.Stop();
            view.Body.Height = double.NaN;
            if (!view.Moving)
                return;
            view.Moving = false;
            if (view.Number == number && panelSelected != number)
                view.Body.Visibility = Visibility.Collapsed;
            view.Body.Opacity = 1;
            if (view.Number != number || panelSelected == view.Number)
                BindEpisode(view);
        };
        board.Begin();
    }

    PanelEpisodeView? EpisodeOf(DependencyObject? element)
    {
        for (var at = element; at is not null && at != PanelPane; at = VisualTreeHelper.GetParent(at))
            if (at is FrameworkElement { Tag: PanelEpisodeView view })
                return view;
        return null;
    }

    /// <summary>Up or Down: the selection moves on from the row with the keyboard, and reveals.</summary>
    bool StepEpisode(PanelEpisodeView? from, int by)
    {
        if (panelNumbers.Length == 0)
            return false;
        var at = Array.IndexOf(panelNumbers, from?.Number ?? panelSelected ?? playingEpisode);
        var next = at < 0 ? 0 : at + by;
        if (next < 0 || next >= panelNumbers.Length)
            return false;
        RevealEpisode(panelNumbers[next], toggle: false);
        FocusEpisode(panelNumbers[next], FocusState.Keyboard);
        return true;
    }

    bool FocusEpisode(string number, FocusState state)
    {
        var at = Array.IndexOf(panelNumbers, number);
        if (at < 0 || PanelEpisodes.GetOrCreateElement(at) is not FrameworkElement { Tag: PanelEpisodeView view } element)
            return false;
        element.UpdateLayout();
        element.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = Motion.Enabled && state == FocusState.Keyboard });
        return view.Head.Focus(state);
    }

    /// <summary>Enter on a row: its amber action, or on a row without one, revealed or folded again.</summary>
    void EpisodeEnter(string number)
    {
        if (PrimaryOf(number) is { } action)
            RunEpisodeAction(number, action);
        else
            RevealEpisode(number, toggle: true);
    }

    void PlayEpisodeNow(string number)
    {
        if (PrimaryOf(number) is { } action)
            RunEpisodeAction(number, action);
    }

    EpisodeAction? PrimaryOf(string number)
    {
        if (!panelRows.TryGetValue(number, out var row))
            return null;
        var current = !panelMetadataOnly && row.Number == playingEpisode;
        var actions = PanelWords.Actions(row, current, panelMetadataOnly, PanelWords.NotOut(row, DateTimeOffset.UtcNow.ToUnixTimeSeconds()));
        return actions.Length > 0 && PanelWords.Primary(actions[0]) ? actions[0] : null;
    }

    /// <summary>A revealed row's button. Every one closes the panel but Mark watched.</summary>
    void RunEpisodeAction(string number, EpisodeAction action)
    {
        if (ctx is not { } c || playingId == 0)
            return;
        still.Input();
        switch (action)
        {
            case EpisodeAction.MarkWatched:
                c.Send(new Command.SetWatched(playingId, [number], true));
                break;
            case EpisodeAction.FromBeginningPlaying:
                ClosePanel();
                SeekTo(0);
                break;
            case EpisodeAction.FromBeginning:
                ClosePanel();
                if (InParty)
                    PlayInRoom(number);
                else
                    c.Send(new Command.Play(playingId, number, true));
                break;
            default:
                ClosePanel();
                ChooseEpisode(number);
                break;
        }
    }

    // The Sources tab.

    void AskPanelSources()
    {
        if (ctx is not { } c || playingId == 0)
            return;
        c.Known.SourceChoices.Asking(playingId);
        c.Send(new Command.LoadSourceOptions(playingId));
        if (panel.AskReleases() is { } asked)
            c.Send(new Command.LoadSources(asked.Id, asked.Episode));
    }

    /// <summary>The rows on the Sources tab whose switch the clock moves on: the spinner, and "Starting · 3 s".</summary>
    readonly List<(SwitchTarget Target, UIElement Spinner, TextBlock Detail, TextBlock Counting)> panelChoices = [];

    /// <summary>The source whose releases just opened or closed, for its chevron to turn from where it was.</summary>
    (string Provider, bool Open)? panelTurned;

    /// <summary>The Sources tab laid out again, the keyboard kept on the row it was on.</summary>
    void BuildSources()
    {
        if (!panelReady || !panel.Open && PanelSources.Children.Count > 0)
            return;
        string? focusKey = null;
        var focusState = FocusState.Unfocused;
        if (XamlRoot is not null && FocusManager.GetFocusedElement(XamlRoot) is Control { Tag: string tag } held && Within(held, PanelSources))
        {
            focusKey = tag;
            focusState = held.FocusState;
        }
        PanelSources.Children.Clear();
        panelChoices.Clear();
        if (ctx is not { } c)
            return;
        var options = c.Known.SourceChoices.For(playingId);
        var playingOption = options is null ? null
            : panel.PlayingRelease is { } pr ? options.FirstOrDefault(o => o.ProviderId == pr.ProviderId)
            : panel.PlayingSource is { } ps ? options.FirstOrDefault(o => o.Name == ps) : null;
        var playingProvider = panel.PlayingRelease?.ProviderId ?? playingOption?.ProviderId;

        PanelSources.Children.Add(SectionHead(T.PlayerPanelPlayingNow));
        var name = panel.PlayingSource ?? playingOption?.Name;
        if (name is not null)
        {
            var block = new StackPanel { Padding = new Thickness(8, 2, 8, 4), Spacing = 1 };
            block.Children.Add(new TextBlock { Text = name, FontSize = 15, LineHeight = 22, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap, Foreground = Res("SilkBrush") });
            if (playingOption is not null)
                block.Children.Add(DetailText(PanelWords.Source(playingOption)));
            PanelSources.Children.Add(block);
        }
        if (panel.PlayingRelease is { } release)
        {
            PanelSources.Children.Add(ChoiceRow($"release/{release.ProviderId}/{release.Id}", release.Title, PanelWords.Release(release),
                SwitchTarget.Of(release), on: true, sub: true));
        }
        else
        {
            foreach (var stream in menus?.Sources ?? [])
            {
                var picked = stream;
                PanelSources.Children.Add(ChoiceRow($"stream/{stream.Id}", stream.Label, stream.Health ?? stream.Detail,
                    new SwitchTarget("", name ?? "", Stream: stream.Id), on: stream.Current,
                    click: stream.Current ? null : () => SwitchToStream(picked)));
            }
        }
        PanelSources.Children.Add(TranslationSwitch(c));
        PanelSources.Children.Add(new Border { Height = 1, Margin = new Thickness(8, 10, 8, 4), Background = Faint(0.10) });
        PanelSources.Children.Add(SectionHead(T.PlayerPanelOtherSources));
        foreach (var group in PanelWords.Others(options, panel.Releases, playingProvider))
            AddGroup(group);
        PanelSources.Children.Add(new TextBlock
        {
            Text = $"{T.PlayerPanelOrder} {T.PlayerPanelPickScope}",
            Margin = new Thickness(8, 10, 8, 4),
            FontSize = 12,
            LineHeight = 16,
            TextWrapping = TextWrapping.Wrap,
            Foreground = Res("HazeBrush"),
        });
        if (focusKey is not null)
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
            {
                if (Tagged(PanelSources, focusKey) is { } again)
                    again.Focus(focusState == FocusState.Unfocused ? FocusState.Programmatic : focusState);
            });
    }

    static Control? Tagged(DependencyObject root, string tag)
    {
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is Control { Tag: string t } control && t == tag)
                return control;
            if (Tagged(child, tag) is { } found)
                return found;
        }
        return null;
    }

    static TextBlock SectionHead(string text)
    {
        var head = new TextBlock
        {
            Text = text,
            Padding = new Thickness(8, 10, 8, 4),
            FontSize = 14,
            LineHeight = 20,
            FontWeight = FontWeights.SemiBold,
            Foreground = Res("HazeBrush"),
        };
        AutomationProperties.SetHeadingLevel(head, AutomationHeadingLevel.Level2);
        return head;
    }

    static TextBlock DetailText(string text) => new()
    {
        Text = text,
        FontSize = 12,
        LineHeight = 16,
        TextWrapping = TextWrapping.Wrap,
        Foreground = Res("HazeBrush"),
    };

    void AddGroup(SourceGroup group)
    {
        var line = group.Option is { } o ? PanelWords.Source(o) : null;
        switch (group)
        {
            case SourceGroup.Releases r:
            {
                var open = panelOpenSources.Contains(r.ProviderId);
                PanelSources.Children.Add(ChoiceRow($"source/{r.ProviderId}", r.Name, line, new SwitchTarget(r.ProviderId, r.Name),
                    disclosure: (T.PlayerPanelReleases(r.List.Length), open), click: () => ToggleReleases(r.ProviderId)));
                if (!open)
                    break;
                var list = new StackPanel { Tag = $"releases/{r.ProviderId}" };
                foreach (var release in r.List)
                {
                    var picked = release;
                    list.Children.Add(ChoiceRow($"release/{release.ProviderId}/{release.Id}", release.Title, PanelWords.Release(release),
                        new SwitchTarget(release.ProviderId, r.Name, release.Id), sub: true, inset: true, click: () => SwitchToRelease(picked)));
                }
                PanelSources.Children.Add(list);
                if (panelTurned is { Open: true } turned && turned.Provider == r.ProviderId)
                    DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => GrowList(list));
                break;
            }
            case SourceGroup.One one:
            {
                var release = one.Release;
                PanelSources.Children.Add(ChoiceRow($"source/{one.ProviderId}", one.Name, line ?? PanelWords.Release(release),
                    new SwitchTarget(release.ProviderId, one.Name, release.Id), click: () => SwitchToRelease(release)));
                break;
            }
            case SourceGroup.Dim dim:
                PanelSources.Children.Add(ChoiceRow($"source/{dim.ProviderId}", dim.Name, line, new SwitchTarget(dim.ProviderId, dim.Name), dim: true));
                break;
            case SourceGroup.Waiting waiting:
                PanelSources.Children.Add(ChoiceRow($"source/{waiting.ProviderId}", waiting.Name, line, new SwitchTarget(waiting.ProviderId, waiting.Name), waiting: true));
                break;
        }
    }

    /// <summary>A source's releases open under it (0.3 s), or fold away (quick).</summary>
    void ToggleReleases(string provider)
    {
        still.Input();
        var open = !panelOpenSources.Contains(provider);
        if (!open && Motion.Enabled && PanelSources.Children.OfType<StackPanel>().FirstOrDefault(p => p.Tag as string == $"releases/{provider}") is { } list)
        {
            panelOpenSources.Remove(provider);
            TurnChevron(provider, open: false);
            var from = list.ActualHeight;
            var animation = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
            animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
            animation.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = Motion.Scaled(Motion.Quick), Value = 0, KeySpline = Motion.SpringSpline() });
            Storyboard.SetTarget(animation, list);
            Storyboard.SetTargetProperty(animation, "Height");
            var board = new Storyboard();
            board.Children.Add(animation);
            Motion.FadeTo(list, 0, TimeSpan.FromMilliseconds(100));
            board.Completed += (_, _) =>
            {
                panelTurned = null;
                BuildSources();
            };
            board.Begin();
            return;
        }
        if (open)
            panelOpenSources.Add(provider);
        else
            panelOpenSources.Remove(provider);
        panelTurned = (provider, open);
        BuildSources();
        panelTurned = null;
    }

    void GrowList(StackPanel list)
    {
        list.Measure(new global::Windows.Foundation.Size(Math.Max(1, PanelSources.ActualWidth), double.PositiveInfinity));
        list.Opacity = 0;
        if (Motion.Enabled)
            Motion.Height(list, 0, list.DesiredSize.Height);
        Motion.FadeTo(list, 1, Motion.Standard);
    }

    void TurnChevron(string provider, bool open)
    {
        if (Tagged(PanelSources, $"source/{provider}") is not ContentControl { Content: Grid line })
            return;
        foreach (var chevron in line.Children.OfType<StackPanel>().SelectMany(p => p.Children.OfType<FontIcon>()))
            Spin(chevron, open ? 0 : 90, open ? 90 : 0);
    }

    static void Spin(FontIcon chevron, float from, float to)
    {
        var visual = ElementCompositionPreview.GetElementVisual(chevron);
        visual.CenterPoint = new Vector3(6, 6, 0);
        if (!Motion.Enabled)
        {
            visual.RotationAngleInDegrees = to;
            return;
        }
        var turn = visual.Compositor.CreateScalarKeyFrameAnimation();
        turn.InsertKeyFrame(0, from);
        turn.InsertKeyFrame(1, to, Motion.SpringEasing(visual.Compositor));
        turn.Duration = Motion.Scaled(Motion.Standard);
        visual.StartAnimation("RotationAngleInDegrees", turn);
    }

    /// <summary>
    /// A stream, source or release: the check on what plays, a spinner on its switch past 600 ms,
    /// and "Starting · 3 s" in place of its line from 3 s. A failed switch says so under it, with
    /// Try again. A row with nothing to pick is no button.
    /// </summary>
    FrameworkElement ChoiceRow(string key, string title, string? detail, SwitchTarget target, bool on = false, bool sub = false,
        bool inset = false, bool dim = false, bool waiting = false, (string Count, bool Open)? disclosure = null, Action? click = null)
    {
        var picked = panel.Switch is { } s && s.On(target) ? s : null;
        var now = PanelNow;
        var line = new Grid { ColumnSpacing = 6 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(18) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var slot = new Grid { Width = 18, VerticalAlignment = VerticalAlignment.Center };
        var spinner = new ProgressRing { Width = 16, Height = 16, IsActive = true, Foreground = Res("SilkBrush"), Opacity = 0, Visibility = Visibility.Collapsed };
        if (picked is not null && picked.Failed is null)
        {
            spinner.Visibility = Visibility.Visible;
            spinner.Opacity = picked.Spinning(now) ? 1 : 0;
            spinner.Tag = picked.Spinning(now) ? "shown" : null;
        }
        else if (on)
        {
            slot.Children.Add(new FontIcon { Glyph = "", FontSize = 14, Foreground = Res("SilkBrush") });
        }
        slot.Children.Add(spinner);
        line.Children.Add(slot);

        var words = new StackPanel { Spacing = 1, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(words, 1);
        words.Children.Add(new TextBlock
        {
            Text = title,
            FontSize = sub ? 13 : 14,
            LineHeight = sub ? 18 : 20,
            FontWeight = sub ? FontWeights.Normal : FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap,
            Foreground = Res(dim ? "HazeBrush" : "SilkBrush"),
        });
        var detailText = DetailText(detail ?? "");
        detailText.Visibility = string.IsNullOrEmpty(detail) ? Visibility.Collapsed : Visibility.Visible;
        var counting = DetailText("");
        counting.Opacity = 0;
        counting.Visibility = Visibility.Collapsed;
        if (picked?.Counting(now) is { } seconds)
        {
            counting.Text = T.PlayerPanelStarting(seconds);
            counting.Opacity = 1;
            counting.Visibility = Visibility.Visible;
            detailText.Visibility = Visibility.Collapsed;
        }
        var detailHost = new Grid();
        detailHost.Children.Add(detailText);
        detailHost.Children.Add(counting);
        if (waiting && string.IsNullOrEmpty(detail))
            detailHost.Children.Add(new ProgressRing { Width = 12, Height = 12, IsActive = true, Margin = new Thickness(0, 2, 0, 2), HorizontalAlignment = HorizontalAlignment.Left, Foreground = Res("HazeBrush") });
        words.Children.Add(detailHost);
        line.Children.Add(words);
        if (picked is not null)
            panelChoices.Add((target, spinner, detailText, counting));

        if (disclosure is { } d)
        {
            var tail = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
            tail.Children.Add(new TextBlock { Text = d.Count, FontSize = 12, Foreground = Res("HazeBrush"), VerticalAlignment = VerticalAlignment.Center });
            var chevron = new FontIcon { Glyph = "", FontSize = 12, Width = 12, Height = 12, Foreground = Res("HazeBrush") };
            tail.Children.Add(chevron);
            Grid.SetColumn(tail, 2);
            line.Children.Add(tail);
            var turned = panelTurned is { } t && t.Provider == target.ProviderId;
            chevron.Loaded += (_, _) => Spin(chevron, turned ? (d.Open ? 0 : 90) : (d.Open ? 90 : 0), d.Open ? 90 : 0);
        }

        var padding = new Thickness(2 + (inset ? 18 : 0), 8, 8, 8);
        FrameworkElement head;
        if (click is not null)
        {
            var button = new Button
            {
                Style = (Style)Application.Current.Resources["PanelRowButtonStyle"],
                Content = line,
                Padding = padding,
                MinHeight = 44,
                Tag = key,
            };
            button.Click += (_, _) => click();
            AutomationProperties.SetName(button, string.Join(", ", new[] { title, detail, disclosure?.Count }.Where(w => !string.IsNullOrEmpty(w))));
            if (disclosure is { } open)
                AutomationProperties.SetItemStatus(button, open.Open ? T.PlayerPanelLess : T.PlayerPanelMore);
            head = button;
        }
        else
        {
            head = new Border { Child = line, Padding = padding, MinHeight = 44, CornerRadius = new CornerRadius(4) };
            AutomationProperties.SetName(head, string.Join(", ", new[] { title, detail }.Where(w => !string.IsNullOrEmpty(w))));
        }
        if (on)
            AutomationProperties.SetItemStatus(head, T.PlayerPanelPlayingNow);

        if (picked?.Failed is not { } failed)
            return head;
        // The failure stays on the row until the next pick, with Try again.
        var column = new StackPanel();
        column.Children.Add(head);
        var alert = new StackPanel { Spacing = 6, Padding = new Thickness(2 + (inset ? 18 : 0) + 24, 0, 8, 8) };
        var said = new Grid { ColumnSpacing = 6 };
        said.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        said.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        said.Children.Add(new FontIcon { Glyph = "", FontSize = 14, Margin = new Thickness(0, 1, 0, 0), VerticalAlignment = VerticalAlignment.Top, Foreground = Res("SystemFillColorCriticalBrush") });
        var text = DetailText(failed.Text());
        text.Foreground = Res("SystemFillColorCriticalBrush");
        AutomationProperties.SetLiveSetting(text, AutomationLiveSetting.Assertive);
        Grid.SetColumn(text, 1);
        said.Children.Add(text);
        alert.Children.Add(said);
        var retry = new Button
        {
            Content = T.ActionTryAgain,
            Style = (Style)Application.Current.Resources["PanelSecondaryButtonStyle"],
            Tag = $"{key}/retry",
        };
        retry.Click += (_, _) => RetrySwitch();
        alert.Children.Add(retry);
        column.Children.Add(alert);
        if (panelFailShown != picked)
        {
            panelFailShown = picked;
            alert.Opacity = 0;
            Motion.FadeTo(alert, 1, Motion.FadeInTime);
        }
        return column;
    }

    /// <summary>The failure last faded in, so a rebuild does not fade it in again.</summary>
    PanelSwitch? panelFailShown;

    /// <summary>Subtitled or Dubbed: the engine restarts the episode in the other at the playhead.</summary>
    FrameworkElement TranslationSwitch(Ctx c)
    {
        var current = menus?.Translation ?? Engine.Core.Translation.Sub;
        var dub = c.Known.Dubs.GetValueOrDefault(playingId);
        var pastDub = PanelWords.PastDub(dub, playingEpisode);
        var block = new StackPanel { Padding = new Thickness(8, 8, 8, 4), Spacing = 8 };
        var track = new Grid { Height = 32, Padding = new Thickness(2), CornerRadius = new CornerRadius(4), Background = Faint(0.10) };
        track.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        track.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        AutomationProperties.SetName(track, T.PlayerTranslation);
        var at = 0;
        foreach (var option in new[] { Engine.Core.Translation.Sub, Engine.Core.Translation.Dub })
        {
            var segment = new RadioButton
            {
                Style = (Style)Application.Current.Resources["PanelSegmentStyle"],
                Content = option == Engine.Core.Translation.Sub ? T.PlayerStartSubtitled : T.AudioDubbed,
                GroupName = "panel-translation",
                IsChecked = current == option,
                IsEnabled = current == option || !(option == Engine.Core.Translation.Dub && pastDub),
                Tag = option == Engine.Core.Translation.Sub ? "translation/sub" : "translation/dub",
            };
            var chosen = option;
            segment.Checked += (_, _) =>
            {
                if ((menus?.Translation ?? Engine.Core.Translation.Sub) != chosen)
                    Send(new PlayerCommand.Choose(new PlayerChoice.Translation(chosen)));
            };
            Grid.SetColumn(segment, at++);
            track.Children.Add(segment);
        }
        block.Children.Add(track);
        var notes = new StackPanel { Spacing = 2 };
        if (Wording.DubLine(dub) is { } line)
            notes.Children.Add(DetailText(line));
        if (InParty)
            notes.Children.Add(DetailText(T.PartyVersionOwn));
        if (notes.Children.Count > 0)
            block.Children.Add(notes);
        return block;
    }

    // Switching.

    /// <summary>A release from the playhead, for this episode only: the engine finds it while the episode plays on.</summary>
    void SwitchToRelease(SourceCandidate candidate)
    {
        if (ctx is not { } c || panel.Episode is not { } episode)
            return;
        still.Input();
        panel.Picked(SwitchTarget.Of(candidate), PanelNow, candidate);
        c.Send(new Command.SwitchSource(episode.Id, episode.Episode, candidate.ProviderId, candidate.Id));
        BuildSources();
    }

    /// <summary>Another stream of the source playing, from the playhead.</summary>
    void SwitchToStream(MenuSource stream)
    {
        if (stream.Current)
            return;
        panel.Picked(new SwitchTarget("", panel.PlayingSource ?? "", Stream: stream.Id), PanelNow);
        Send(new PlayerCommand.Choose(new PlayerChoice.Source(stream.Id)));
        BuildSources();
    }

    void RetrySwitch()
    {
        if (panel.Switch is not { } failed)
            return;
        if (failed.Candidate is { } candidate)
            SwitchToRelease(candidate);
        else if (failed.Target.Stream is { } stream && menus?.Sources.FirstOrDefault(s => s.Id == stream) is { } again)
            SwitchToStream(again);
    }

    /// <summary>What the engine says of a switch: the row moves on, the picture holds while it loads and goes once it lands or fails.</summary>
    void PanelSwitched(SourceSwitch state)
    {
        var effect = panel.Switched(state, PanelNow);
        switch (effect)
        {
            case PanelState.Effect.Hold:
                ShowHeld();
                break;
            case PanelState.Effect.Release:
                ReleaseHeld();
                break;
            case PanelState.Effect.ReleaseAndNotice when state is SourceSwitch.Landed landed:
                ReleaseHeld();
                Alert(T.NoticePlayingFromSource(landed.To.Name, Wording.Clock(landed.At)));
                break;
        }
        if (state is SourceSwitch.Landed)
            PanelSourcesScroll.ChangeView(null, 0, null, !Motion.Enabled);
        if (panel.Open)
            BuildSources();
    }

    /// <summary>Four times a second while the panel is open: the switch's wait, and the playing row's line.</summary>
    void PanelTicked()
    {
        if (!panel.Open)
        {
            panelTick?.Stop();
            return;
        }
        var now = PanelNow;
        foreach (var (target, spinner, detail, counting) in panelChoices)
        {
            if (panel.Switch is not { } s || !s.On(target) || s.Failed is not null)
                continue;
            if (s.Spinning(now) && spinner is FrameworkElement { Tag: null } ring)
            {
                ring.Tag = "shown";
                Motion.FadeTo(ring, 1, Motion.FadeInTime);
            }
            if (s.Counting(now) is { } seconds)
            {
                counting.Text = T.PlayerPanelStarting(seconds);
                if (counting.Visibility != Visibility.Visible)
                {
                    counting.Visibility = Visibility.Visible;
                    counting.Opacity = 0;
                    if (detail.Visibility == Visibility.Visible)
                        Motion.Crossfade(detail, counting, () => detail.Visibility = Visibility.Collapsed);
                    else
                        Motion.FadeTo(counting, 1, Motion.FadeInTime);
                }
            }
        }
        if (panel.Tab == PanelTab.Episodes && !panelMetadataOnly && panelRealized.TryGetValue(playingEpisode, out var playing)
            && panelRows.TryGetValue(playingEpisode, out var row))
        {
            var line = PanelWords.Detail(row, true, false, position, duration, DateTimeOffset.UtcNow.ToUnixTimeSeconds());
            if (playing.Detail.Text != line)
                playing.Detail.Text = line;
            BindProgress(playing, row, true);
        }
    }

    // The held picture.

    /// <summary>The picture on screen as a switch began loading, copied off the UI thread.</summary>
    internal void TookSnapshot(FfiSnapshot shot)
    {
        if (shot.Width == 0 || shot.Height == 0)
            return;
        var bitmap = new WriteableBitmap((int)shot.Width, (int)shot.Height);
        using (var stream = bitmap.PixelBuffer.AsStream())
        {
            var row = (int)shot.Width * 4;
            if (shot.Stride == row)
            {
                stream.Write(shot.Pixels, 0, Math.Min(shot.Pixels.Length, row * (int)shot.Height));
            }
            else
            {
                for (var y = 0; y < shot.Height; y++)
                    stream.Write(shot.Pixels, y * (int)shot.Stride, row);
            }
        }
        bitmap.Invalidate();
        if (panel.Holding)
            ShowHeld(bitmap);
        else
            heldPending = bitmap;
    }

    void ShowHeld(WriteableBitmap? bitmap = null)
    {
        bitmap ??= heldPending;
        heldPending = null;
        if (bitmap is null)
            return;
        HeldPicture.Source = bitmap;
        Motion.Set(HeldPicture, true);
    }

    /// <summary>The new picture is up, or the old one back: the held one fades over it (about 0.3 s) and is dropped.</summary>
    void ReleaseHeld()
    {
        heldPending = null;
        if (HeldPicture.Visibility != Visibility.Visible)
            return;
        Motion.FadeOutThen(HeldPicture, () =>
        {
            HeldPicture.Visibility = Visibility.Collapsed;
            HeldPicture.Source = null;
            HeldPicture.Opacity = 1;
        });
    }

    // What the engine says.

    /// <summary>The engine's events the panel follows, ahead of the player's own handling of them.</summary>
    void PanelHeard(Event e)
    {
        switch (e)
        {
            case Event.Episodes ep when ep.Id == playingId && panel.Open:
                RefreshPanelEpisodes();
                break;
            case Event.Progress p when p.Id == playingId && panel.Open:
                RefreshPanelEpisodes();
                break;
            case Event.ShowSettings st when st.V1.Id == playingId && panel.Open:
                DispatcherQueue.TryEnqueue(() => RefreshPanelEpisodes());
                break;
            case Event.Sources s:
                if (panel.Listed(s.Id, s.Episode, s.Candidates) && panel.Open)
                    BuildSources();
                break;
            case Event.SourceOptions o when o.Id == playingId && panel.Open:
            case Event.TitleDub td when td.Id == playingId && panel.Open:
            case Event.PartyJoined or Event.PartyLeft when panel.Open:
                BuildSources();
                break;
            case Event.StartingStep { Step: StartStep.Found found } step:
                panelFound = (step.Id, step.Episode, found.Source);
                if (step.Id == playingId && step.Episode == playingEpisode && found.Source is { } source)
                    panel.PlayingSource = source;
                break;
            case Event.PlayerSwitch ps when ps.PlaybackValue == CurrentPlayback() || CurrentPlayback() == 0:
                PanelSwitched(ps.State);
                break;
            case Event.PlaybackEnded:
                panel.LetGo();
                ReleaseHeld();
                break;
        }
    }

    ulong CurrentPlayback()
    {
        lock (loads)
            return playback;
    }

    /// <summary>Another episode on screen: its releases, picks and rows are its own.</summary>
    void PanelEpisodeIs(uint id, string episode, bool starting)
    {
        if (panel.Episode is { } e && e.Id == id && e.Episode == episode)
            return;
        panel.Playing(id, episode);
        panel.PlayingSource = panelFound is { } found && found.Id == id && found.Episode == episode ? found.Source : null;
        panel.LetGo();
        ReleaseHeld();
        panelSelected = null;
        panelWhole.Clear();
        panelOpenSources.Clear();
        panelFailShown = null;
        if (starting)
        {
            ClosePanel(animated: false);
            return;
        }
        if (!panel.Open)
            return;
        RefreshPanelEpisodes(scroll: true);
        if (panel.Tab == PanelTab.Sources)
        {
            AskPanelSources();
            BuildSources();
        }
    }

    /// <summary>The engine's menus changed: the tab shown follows them.</summary>
    void PanelMenus()
    {
        if (!panel.Open)
            return;
        if (panel.Tab == PanelTab.Sources)
            BuildSources();
        else
            RefreshPanelEpisodes();
    }

    /// <summary>Whether a key belongs to the panel while the keyboard is in it, rather than to the player.</summary>
    bool PanelTakes(KeyboardAcceleratorInvokedEventArgs e)
    {
        if (!panel.Open || XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not DependencyObject focused || !Within(focused, PanelPane))
            return false;
        return e.KeyboardAccelerator.Key is VirtualKey.Up or VirtualKey.Down or VirtualKey.Left or VirtualKey.Right or VirtualKey.Enter
            && e.KeyboardAccelerator.Modifiers == VirtualKeyModifiers.None;
    }

    /// <summary>Space on a button in the panel still plays and pauses.</summary>
    bool InPanel(DependencyObject element) => panel.Open && Within(element, PanelPane);
}

/// <summary>One episode's row in the panel: its still, title and line, and revealed, the synopsis and buttons.</summary>
sealed class PanelEpisodeView
{
    public string Number = "";
    public string Actions = "";
    public string? StillUrl;
    public bool Moving, Readable;
    public readonly Grid Root = new() { CornerRadius = new CornerRadius(4) };
    public readonly Border Pill = new() { Width = 3, Height = 16, CornerRadius = new CornerRadius(2), HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 27, 0, 0), Visibility = Visibility.Collapsed };
    public readonly Button Head = new();
    public readonly Grid StillFrame = new() { Width = 96, Height = 54, CornerRadius = new CornerRadius(4), VerticalAlignment = VerticalAlignment.Center };
    public readonly Image Still = new() { Stretch = Stretch.UniformToFill };
    public readonly Grid Progress = new() { Height = 3, Margin = new Thickness(4), VerticalAlignment = VerticalAlignment.Bottom, CornerRadius = new CornerRadius(2) };
    public readonly Border ProgressFill = new() { HorizontalAlignment = HorizontalAlignment.Left };
    public readonly TextBlock Title = new() { FontSize = 14, LineHeight = 20, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap, TextTrimming = TextTrimming.CharacterEllipsis };
    public readonly TextBlock Detail = new() { FontSize = 12, LineHeight = 16, TextWrapping = TextWrapping.Wrap, TextTrimming = TextTrimming.CharacterEllipsis };
    public readonly StackPanel Body = new() { Spacing = 10, Margin = new Thickness(8, 2, 8, 8), Visibility = Visibility.Collapsed };
    public readonly TextBlock Synopsis = new() { FontSize = 12, LineHeight = 17, TextWrapping = TextWrapping.Wrap, TextTrimming = TextTrimming.CharacterEllipsis };
    public readonly HyperlinkButton More = new() { Padding = new Thickness(0, 4, 0, 4), FontSize = 12, FontWeight = FontWeights.SemiBold };
    public readonly StackPanel Buttons = new() { Orientation = Orientation.Horizontal, Spacing = 8 };

    public PanelEpisodeView()
    {
        var silk = (Brush)Application.Current.Resources["SilkBrush"];
        var haze = (Brush)Application.Current.Resources["HazeBrush"];
        Pill.Background = silk;
        StillFrame.Background = new SolidColorBrush(global::Windows.UI.Color.FromArgb(18, 0xE4, 0xE0, 0xD6));
        StillFrame.Children.Add(Still);
        Progress.Background = new SolidColorBrush(global::Windows.UI.Color.FromArgb(82, 0xE4, 0xE0, 0xD6));
        ProgressFill.Background = silk;
        Progress.Children.Add(ProgressFill);
        StillFrame.Children.Add(Progress);
        AutomationProperties.SetAccessibilityView(StillFrame, AccessibilityView.Raw);

        var line = new Grid { ColumnSpacing = 12 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.Children.Add(StillFrame);
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        Title.Foreground = silk;
        Detail.Foreground = haze;
        words.Children.Add(Title);
        words.Children.Add(Detail);
        Grid.SetColumn(words, 1);
        line.Children.Add(words);
        Head.Style = (Style)Application.Current.Resources["PanelRowButtonStyle"];
        Head.Content = line;

        Synopsis.Foreground = haze;
        More.Foreground = silk;
        var synopsis = new StackPanel { Spacing = 0 };
        synopsis.Children.Add(Synopsis);
        synopsis.Children.Add(More);
        Body.Children.Add(synopsis);
        Body.Children.Add(Buttons);

        var column = new StackPanel();
        column.Children.Add(Head);
        column.Children.Add(Body);
        Root.Children.Add(column);
        Root.Children.Add(Pill);
    }
}

/// <summary>The Episodes list's rows, from the panel's pool: a long show's table makes only the rows on screen.</summary>
sealed partial class PanelRowFactory(PlayerView view) : IElementFactory
{
    public UIElement GetElement(ElementFactoryGetArgs args) => view.PanelRowFor((string)args.Data);

    public void RecycleElement(ElementFactoryRecycleArgs args) => view.PanelRowGone(args.Element);
}
