using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Player;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

using Pick = Kaeri.Engine.Model.Pick;

/// <summary>
/// Search's filters panel: every filter the engine offers, in the Mac's sections (genres, tags,
/// released, length, reception, origin, where to watch, your list), 340 wide at the page's right
/// edge. Each change searches as it is made; there is nothing to apply. The controls are made
/// once and <see cref="Show"/> sets each to what the filters say, so focus stays where it was.
/// The genres, tags and streaming sites come from the engine's facets, made again when they come.
/// </summary>
sealed partial class SearchFiltersPanel : Grid
{
    public const double PanelWidth = 340;

    readonly Ctx ctx = Ctx.Current;
    readonly Func<SearchQuery> current;
    readonly Action<SearchQuery> change;
    /// <summary>Set while the controls take what the filters say, so their events change nothing.</summary>
    bool showing;
    SearchQuery shown = Searches.Nothing;
    SearchFacets? facets;

    readonly Button reset, closeButton;
    readonly ScrollViewer scroll;
    readonly WrapRow genres = Row();
    readonly List<(string Name, ToggleButton Chip)> genreChips = [];
    readonly TextBox findTag = new() { PlaceholderText = T.FilterTagFind };
    readonly Slider tagRank = new() { Minimum = 0, Maximum = 100, StepFrequency = 10, TickFrequency = 10, SnapsTo = SliderSnapsTo.Ticks };
    readonly TextBlock tagRankWords = new();
    readonly StackPanel tagGroups = new() { Spacing = 4 };
    readonly StackPanel tagsFound = new() { Spacing = 0 };
    readonly TextBlock noTag = Ui.Haze(T.FilterTagNone);
    readonly ToggleSwitch spoilers = Switch();
    readonly List<(string Name, CheckBox Box)> tagRows = [];
    readonly List<(TagGroup Group, TextBlock Count)> groupCounts = [];
    readonly (NumberBox From, NumberBox To) years = Range(T.FilterReleased), episodes = Range(T.FilterEpisodes), minutes = Range(T.FilterEpisodeLength);
    readonly List<(Bounds Bounds, ToggleButton Chip)> episodePresets = [], minutePresets = [];
    readonly Slider score = new() { Minimum = 0, Maximum = 95, StepFrequency = 5, TickFrequency = 5, SnapsTo = SliderSnapsTo.Ticks };
    readonly TextBlock scoreWords = new() { Style = Ui.Res<Style>("HazeBodyStyle"), HorizontalAlignment = HorizontalAlignment.Right };
    readonly ToggleSwitch obscure = Switch(), licensed = Switch();
    readonly List<(SourceMaterial Source, ToggleButton Chip)> sources = [];
    readonly List<(string Code, ToggleButton Chip)> countries = [];
    readonly WrapRow streaming = Row();
    readonly List<(string Site, ToggleButton Chip)> sites = [];
    readonly StackPanel myListSection;
    readonly RadioButtons myList = new() { MaxColumns = 3 };

