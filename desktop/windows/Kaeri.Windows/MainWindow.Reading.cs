using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;

namespace Kaeri.Windows;

// Reading in the window: the sidebar by what Kaeri is for, the places that differ by medium, the
// series page and the reader over everything. With watching alone the sidebar is as it always
// was; with reading alone Read is Home; with both, Watch and Read lead their own groups and
// Search, Library and Downloads are shared, each with a Shows or Reading switch.
public sealed partial class MainWindow
{
    /// <summary>With both media on: whether Library and Search show reading. Each keeps its own.</summary>
    internal bool LibraryReading { get; set; }
    internal bool SearchReading { get; set; }

    KaeriIsFor shownAnswer = KaeriIsFor.Watch;
    bool askingMedium;

    KaeriIsFor Answer => ctx?.Known.Medium.Answer ?? KaeriIsFor.Watch;

    /// <summary>The sidebar's places as shown now, in order: Ctrl+1 to Ctrl+9 follow them.</summary>
    IEnumerable<NavigationViewItem> Places() =>
        Nav.MenuItems.OfType<NavigationViewItem>().Where(i => i.Visibility == Visibility.Visible);

    /// <summary>The page a sidebar tag opens: Library, Search and Downloads follow the medium.</summary>
    Type? PageFor(string tag)
    {
        var answer = Answer;
        return tag switch
        {
            "library" when answer == KaeriIsFor.Read || (answer == KaeriIsFor.Both && LibraryReading) => typeof(ReadLibraryPage),
            "search" when answer == KaeriIsFor.Read || (answer == KaeriIsFor.Both && SearchReading) => typeof(ReadSearchPage),
            // Downloads with both shows reading's section first on the usual page.
            "downloads" when answer == KaeriIsFor.Read => typeof(ReadDownloadsPage),
            "home" when answer == KaeriIsFor.Read => typeof(ReadHomePage),
            _ => Destinations.TryGetValue(tag, out var page) ? page : null,
        };
    }

    /// <summary>The sidebar tag a page belongs to, for its selection.</summary>
    static string? TagOf(Type page) =>
        page == typeof(ReadLibraryPage) ? "library"
        : page == typeof(ReadSearchPage) ? "search"
        : page == typeof(ReadDownloadsPage) ? "downloads"
        : page == typeof(SeriesPage) ? null
        : Destinations.FirstOrDefault(d => d.Value == page && d.Key != "continue").Key;

    /// <summary>
    /// Shows the places of the answer in force: the groups, Watch and Read named so with both and
    /// Home with one, the Sources heading with reading on, and the tips with each place's key.
    /// A page that has left the sidebar gives way to Home's stand-in.
    /// </summary>
    internal void ApplyMedium()
    {
        var answer = Answer;
        var watches = answer != KaeriIsFor.Read;
        var reads = answer != KaeriIsFor.Watch;
        var both = answer == KaeriIsFor.Both;
        Visibility Shown(bool on) => on ? Visibility.Visible : Visibility.Collapsed;
        HomeItem.Visibility = CalendarItem.Visibility = SeasonalItem.Visibility = TogetherItem.Visibility = Shown(watches);
        ReadItem.Visibility = UpdatesItem.Visibility = Shown(reads);
        ReadSeparator.Visibility = SharedSeparator.Visibility = Shown(both);
        HomeItem.Content = both ? T.NavWatch : T.HomeTitle;
        ReadItem.Content = both ? T.NavRead : T.HomeTitle;
        ReadGlyph.Glyph = both ? "" : "";
        if (HomeItem.Icon is FontIcon homeGlyph)
            homeGlyph.Glyph = both ? "" : "";
        // With reading on, the board heads the sources "Sources" and names them each.
        SourcesHeader.Visibility = Shown(reads);
        ProvidersItem.Content = reads ? T.SourcesYour : T.SourcesTitle;
        foreach (var (item, n) in Places().Select((item, i) => (item, i)))
        {
            var name = item.Content as string ?? "";
            ToolTipService.SetToolTip(item, n < 9 ? T.TooltipWithKey(name, $"Ctrl+{n + 1}") : name);
        }
        var changed = shownAnswer != answer;
        shownAnswer = answer;
        if (!changed || ctx is null)
            return;
        // The page on screen may belong to the medium that left, or show the other's scope.
        var page = ContentFrame.CurrentSourcePageType;
        if (page is null)
            return;
        var tag = TagOf(page);
        if (tag is "home" or "calendar" or "seasonal" or "together" && !watches)
            Navigate("read");
        else if (tag is "read" or "updates" && !reads)
            Navigate("home");
        else if (tag is "library" or "search" or "downloads" && PageFor(tag) != page)
            Navigate(tag);
        else if (tag == "read" && !both && page == typeof(ReadHomePage))
            OnFrameNavigatedSelection();
    }

