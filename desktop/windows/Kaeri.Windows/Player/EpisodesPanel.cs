using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Player;

/// <summary>
/// The player's Episodes panel: a flyout from the Episodes button with compact rows, and under
/// them a footer with the title and synopsis of the row under the pointer or the keyboard focus,
/// else the one playing. The footer is reserved only when the table has a synopsis as the panel
/// opens, so text arriving later moves nothing. A row spoiler protection holds back shows its
/// title and facts.
/// </summary>
sealed class EpisodesPanel
{
    const double Width = 236, ListMax = 360, FooterHeight = 92;

    public Flyout Flyout { get; } = new() { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.BottomEdgeAlignedRight };

    readonly Func<(MenuEpisode[] Episodes, EpisodeRow[] Table, bool Protect)> source;
    readonly Action<string> choose;

    public EpisodesPanel(Func<(MenuEpisode[] Episodes, EpisodeRow[] Table, bool Protect)> source, Action<string> choose)
    {
        this.source = source;
        this.choose = choose;
        Flyout.FlyoutPresenterStyle = Presenter();
        Flyout.Opening += (_, _) => Build();
    }

    static Style Presenter()
    {
        var style = new Style(typeof(FlyoutPresenter));
        style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(0)));
        style.Setters.Add(new Setter(FrameworkElement.MinWidthProperty, Width));
        style.Setters.Add(new Setter(FrameworkElement.MaxWidthProperty, Width));
        return style;
    }

    /// <summary>The footer's words for a row: the synopsis, or the facts where protection holds it back or there is none.</summary>
    static (string Heading, string Body) Words(MenuEpisode episode, EpisodeRow? row, bool protect)
    {
        var heading = Lines.Episode(episode.Label, episode.Title, null);
        if (row is null)
            return (heading, "");
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var held = Synopsis.Held(row, protect, false);
        return (heading, Synopsis.TextOf(row) is { } text && !held ? text : Synopsis.Facts(row, now, at => Synopsis.Date(at)));
    }

    void Build()
    {
        var (episodes, table, protect) = source();
        var reserved = KaeriFfiMethods.EpisodeText(table) == EpisodeText.Present;
        var playing = episodes.FirstOrDefault(e => e.Current);
        EpisodeRow? RowOf(MenuEpisode e) => table.FirstOrDefault(r => r.Number == e.Number);

        var heading = new TextBlock { Style = (Style)Application.Current.Resources["HazeCaptionStyle"], TextTrimming = TextTrimming.CharacterEllipsis };
        var body = new TextBlock { TextWrapping = TextWrapping.Wrap, MaxLines = 3, TextTrimming = TextTrimming.CharacterEllipsis, FontSize = 12, LineHeight = 17 };
        body.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
        void Show(MenuEpisode? e)
        {
            if (e is null)
                return;
            var (h, b) = Words(e, RowOf(e), protect);
            if (heading.Text == h && body.Text == b)
                return;
            heading.Text = h;
            body.Text = b;
        }
        Show(playing);

        var list = new StackPanel { Padding = new Thickness(4, 6, 4, 6) };
        Button? current = null;
        buttons.Clear();
        // A show of over 100 episodes lists them in ranges of 100, the one playing open first.
        var ranges = KaeriFfiMethods.EpisodeRanges(table, DateTimeOffset.UtcNow.ToUnixTimeSeconds());
        var rangeOf = new Dictionary<string, int>();
        for (var r = 0; r < ranges.Length; r++)
            for (var i = (int)ranges[r].Start; i < Math.Min((int)ranges[r].End, table.Length); i++)
                rangeOf.TryAdd(table[i].Number, r);
        var rowRange = new List<int>();
        var shownRange = playing is not null && rangeOf.TryGetValue(playing.Number, out var playingRange) ? playingRange : 0;
        // Back from Go to episode: the panel opens on the range that holds it, the keyboard on it.
        var target = goTarget;
        goTarget = null;
        if (target is not null && rangeOf.TryGetValue(target, out var targetRange))
            shownRange = targetRange;
        foreach (var episode in episodes)
        {
            var line = new Grid { ColumnSpacing = 10 };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(24) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var number = new TextBlock { Text = episode.Number, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
            if (episode.Current)
                number.Foreground = (Brush)Application.Current.Resources["AmberBrush"];
            var title = new TextBlock { Text = episode.Title ?? episode.Label, TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Center };
            if (episode.Watched)
                title.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
            Grid.SetColumn(title, 1);
            line.Children.Add(number);
            line.Children.Add(title);
            if (episode.Watched)
            {
                var check = new FontIcon { Glyph = "", FontSize = 12, Foreground = (Brush)Application.Current.Resources["HazeBrush"] };
                Grid.SetColumn(check, 2);
                line.Children.Add(check);
            }
            // A row answers the pointer with Fluent's subtle fill at the control radius, as the
            // menus beside the panel do, not with the solid control fill.
            var row = new Button
            {
                Style = (Style)Application.Current.Resources["SubtleButtonStyle"],
                Content = line,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                BorderThickness = new Thickness(0),
                Padding = new Thickness(8, 6, 8, 6),
            };
            AutomationProperties.SetName(row, Lines.Episode(episode.Label, episode.Title, null));
            var picked = episode;
            row.PointerEntered += (_, _) => Show(picked);
            row.GotFocus += (_, _) => Show(picked);
            row.Click += (_, _) =>
            {
                Flyout.Hide();
                choose(picked.Number);
            };
            if (target is null ? episode.Current : episode.Number == target)
                current = row;
            list.Children.Add(row);
            buttons.Add(row);
            rowRange.Add(rangeOf.GetValueOrDefault(episode.Number));
        }
        void ShowRange()
        {
            for (var i = 0; i < buttons.Count; i++)
                buttons[i].Visibility = ranges.Length < 2 || rowRange[i] == shownRange ? Visibility.Visible : Visibility.Collapsed;
        }
        ShowRange();
        // Left again, the footer goes back to the episode playing.
        list.PointerExited += (_, _) => Show(playing);

        var scroll = new ScrollViewer { MaxHeight = ListMax, Content = list, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
        FrameworkElement? picker = null;
        if (ranges.Length > 1)
        {
            var items = Synopsis.RangeItems(table, ranges);
            var menu = new MenuFlyout { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.BottomEdgeAlignedLeft };
            var dropdown = new DropDownButton { Content = items[Math.Min(shownRange, items.Length - 1)].Label, Margin = new Thickness(8, 8, 8, 0), HorizontalAlignment = HorizontalAlignment.Left };
            AutomationProperties.SetName(dropdown, T.EpisodeRangeLabel);
            for (var r = 0; r < items.Length; r++)
            {
                var at = r;
                var item = new RadioMenuFlyoutItem { Text = items[r].State is null ? items[r].Label : $"{items[r].Label} \u00b7 {items[r].State}", GroupName = "player-ranges", IsChecked = r == shownRange };
                AutomationProperties.SetName(item, items[r].Spoken);
                item.Click += (_, _) =>
                {
                    if (at == shownRange)
                        return;
                    shownRange = at;
                    dropdown.Content = items[at].Label;
                    // The old rows dim as the new ones come in: never empty between.
                    list.Opacity = 0.4;
                    ShowRange();
                    scroll.ChangeView(null, 0, null, true);
                    Motion.Fade(list, 1);
                };
                menu.Items.Add(item);
            }
            // The title page's last item: Go to episode, which opens the panel again on that row.
            menu.Items.Add(new MenuFlyoutSeparator());
            var go = new MenuFlyoutItem { Text = T.EpisodeGoTo, KeyboardAcceleratorTextOverride = "Ctrl+G" };
            go.Click += (_, _) => GoTo(table);
            menu.Items.Add(go);
            dropdown.Flyout = menu;
            picker = dropdown;
            rangePicker = dropdown;
        }
        var footer = new StackPanel
        {
            Spacing = 3,
            Padding = new Thickness(12, 8, 12, 10),
            Height = FooterHeight,
            BorderThickness = new Thickness(0, 1, 0, 0),
            BorderBrush = (Brush)Application.Current.Resources["FaintRuleBrush"],
            Visibility = reserved ? Visibility.Visible : Visibility.Collapsed,
        };
        footer.Children.Add(heading);
        footer.Children.Add(body);
        var panel = new StackPanel { Width = Width };
        if (picker is not null)
            panel.Children.Add(picker);
        panel.Children.Add(scroll);
        panel.Children.Add(footer);
        Flyout.Content = panel;
        // Opened, the keyboard starts on the episode playing.
        Flyout.Opened -= OpenedAt;
        focusAt = current;
        Flyout.Opened += OpenedAt;
    }

    Button? focusAt;
    string? goTarget;
    DropDownButton? rangePicker;

    /// <summary>Go to episode from the panel: the dialog, then the panel again with that row focused.</summary>
    async void GoTo(EpisodeRow[] table)
    {
        var anchor = Flyout.Target;
        var root = Flyout.XamlRoot ?? anchor?.XamlRoot;
        Flyout.Hide();
        if (root is null || table.Length == 0)
            return;
        if (await Pages.GoToDialog.Ask(root, table) is not { } row || row.Index >= table.Length)
            return;
        goTarget = table[(int)row.Index].Number;
        if (anchor is not null)
            Flyout.ShowAt(anchor);
    }
    readonly List<Button> buttons = [];

#if KAERI_TESTING
    /// <summary>For a shot: the range menu open.</summary>
    public void OpenRanges()
    {
        if (rangePicker?.Flyout is MenuFlyout menu)
            menu.ShowAt(rangePicker);
    }

    /// <summary>For a shot: the keyboard on row <paramref name="at"/>, which the footer follows.</summary>
    public void FocusRow(int at)
    {
        if (at >= 0 && at < buttons.Count)
            buttons[at].Focus(FocusState.Keyboard);
    }
#endif

    void OpenedAt(object? sender, object e)
    {
        if (focusAt is not { } row)
            return;
        row.Focus(FocusState.Programmatic);
        // Focus alone leaves the list at its top: the row comes into view a third of the way down,
        // once the panel has laid out.
        row.DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
            row.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0.33 }));
    }
}