    public SearchFiltersPanel(Func<SearchQuery> current, Action<SearchQuery> change, Action close)
    {
        this.current = current;
        this.change = change;
        (var body, scroll, reset, closeButton) = Frame(this, () => this.change(this.current().Cleared()), close);

        // Genres: a click includes, a second leaves out, a third lets go; a right-click offers each.
        body.Children.Add(Section(T.FilterGenres, genres, Ui.Haze(T.FilterHintClick)));

        // Tags: found by name, or opened by category; the rank applies once a tag is picked.
        findTag.TextChanged += (_, _) => ShowTags();
        AutomationProperties.SetName(findTag, T.FilterTagFind);
        tagRank.ValueChanged += (_, e) =>
        {
            tagRankWords.Text = T.FilterTagRank(Wording.Percent(e.NewValue));
            AutomationProperties.SetName(tagRank, tagRankWords.Text);
            if (!showing)
                this.change(this.current() with { MinTagRank = SearchFilters.TagRank(e.NewValue) });
        };
        AutomationProperties.SetName(tagRank, T.FilterTagRank(Wording.Percent(SearchFilters.DefaultTagRank)));
        spoilers.Toggled += (_, _) =>
        {
            // The open groups are made again with their spoiler tags shown or hidden.
            foreach (var expander in tagGroups.Children.OfType<Expander>())
                expander.Content = null;
            tagRows.Clear();
            foreach (var expander in tagGroups.Children.OfType<Expander>().Where(x => x.IsExpanded))
                expander.Content = TagList(((TagGroup)expander.Tag).Tags ?? []);
            ShowTags();
        };
        body.Children.Add(Section(T.FilterTags,
            findTag,
            new StackPanel { Children = { tagRankWords, tagRank } },
            tagGroups,
            tagsFound,
            noTag,
            SwitchRow(T.FilterSpoilerTags, spoilers)));

        body.Children.Add(Section(T.FilterReleased, RangeRow(years, null)));
        years.From.ValueChanged += (_, _) => Ranged(q => q.InYears(SearchFilters.Range(years.From.Value, years.To.Value)));
        years.To.ValueChanged += (_, _) => Ranged(q => q.InYears(SearchFilters.Range(years.From.Value, years.To.Value)));

        var episodeRow = Row();
        foreach (var bounds in SearchFilters.EpisodePresets)
            episodePresets.Add((bounds, Preset(episodeRow, SearchFilters.EpisodesWords(bounds), () => Toggled(q => q.Episodes, bounds, (q, b) => q with { Episodes = b }))));
        var minuteRow = Row();
        foreach (var bounds in SearchFilters.MinutePresets)
            minutePresets.Add((bounds, Preset(minuteRow, SearchFilters.MinutesWords(bounds), () => Toggled(q => q.Minutes, bounds, (q, b) => q with { Minutes = b }))));
        episodes.From.ValueChanged += (_, _) => Ranged(q => q with { Episodes = SearchFilters.Range(episodes.From.Value, episodes.To.Value) });
        episodes.To.ValueChanged += (_, _) => Ranged(q => q with { Episodes = SearchFilters.Range(episodes.From.Value, episodes.To.Value) });
        minutes.From.ValueChanged += (_, _) => Ranged(q => q with { Minutes = SearchFilters.Range(minutes.From.Value, minutes.To.Value) });
        minutes.To.ValueChanged += (_, _) => Ranged(q => q with { Minutes = SearchFilters.Range(minutes.From.Value, minutes.To.Value) });
        body.Children.Add(Section(T.FilterLength,
            Label(T.FilterEpisodes), RangeRow(episodes, null), episodeRow,
            Label(T.FilterEpisodeLength), RangeRow(minutes, T.SearchMinutesUnit), minuteRow));

        // Reception: the least score, from Any, and the obscure titles left out.
        var scoreHead = new Grid();
        scoreHead.Children.Add(new TextBlock { Text = T.FilterScoreAtLeast });
        scoreHead.Children.Add(scoreWords);
        AutomationProperties.SetName(score, T.FilterScoreAtLeast);
        score.ValueChanged += (_, e) =>
        {
            scoreWords.Text = SearchFilters.Score(e.NewValue) is { } s ? Wording.Percent(s) : T.SearchScoreAny;
            if (!showing)
                this.change(this.current() with { MinScore = SearchFilters.Score(e.NewValue) });
        };
        obscure.Toggled += (_, _) => Switched(q => q with { MinPopularity = obscure.IsOn ? SearchFilters.ObscureBelow : null });
        body.Children.Add(Section(T.FilterReception, new StackPanel { Children = { scoreHead, score } }, SwitchRow(T.FilterHideObscure, obscure)));

        // Origin: what a title is based on, and where it was made, in the app's words.
        var sourceRow = Row();
        foreach (var source in Searches.Sources)
            sources.Add((source, Toggle(sourceRow, Searches.SourceName(source), () => this.change(this.current().TogglingSource(source)))));
        var countryRow = Row();
        foreach (var code in Searches.Countries)
            countries.Add((code, Toggle(countryRow, Searches.CountryName(code), () => this.change(this.current().TogglingCountry(code)))));
        body.Children.Add(Section(T.FilterOrigin, Label(T.TitleBasedOn), sourceRow, Label(T.FilterCountry), countryRow));

        // Where to watch: the sites AniList knows a title streams on, and licensed titles only.
        licensed.Toggled += (_, _) => Switched(q => q with { LicensedOnly = licensed.IsOn });
        body.Children.Add(Section(T.FilterWhereToWatch, streaming, SwitchRow(T.FilterLicensedOnly, licensed)));

        // Your list, kept to or left out: only while AniList is signed in, as the engine ignores it otherwise.
        myList.Items.Add(T.FilterShow);
        myList.Items.Add(T.FilterHide);
        myList.Items.Add(T.FilterOnly);
        AutomationProperties.SetName(myList, T.FilterMyList);
        myList.SelectionChanged += (_, _) =>
        {
            if (showing)
                return;
            OnList? pick = myList.SelectedIndex switch { 1 => OnList.Hide, 2 => OnList.Only, _ => null };
            this.change(this.current() with { OnList = pick });
        };
        myListSection = Section(T.FilterMyList, myList);
        body.Children.Add(myListSection);

        body.Children.Add(Ui.Haze(T.FilterLiveNote));

        Facets(ctx.Known.Facets);
    }

