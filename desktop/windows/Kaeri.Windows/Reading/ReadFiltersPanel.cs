using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Kaeri.Windows.Pages;

using Pick = Kaeri.Engine.Model.Pick;

/// <summary>
/// Reading Search's filters panel, in the shows' panel frame with its sections and chips: Your
/// sources only first while words are typed, then the genres (a click includes, a second leaves
/// out, a third lets go), Started, Origin, Format and Status. Each change searches as it is made;
/// there is nothing to apply. <see cref="Show"/> sets each control to what the query says.
/// </summary>
sealed partial class ReadFiltersPanel : Grid
{
    readonly Func<ReadSearchQuery> current;
    readonly Action<ReadSearchQuery> change;
    readonly Button reset, closeButton;
    readonly StackPanel sourcesSection;
    readonly ToggleSwitch sourcesOnly = SearchFiltersPanel.Switch();
    readonly List<(string Name, ToggleButton Chip)> genres = [];
    readonly List<(string Key, ToggleButton Chip)> started = [], origins = [], formats = [], statuses = [];
    /// <summary>Set while the controls take what the query says, so their events change nothing.</summary>
    bool showing;

    public ReadFiltersPanel(ReadFilters f, Func<ReadSearchQuery> current, Action<ReadSearchQuery> change, Action close)
    {
        this.current = current;
        this.change = change;
        (var body, _, reset, closeButton) = SearchFiltersPanel.Frame(this, () => this.change(Reset(this.current())), close);

        // Your sources only, while words are typed: AniList is left out.
        sourcesOnly.Toggled += (_, _) =>
        {
            if (!showing)
                this.change(this.current() with { SourcesOnly = sourcesOnly.IsOn });
        };
        sourcesSection = new StackPanel { Children = { SearchFiltersPanel.SwitchRow(T.ReadSearchSourcesOnly, sourcesOnly) } };
        body.Children.Add(sourcesSection);

        var genreRow = SearchFiltersPanel.Row();
        foreach (var name in f.Genres)
        {
            var genre = name;
            var chip = SearchFiltersPanel.PickChip(Searches.GenreName(genre), pick => this.change(Picking(this.current(), genre, pick)));
            genreRow.Children.Add(chip);
            genres.Add((genre, chip));
        }
        body.Children.Add(SearchFiltersPanel.Section(T.FilterGenres, genreRow, Ui.Haze(T.FilterHintClick)));

        // Started: one at a time; a second click lets it go.
        var startedRow = SearchFiltersPanel.Row();
        foreach (var choice in (f.Started ?? []).Where(c => c.Key != AnyTime))
        {
            var key = choice.Key;
            started.Add((key, SearchFiltersPanel.Toggle(startedRow, choice.Label.Text(), () =>
                this.change(this.current() with { Started = this.current().Started == key ? null : key }))));
        }
        body.Children.Add(SearchFiltersPanel.Section(T.ReadSearchStarted, startedRow));

        var originRow = SearchFiltersPanel.Row();
        foreach (var choice in f.Origins)
        {
            var key = choice.Key;
            origins.Add((key, SearchFiltersPanel.Toggle(originRow, choice.Label.Text(), () =>
                this.change(this.current() with { Origins = Toggled(this.current().Origins, key) }))));
        }
        body.Children.Add(SearchFiltersPanel.Section(T.FilterOrigin, originRow));

        // One Manga, which takes one-shots in.
        var formatRow = SearchFiltersPanel.Row();
        var apart = ReadDiscovery.FormatChoices(f.Formats).Length != f.Formats.Length;
        foreach (var choice in ReadDiscovery.FormatChoices(f.Formats))
        {
            var key = choice.Key;
            formats.Add((key, SearchFiltersPanel.Toggle(formatRow, choice.Label.Text(), () =>
                this.change(this.current() with { Formats = ReadDiscovery.ToggleFormat(this.current().Formats, key, apart) }))));
        }
        body.Children.Add(SearchFiltersPanel.Section(T.FilterFormat, formatRow));

        // Status: one at a time; a second click lets it go.
        var statusRow = SearchFiltersPanel.Row();
        foreach (var choice in f.Statuses)
        {
            var key = choice.Key;
            statuses.Add((key, SearchFiltersPanel.Toggle(statusRow, choice.Label.Text(), () =>
                this.change(this.current() with { Status = this.current().Status == key ? null : key }))));
        }
        body.Children.Add(SearchFiltersPanel.Section(T.FilterStatus, statusRow));

        body.Children.Add(Ui.Haze(T.FilterLiveNote));
    }

    // i18n-skip: the engine's key for any time
    const string AnyTime = "any";

    /// <summary>The first control in the panel, for focus as it opens from the keyboard.</summary>
    public void FocusStart() => closeButton.Focus(FocusState.Keyboard);

    /// <summary>Each control as the query says; Your sources only shows only while words are typed.</summary>
    public void Show(ReadSearchQuery q, bool words)
    {
        showing = true;
        try
        {
            reset.IsEnabled = ReadSearchPage.AnyFilter(q) || q.SourcesOnly;
            sourcesSection.Visibility = words ? Visibility.Visible : Visibility.Collapsed;
            sourcesOnly.IsOn = q.SourcesOnly;
            foreach (var (name, chip) in genres)
                SearchFiltersPanel.ShowPick(chip, (q.Genres ?? []).Contains(name) ? Pick.Include : (q.ExcludeGenres ?? []).Contains(name) ? Pick.Exclude : Pick.Off);
            var startedKey = ReadDiscovery.StartedSet(q) ? q.Started : null;
            foreach (var (key, chip) in started)
                chip.IsChecked = key == startedKey;
            foreach (var (key, chip) in origins)
                chip.IsChecked = (q.Origins ?? []).Contains(key);
            var folded = ReadDiscovery.FoldedFormats(q.Formats);
            foreach (var (key, chip) in formats)
                chip.IsChecked = folded.Contains(key);
            foreach (var (key, chip) in statuses)
                chip.IsChecked = key == q.Status;
        }
        finally
        {
            showing = false;
        }
    }

    /// <summary>Reset: every filter off, the words and the sort kept, as the shows' Reset does.</summary>
    static ReadSearchQuery Reset(ReadSearchQuery q) => ReadingStore.EmptyQuery with { Text = q.Text, Sort = q.Sort };

    /// <summary>A genre included, left out or let go.</summary>
    internal static ReadSearchQuery Picking(ReadSearchQuery q, string genre, Pick pick)
    {
        var with = (q.Genres ?? []).Where(g => g != genre).ToList();
        var without = (q.ExcludeGenres ?? []).Where(g => g != genre).ToList();
        if (pick == Pick.Include)
            with.Add(genre);
        else if (pick == Pick.Exclude)
            without.Add(genre);
        return q with { Genres = [.. with], ExcludeGenres = [.. without] };
    }

    static string[] Toggled(string[]? keys, string key)
    {
        var list = keys ?? [];
        return list.Contains(key) ? [.. list.Where(k => k != key)] : [.. list, key];
    }
}