    /// <summary>The selection again after a rename, without navigating.</summary>
    void OnFrameNavigatedSelection()
    {
        selecting = true;
        if (ContentFrame.CurrentSourcePageType is { } page && TagOf(page) is { } tag)
            Nav.SelectedItem = Places().FirstOrDefault(i => (string?)i.Tag == tag || (tag == "home" && (string?)i.Tag == "read" && Answer == KaeriIsFor.Read));
        selecting = false;
    }

    /// <summary>Library or Search's Shows or Reading switch, with both media on: the page swaps in place.</summary>
    internal void SetScope(string tag, bool reading)
    {
        if (tag == "library")
            LibraryReading = reading;
        else if (tag == "search")
            SearchReading = reading;
        else
            return;
        if (PageFor(tag) is not { } page || ContentFrame.CurrentSourcePageType == page)
            return;
        object? parameter = tag == "search" && SearchBox.Text.Trim() is { Length: > 0 } text ? text : null;
        Crossfade.Swap(ContentFrame, () =>
        {
            ContentFrame.Navigate(page, parameter, new SuppressNavigationTransitionInfo());
            // The other scope is the same place, not a step to go back to.
            if (ContentFrame.BackStack.Count > 0)
                ContentFrame.BackStack.RemoveAt(ContentFrame.BackStack.Count - 1);
        });
    }

    /// <summary>A series' page: drill-in, as a title's page opens.</summary>
    internal void OpenSeries(string work)
    {
        if (ReaderLayer.Showing)
            ReaderLayer.Close();
        if (Player.Showing)
            Player.Close();
        if (Motion.Enabled)
            ContentFrame.Navigate(typeof(SeriesPage), work, new DrillInNavigationTransitionInfo());
        else
        {
            ContentFrame.Navigate(typeof(SeriesPage), work, new SuppressNavigationTransitionInfo());
            Motion.FadeIn(ContentFrame, Motion.Standard);
        }
    }

    /// <summary>The reader over the window, at a chapter or where the person stopped.</summary>
    internal void OpenReader(string work, string? chapter = null, OpenAt? at = null)
    {
        if (Player.Showing)
            Player.Close();
        HoverPreview.Close();
        ReaderLayer.Open(work, chapter, at);
    }

    /// <summary>
    /// The engine asks what Kaeri is for: after an update that brought reading, or when this PC
    /// joins devices that answered otherwise. First run asks on its own page instead.
    /// </summary>
    void HearMedium()
    {
        ApplyMedium();
        if (ctx.Known.Medium.Asking is not { } ask || ask is MediumAsk.FirstRun || askingMedium || welcoming is not null)
            return;
        askingMedium = true;
        ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
        {
            try
            {
                await MediumDialogs.Ask(ctx, Root.XamlRoot, ask);
            }
            finally
            {
                askingMedium = false;
            }
        });
    }

    /// <summary>Reading's start: the medium, and what the testing build's options open.</summary>
    void StartReading()
    {
        ctx.Known.Medium.Changed += HearMedium;
        // What came in after a book source was added, heard from the start.
        CameInDesk.Ensure(ctx);
        ctx.Known.Medium.Load();
#if KAERI_TESTING
        if (options.Work is { } work)
            ui.TryEnqueue(DispatcherQueuePriority.Low, () => OpenSeries(work));
        if (options.Read is { } read)
        {
            var parts = read.Split(',', 2);
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(400);
                OpenReader(parts[0], parts.Length > 1 ? parts[1] : null);
            });
        }
#endif
    }
}
