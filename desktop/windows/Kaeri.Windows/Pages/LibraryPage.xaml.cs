using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Library: the tracker's five lists, Watching among them, each a poster grid with its count, ordered by last updated,
/// title or score. Signed out, Nagare and Connect AniList.
/// </summary>
public sealed partial class LibraryPage : Page
{
    static readonly (LibrarySegment Segment, string Name)[] Segments =
    [
        (LibrarySegment.Watching, T.LibraryStatusWatching),
        (LibrarySegment.Planning, T.LibraryStatusPlanning),
        (LibrarySegment.Completed, T.LibraryStatusCompleted),
        (LibrarySegment.Paused, T.LibraryStatusPaused),
        (LibrarySegment.Dropped, T.LibraryStatusDropped),
    ];

    readonly Ctx ctx = Ctx.Current;
    LibrarySegment segment = LibrarySegment.Watching;
    string order = "updated";
    readonly SurpriseButton surprise = new(() => new SurprisePool.Planning(), T.SurpriseHintPlanning);

    public LibraryPage()
    {
        InitializeComponent();
        SurpriseHost.Content = surprise;
        SignedOut.Child = States.Empty(T.LibrarySignedOutTitle, T.LibrarySignedOutDetail, T.LibrarySignedOutAction, () => ctx.SignIn("anilist"));
        if (SignedOut.Child is StackPanel panel && panel.Children[^1] is Button signIn)
            signIn.Style = Ui.Res<Style>("AccentButtonStyle");
        State.Placeholders = () => States.Covers();
        State.ActivityText = T.LibraryLoading;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(OrderButton, T.LibrarySortLabel(T.LibrarySortUpdated));
        foreach (var (s, name) in Segments)
            Lists.Items.Add(Tab(s, name));
        // The frame keeps the page for Back and the next visit: the list, the sort and the scroll stay.
        NavigationCacheMode = NavigationCacheMode.Enabled;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        // Heard from each visit's start, before anything is asked, until the frame moves on.
        ctx.Hear(this, Heard);
        foreach (var (s, _) in Segments)
            ctx.Send(new Command.LoadLibrary(s));
        signedIn = ctx.Known.Anilist is not null;
        Show();
    }

    bool signedIn;

    void Heard(IReadOnlyList<Event> batch)
    {
        surprise.Heard(batch);
        // Signed in here: the lists asked for while signed out came back empty, so they are asked again.
        var now = ctx.Known.Anilist is not null;
        if (now && !signedIn)
            foreach (var (s, _) in Segments)
                ctx.Send(new Command.LoadLibrary(s));
        signedIn = now;
        if (batch.Any(e => e is Event.Content { View: ContentFor.Library } or Event.Sync or Event.Progress))
            Show();
    }

    void Show()
    {
        var signedIn = ctx.Known.Anilist is not null;
        SignedOut.Visibility = signedIn ? Visibility.Collapsed : Visibility.Visible;
        Bar.Visibility = signedIn ? Visibility.Visible : Visibility.Collapsed;
        Scroll.Visibility = signedIn ? Visibility.Visible : Visibility.Collapsed;
        if (!signedIn)
            return;
        SurpriseHost.Visibility = segment == LibrarySegment.Planning ? Visibility.Visible : Visibility.Collapsed;
        foreach (var item in Lists.Items)
        {
            var s = (LibrarySegment)item.Tag;
            var name = Segments.First(x => x.Segment == s).Name;
            var count = ctx.Known.Lists.TryGetValue(new ContentFor.Library(s), out var list)
                ? list switch { ContentList.Entries e => e.V1.Length, ContentList.Empty => 0, _ => (int?)null }
                : null;
            var text = count is { } n ? T.LibraryListCount(name, n) : name;
            if (item.Child is Grid { Children: [TextBlock label, ..] } && label.Text != text)
            {
                label.Text = text;
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, text);
            }
        }
        IEnumerable<Entry> entries = ctx.Known.ListEntries(new ContentFor.Library(segment));
        entries = order switch
        {
            "title" => entries.OrderBy(e => e.Title, StringComparer.CurrentCultureIgnoreCase),
            "score" => entries.OrderByDescending(e => e.Score ?? 0),
            _ => entries,
        };
        var covers = entries.ToList();
        Grid.Show(covers);
        ctx.Known.Lists.TryGetValue(new ContentFor.Library(segment), out var shown);
        if (covers.Count > 0)
            State.Done();
        else if (shown is ContentList.Failed f)
            State.Failed(f.V1, () => ctx.Send(new Command.LoadLibrary(segment)));
        else if (shown is ContentList.Entries or ContentList.Empty)
            State.Empty(States.Unavailable(T.LibraryListEmpty(Segments.First(x => x.Segment == segment).Name.ToLower(System.Globalization.CultureInfo.CurrentCulture)), T.LibraryListEmptyDetail));
        else
            // The lists come from this device's store, usually within a frame or two: the
            // usual 600 ms of silence, so a quick answer is one change, not plates then covers.
            State.Wait();
    }

    /// <summary>
    /// A list's tab. Its label sits over the same label with a three-figure count, unseen, so the
    /// tab is as wide before its count arrives as after, and the tabs beside it never move.
    /// </summary>
    SelectorBarItem Tab(LibrarySegment s, string name)
    {
        var room = new TextBlock { Text = T.LibraryListCount(name, 888), Opacity = 0 };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(room, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        // A tab made from a Child gets none of the 12 px inset a tab made from Text has: given here,
        // so these words start where Seasonal's and Calendar's do, on the page title's edge.
        var label = new Grid { Margin = new Thickness(12, 0, 12, 0), Children = { new TextBlock { Text = name }, room } };
        var item = new SelectorBarItem { Child = label, Tag = s, IsSelected = s == segment };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, name);
        return item;
    }

    void OnListChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs e)
    {
        if (sender.SelectedItem?.Tag is LibrarySegment s)
        {
            if (s == segment)
                return;
            segment = s;
            // Another list starts at its top, not part way down where the last one was.
            Scroll.ChangeView(null, 0, null, true);
            Show();
        }
    }

    void OnOrder(object sender, RoutedEventArgs e)
    {
        if (sender is RadioMenuFlyoutItem item)
        {
            order = (string)item.Tag;
            OrderButton.Content = item.Text;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(OrderButton, T.LibrarySortLabel(item.Text));
            Show();
        }
    }

}