    /// <summary>
    /// The panel's frame, which reading's filters panel shares: 340 wide on the ground with a
    /// rule at its left; its name, Reset and the close button pinned above the scrolling filters;
    /// Esc inside it closes it, as on the Mac. Gives the body the sections go in.
    /// </summary>
    internal static (StackPanel Body, ScrollViewer Scroll, Button Reset, Button Close) Frame(Grid panel, Action reset, Action close)
    {
        panel.Background = Ui.Res<Brush>("GroundBrush");
        panel.BorderBrush = Ui.Res<Brush>("FaintRuleBrush");
        panel.BorderThickness = new Thickness(1, 0, 0, 0);
        panel.Width = PanelWidth;
        panel.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        panel.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        var header = new Grid { ColumnSpacing = 8, Padding = new Thickness(16, 20, 12, 8) };
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var title = Ui.Text(T.FiltersTitle, "SubtitleTextBlockStyle", wrap: false);
        title.VerticalAlignment = VerticalAlignment.Center;
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
        header.Children.Add(title);
        var resetButton = new Button { Content = T.ActionReset, VerticalAlignment = VerticalAlignment.Center };
        resetButton.Click += (_, _) => reset();
        SetColumn(resetButton, 1);
        header.Children.Add(resetButton);
        var closeButton = new Button
        {
            Content = new FontIcon { Glyph = "\uE711", FontSize = 12 },
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetName(closeButton, T.FiltersClose);
        ToolTipService.SetToolTip(closeButton, T.TooltipWithKey(T.FiltersClose, "Esc"));
        closeButton.Click += (_, _) => close();
        SetColumn(closeButton, 2);
        header.Children.Add(closeButton);
        panel.Children.Add(header);

        var body = new StackPanel { Spacing = 24, Padding = new Thickness(16, 8, 16, 24) };
        var scroll = new ScrollViewer { Content = body, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollMode = ScrollMode.Disabled };
        SmoothWheel.SetOn(scroll, true);
        SetRow(scroll, 1);
        panel.Children.Add(scroll);

        panel.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Escape && !e.Handled)
            {
                e.Handled = true;
                close();
            }
        };
        return (body, scroll, resetButton, closeButton);
    }

    /// <summary>The first control in the panel, for focus as it opens from the keyboard.</summary>
    public void FocusStart() => closeButton.Focus(FocusState.Keyboard);

#if KAERI_TESTING
    /// <summary>For a screenshot: the lower sections, the panel scrolled to its end, or the tags with the first group open.</summary>
    public void ShowFurther(bool tags)
    {
        if (tags && tagGroups.Children.OfType<Expander>().FirstOrDefault() is { } first)
        {
            first.IsExpanded = true;
            DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () => findTag.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0, AnimationDesired = false }));
            return;
        }
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () => scroll.ChangeView(null, scroll.ScrollableHeight, null, true));
    }
#endif

    /// <summary>The genres, tags and streaming sites to choose from, made again as the engine's list comes.</summary>
    public void Facets(SearchFacets? next)
    {
        facets = next;
        genres.Children.Clear();
        genreChips.Clear();
        foreach (var name in next?.Genres is { Length: > 0 } g ? g : Searches.Genres)
        {
            var genre = name;
            var chip = PickChip(Searches.GenreName(genre), pick => change(current().PickingGenre(genre, pick)));
            genres.Children.Add(chip);
            genreChips.Add((genre, chip));
        }

        tagGroups.Children.Clear();
        tagRows.Clear();
        groupCounts.Clear();
        foreach (var group in next?.TagGroups ?? [])
        {
            var count = new TextBlock { Style = Ui.Res<Style>("HazeBodyStyle"), HorizontalAlignment = HorizontalAlignment.Right };
            var head = new Grid { ColumnSpacing = 8 };
            head.Children.Add(new TextBlock { Text = group.Category, TextTrimming = TextTrimming.CharacterEllipsis });
            head.Children.Add(count);
            var expander = new Expander
            {
                Header = head,
                Tag = group,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(8, 4, 8, 4),
            };
            AutomationProperties.SetName(expander, group.Category);
            // A category's tags are made as it first opens: AniList has hundreds.
            expander.Expanding += (s, _) =>
            {
                if (s.Content is null)
                {
                    s.Content = TagList(((TagGroup)s.Tag).Tags ?? []);
                    ShowTagRows();
                }
            };
            tagGroups.Children.Add(expander);
            groupCounts.Add((group, count));
        }

        streaming.Children.Clear();
        sites.Clear();
        foreach (var site in next?.StreamingSites ?? [])
        {
            var name = site;
            sites.Add((name, Toggle(streaming, name, () => change(current().TogglingStreaming(name)))));
        }
        streaming.Visibility = sites.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        ShowTags();
        Show(current(), force: true);
    }

    /// <summary>Each control as the filters say, changing only what differs.</summary>
    public void Show(SearchQuery filters, bool force = false)
    {
        if (!force && Searches.Same(filters, shown))
            return;
        shown = filters;
        showing = true;
        try
        {
            reset.IsEnabled = filters.HasFilters();
            foreach (var (name, chip) in genreChips)
                ShowPick(chip, filters.GenrePick(name));
            var tagged = filters.Tags.Length + filters.TagsExcluded.Length > 0;
            tagRank.IsEnabled = tagged;
            tagRank.Value = filters.MinTagRank ?? SearchFilters.DefaultTagRank;
            tagRankWords.Text = T.FilterTagRank(Wording.Percent(tagRank.Value));
            AutomationProperties.SetName(tagRank, tagRankWords.Text);
            tagRankWords.Foreground = Ui.Res<Brush>(tagged ? "TextFillColorPrimaryBrush" : "TextFillColorDisabledBrush");
            ShowTagRows();
            ShowRange(years, filters.Years);
            ShowRange(episodes, filters.Episodes);
            ShowRange(minutes, filters.Minutes);
            foreach (var (bounds, chip) in episodePresets)
                chip.IsChecked = filters.Episodes == bounds;
            foreach (var (bounds, chip) in minutePresets)
                chip.IsChecked = filters.Minutes == bounds;
            score.Value = filters.MinScore ?? 0;
            scoreWords.Text = filters.MinScore is { } s ? Wording.Percent(s) : T.SearchScoreAny;
            obscure.IsOn = filters.MinPopularity is not null;
            foreach (var (source, chip) in sources)
                chip.IsChecked = filters.Sources.Contains(source);
            foreach (var (code, chip) in countries)
                chip.IsChecked = filters.Countries.Contains(code);
            foreach (var (site, chip) in sites)
                chip.IsChecked = filters.StreamingOn.Contains(site);
            licensed.IsOn = filters.LicensedOnly;
            myListSection.Visibility = ctx.Known.Anilist is not null || filters.OnList is not null ? Visibility.Visible : Visibility.Collapsed;
            myList.SelectedIndex = filters.OnList switch { OnList.Hide => 1, OnList.Only => 2, _ => 0 };
        }
        finally
        {
            showing = false;
        }
    }

    // The tags.

    /// <summary>The groups while nothing is typed in Find a tag, else the tags found.</summary>
    void ShowTags()
    {
        var words = findTag.Text.Trim();
        tagGroups.Visibility = words.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        // The rows found are made again; the groups' rows stay.
        foreach (var box in tagsFound.Children.OfType<CheckBox>())
            tagRows.RemoveAll(r => r.Box == box);
        tagsFound.Children.Clear();
        var found = SearchFilters.TagsFound(facets, words, spoilers.IsOn);
        if (words.Length > 0)
            foreach (var row in TagRows(found))
                tagsFound.Children.Add(row);
        noTag.Visibility = words.Length > 0 && found.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        ShowTagRows();
    }

    StackPanel TagList(IEnumerable<FacetTag> tags)
    {
        var list = new StackPanel();
        foreach (var row in TagRows(tags.Where(t => spoilers.IsOn || !t.Spoiler)))
            list.Children.Add(row);
        return list;
    }

    /// <summary>
    /// A tag's row: a box that a click moves from off to in to out, AniList's description as its
    /// tooltip, and the pick's word at its end. A right-click offers each choice.
    /// </summary>
    IEnumerable<CheckBox> TagRows(IEnumerable<FacetTag> tags)
    {
        foreach (var tag in tags)
        {
            var name = tag.Name;
            var word = new TextBlock { Style = Ui.Res<Style>("HazeCaptionStyle"), VerticalAlignment = VerticalAlignment.Center };
            var content = new Grid { ColumnSpacing = 8 };
            content.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            content.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            content.Children.Add(new TextBlock { Text = name, TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Center });
            SetColumn(word, 1);
            content.Children.Add(word);
            var box = new CheckBox { Content = content, IsThreeState = true, HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Stretch, Tag = word };
            AutomationProperties.SetName(box, name);
            AutomationProperties.SetHelpText(box, T.FilterHintClick);
            if (tag.Description.Length > 0)
                ToolTipService.SetToolTip(box, tag.Description);
            box.Click += (_, _) => change(current().PickingTag(name, current().TagPick(name).Next()));
            box.ContextFlyout = PickMenu(pick => change(current().PickingTag(name, pick)));
            tagRows.Add((name, box));
            yield return box;
        }
    }

    void ShowTagRows()
    {
        var filters = current();
        foreach (var (name, box) in tagRows)
        {
            var pick = filters.TagPick(name);
            box.IsChecked = pick switch { Pick.Include => true, Pick.Exclude => null, _ => false };
            ((TextBlock)box.Tag).Text = pick.Word() ?? "";
            AutomationProperties.SetItemStatus(box, pick.Word() ?? T.ValueOff);
        }
        foreach (var (group, count) in groupCounts)
        {
            var set = (group.Tags ?? []).Count(t => filters.TagPick(t.Name) != Pick.Off);
            count.Text = set > 0 ? set.ToString(System.Globalization.CultureInfo.CurrentCulture) : "";
        }
    }

    // The pieces.

    /// <summary>
    /// A genre as a chip: a three-state toggle whose own cycle (off, checked, indeterminate) is
    /// off, in, out. In takes the accent fill as a checked toggle does; out takes a vermilion tint
    /// and a minus. A right-click offers each choice.
    /// </summary>
    internal static ToggleButton PickChip(string name, Action<Pick> set)
    {
        var mark = new FontIcon { FontSize = 10, Visibility = Visibility.Collapsed };
        var chip = Chip(new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4, Children = { mark, new TextBlock { Text = name } } });
        chip.IsThreeState = true;
        chip.Tag = mark;
        var tint = new SolidColorBrush(global::Windows.UI.Color.FromArgb(0x24, Ui.Vermilion.R, Ui.Vermilion.G, Ui.Vermilion.B));
        var tintOver = new SolidColorBrush(global::Windows.UI.Color.FromArgb(0x38, Ui.Vermilion.R, Ui.Vermilion.G, Ui.Vermilion.B));
        chip.Resources["ToggleButtonBackgroundIndeterminate"] = tint;
        chip.Resources["ToggleButtonBackgroundIndeterminatePointerOver"] = tintOver;
        chip.Resources["ToggleButtonBackgroundIndeterminatePressed"] = tintOver;
        chip.Resources["ToggleButtonBorderBrushIndeterminate"] = Ui.Res<Brush>("EdgeBrush");
        AutomationProperties.SetName(chip, name);
        AutomationProperties.SetHelpText(chip, T.FilterHintClick);
        chip.Click += (_, _) => set(chip.IsChecked switch { true => Pick.Include, null => Pick.Exclude, _ => Pick.Off });
        chip.ContextFlyout = PickMenu(set);
        return chip;
    }

    internal static void ShowPick(ToggleButton chip, Pick pick)
    {
        chip.IsChecked = pick switch { Pick.Include => true, Pick.Exclude => null, _ => false };
        var mark = (FontIcon)chip.Tag;
        mark.Glyph = pick == Pick.Exclude ? "" : "";
        mark.Visibility = pick == Pick.Off ? Visibility.Collapsed : Visibility.Visible;
        AutomationProperties.SetItemStatus(chip, pick.Word() ?? T.ValueOff);
    }

    static MenuFlyout PickMenu(Action<Pick> set)
    {
        var menu = new MenuFlyout();
        foreach (var (text, pick) in new[] { (T.SearchPickInclude, Pick.Include), (T.SearchPickLeaveOut, Pick.Exclude), (T.ValueOff, Pick.Off) })
        {
            var item = new MenuFlyoutItem { Text = text };
            item.Click += (_, _) => set(pick);
            menu.Items.Add(item);
        }
        return menu;
    }

    /// <summary>A choice that is on or off, as a chip.</summary>
    internal static ToggleButton Toggle(WrapRow row, string name, Action toggle)
    {
        var chip = Chip(new TextBlock { Text = name });
        AutomationProperties.SetName(chip, name);
        chip.Click += (_, _) => toggle();
        row.Children.Add(chip);
        return chip;
    }

    /// <summary>A range picked with one click, in the engine's words; a second click lets it go.</summary>
    static ToggleButton Preset(WrapRow row, string words, Action toggle) => Toggle(row, words, toggle);

    static ToggleButton Chip(UIElement content) => new()
    {
        Content = content,
        CornerRadius = new CornerRadius(14),
        Padding = new Thickness(10, 3, 10, 4),
        MinHeight = 28,
        MinWidth = 0,
        FontSize = 13,
    };

    /// <summary>A preset's range set, or let go when it was the one set.</summary>
    void Toggled(Func<SearchQuery, Bounds> get, Bounds bounds, Func<SearchQuery, Bounds, SearchQuery> with)
    {
        var filters = current();
        change(with(filters, get(filters) == bounds ? Searches.Open : bounds));
    }

    void Ranged(Func<SearchQuery, SearchQuery> with)
    {
        if (!showing)
            change(with(current()));
    }

    void Switched(Func<SearchQuery, SearchQuery> with)
    {
        if (!showing)
            change(with(current()));
    }

    static void ShowRange((NumberBox From, NumberBox To) range, Bounds bounds)
    {
        var from = bounds.Min is { } min ? min : double.NaN;
        var to = bounds.Max is { } max ? max : double.NaN;
        if (!Same(range.From.Value, from))
            range.From.Value = from;
        if (!Same(range.To.Value, to))
            range.To.Value = to;
    }

    static bool Same(double a, double b) => (double.IsNaN(a) && double.IsNaN(b)) || a == b;

    static (NumberBox From, NumberBox To) Range(string name) => (Number(name, T.SearchRangeFrom), Number(name, T.SearchRangeTo));

    static NumberBox Number(string name, string end)
    {
        var box = new NumberBox
        {
            PlaceholderText = end,
            Minimum = 0,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Hidden,
            ValidationMode = NumberBoxValidationMode.InvalidInputOverwritten,
            AcceptsExpression = false,
            NumberFormatter = new global::Windows.Globalization.NumberFormatting.DecimalFormatter
            {
                IsGrouped = false,
                FractionDigits = 0,
                NumberRounder = new global::Windows.Globalization.NumberFormatting.IncrementNumberRounder { Increment = 1 },
            },
            Value = double.NaN,
        };
        AutomationProperties.SetName(box, $"{name}, {end}");
        return box;
    }

    /// <summary>From and to side by side, either open, and the unit after them when there is one.</summary>
    static Grid RangeRow((NumberBox From, NumberBox To) range, string? unit)
    {
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(range.From);
        SetColumn(range.To, 1);
        row.Children.Add(range.To);
        if (unit is not null)
        {
            var words = Ui.Haze(unit, wrap: false);
            words.VerticalAlignment = VerticalAlignment.Center;
            SetColumn(words, 2);
            row.Children.Add(words);
        }
        return row;
    }

    internal static ToggleSwitch Switch() => new() { OnContent = "", OffContent = "", MinWidth = 0 };

    /// <summary>A switch at the end of its row, its words at the start.</summary>
    internal static Grid SwitchRow(string text, ToggleSwitch toggle)
    {
        var row = new Grid { ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = Ui.Text(text);
        words.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(words);
        AutomationProperties.SetName(toggle, text);
        toggle.HorizontalAlignment = HorizontalAlignment.Right;
        SetColumn(toggle, 1);
        row.Children.Add(toggle);
        return row;
    }

    internal static WrapRow Row() => new() { Gap = 6 };

    internal static TextBlock Label(string text) => Ui.Haze(text);

    /// <summary>A section: its heading, then what it holds.</summary>
    internal static StackPanel Section(string heading, params UIElement[] children)
    {
        var section = new StackPanel { Spacing = 10 };
        var words = Ui.Text(heading, "BodyStrongTextBlockStyle");
        AutomationProperties.SetHeadingLevel(words, AutomationHeadingLevel.Level3);
        section.Children.Add(words);
        foreach (var child in children)
            section.Children.Add(child);
        return section;
    }
}
