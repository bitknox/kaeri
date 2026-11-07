using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// One folder of Your media: its path and state, Rescan, Show in Explorer and Remove folder, then
/// the lists Needs a look, Not matched and Matched. Choosing a match opens the fix dialog.
/// </summary>
public sealed partial class FolderPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string path = "";
    bool matchedOpen;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        path = e.Parameter as string ?? "";
        ctx.Hear(this, batch =>
        {
            if (batch.Any(x => x is Event.Local))
                Draw();
        });
        ctx.Send(new Command.Local(new LocalCommand.Load()));
        Draw();
        countdown ??= Countdown.Start(this, () => ctx.Known.AnyWaiting(DateTimeOffset.UtcNow), Draw);
    }

    Microsoft.UI.Dispatching.DispatcherQueueTimer? countdown;

    public FolderPage()
    {
        InitializeComponent();
        State.Placeholders = Placeholders;
        // The folders and what was found in them are on this device.
        State.NeedsConnection = false;
    }

    /// <summary>The folder's state line, its three actions and a list of groups, as plates.</summary>
    static UIElement Placeholders()
    {
        var plates = new StackPanel { Spacing = 12 };
        plates.Children.Add(States.Plate(240, 14, 3));
        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        for (var i = 0; i < 3; i++)
            actions.Children.Add(States.Plate(110, 32, 4));
        plates.Children.Add(actions);
        plates.Children.Add(States.Plate(320, 10, 3));
        plates.Children.Add(States.Heading());
        plates.Children.Add(States.Rows(3, 0, 0, rowHeight: 52));
        return plates;
    }

    void Draw()
    {
        Head.Children.Clear();
        Stack.Children.Clear();
        var view = ctx.Known.Local;
        var folder = view?.Folders.FirstOrDefault(f => f.Path == path);
        var title = Ui.Text(Path.GetFileName(path.TrimEnd('\\', '/')) is { Length: > 0 } name ? name : path, "PageTitleStyle");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        Head.Children.Add(title);
        Head.Children.Add(Ui.Haze(path, wrap: true));
        if (view is null)
        {
            State.Wait();
            return;
        }
        State.Done();
        if (folder is null)
        {
            // Removed while the page was up.
            Stack.Children.Add(Ui.Text(T.LibraryRowMissing, "BodyTextBlockStyle"));
            return;
        }
        var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (!folder.Available)
            state.Children.Add(Ui.Dot(Colours.Haze, ring: true));
        state.Children.Add(Ui.Text(Lawful.FolderLine(folder, ctx.Known, DateTimeOffset.UtcNow), "BodyTextBlockStyle"));
        Stack.Children.Add(state);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        actions.Children.Add(Ui.Label(T.LibraryRescan, (_, _) => ctx.Send(new Command.Local(new LocalCommand.Scan()))));
        actions.Children.Add(Ui.Label(T.LibraryShowExplorer, (_, _) => ShowInExplorer(), accent: false));
        actions.Children.Add(Ui.Label(T.LibraryRemoveFolder, (_, _) => Remove()));
        Stack.Children.Add(actions);
        Stack.Children.Add(Ui.Haze(T.LibraryRemoveDetail));

        // Each folder lists its own groups; with one folder the engine may leave the folder blank.
        var own = view.Groups.Where(g => g.Folder.Length == 0 || g.Folder == path).ToArray();
        Section(T.LibraryNeedsLook, own.Where(g => g.State == GroupState.NeedsLook), choose: true, keep: false);
        var unnumbered = view.Unnumbered.Where(u => u.Folder.Length == 0 || u.Folder == path).ToArray();
        Section(T.LibraryNotMatched, own.Where(g => g.State == GroupState.NotMatched), choose: true, keep: true, unnumbered);
        var matched = own.Where(g => g.State is GroupState.Matched or GroupState.Kept).ToArray();
        if (matched.Length > 0)
        {
            var open = new HyperlinkButton { Content = T.LibraryMatched(matched.Length), Style = Ui.Res<Style>("SilkLinkLeadingStyle"), Margin = new Thickness(-4, 12, 0, 0) };
            open.Click += (_, _) => { matchedOpen = !matchedOpen; Draw(); };
            Stack.Children.Add(open);
            if (matchedOpen)
                foreach (var group in matched)
                    Stack.Children.Add(Row(group, choose: false, keep: false, fix: true));
        }
    }

    void Section(string heading, IEnumerable<LocalGroup> groups, bool choose, bool keep, UnnumberedFile[]? unnumbered = null)
    {
        var list = groups.ToArray();
        if (list.Length == 0 && unnumbered is not { Length: > 0 })
            return;
        Stack.Children.Add(Ui.Heading(heading));
        foreach (var group in list)
            Stack.Children.Add(Row(group, choose, keep, fix: false));
        // Files whose names have no episode number wait here until Set episode.
        foreach (var file in unnumbered ?? [])
            Stack.Children.Add(Unnumbered(file));
    }

    UIElement Unnumbered(UnnumberedFile file)
    {
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 52, Padding = new Thickness(0, 6, 0, 6) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(file.Name, "BodyStrongTextBlockStyle", wrap: false));
        var line = T.LibraryReasonNoEpisode;
        if (file.Place.Length > 0)
            line = $"{file.Place} · {line}";
        text.Children.Add(Ui.Text(line, "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var set = Ui.Label(T.LibrarySetEpisode, async (_, _) => await new SetEpisodeDialog(ctx, XamlRoot, file).ShowAsync());
        set.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(set, 1);
        grid.Children.Add(set);
        return grid;
    }

    UIElement Row(LocalGroup group, bool choose, bool keep, bool fix)
    {
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 52, Padding = new Thickness(0, 6, 0, 6) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(group.MatchedTitle is { Length: > 0 } t && group.State is GroupState.Matched ? t : group.Name, "BodyStrongTextBlockStyle", wrap: false));
        var line = T.LibraryFiles(group.Files);
        // A best guess says how sure it is: "Best guess: Title · 71% match".
        if (group.State == GroupState.NeedsLook && group.MatchedTitle is { Length: > 0 } guess && group.Confidence is { } percent)
            line = $"{T.LibraryBestGuess(guess, percent)} · {line}";
        if (group.State == GroupState.NotMatched)
            line = $"{Lawful.Reason(group.Reason)} · {line}";
        text.Children.Add(Ui.Text(line, "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        if (choose || fix)
            buttons.Children.Add(Ui.Label(fix ? T.MatchFix : T.MatchChoose, (_, _) => Fix(group)));
        if (keep)
        {
            var more = new Button { Content = new FontIcon { Glyph = "", FontSize = 14 }, Width = 36, Height = 32, Padding = new Thickness(0) };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(more, T.ActionMore);
            var menu = new MenuFlyout();
            var item = new MenuFlyoutItem { Text = T.LibraryKeepUnmatched };
            item.Click += (_, _) => ctx.Send(new Command.Local(new LocalCommand.Keep(group.Id)));
            menu.Items.Add(item);
            more.Flyout = menu;
            buttons.Children.Add(more);
        }
        Grid.SetColumn(buttons, 1);
        grid.Children.Add(buttons);
        return grid;
    }

    async void Fix(LocalGroup group)
    {
        var dialog = new FixMatchDialog(ctx, XamlRoot, group);
        await dialog.ShowAsync();
    }

    void ShowInExplorer()
    {
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("explorer.exe", $"\"{path}\"") { UseShellExecute = true });
        }
        catch (Exception)
        {
            // No Explorer to show it in: the path is on the page.
        }
    }

    void Remove()
    {
        ctx.Send(new Command.Local(new LocalCommand.RemoveFolder(path)));
        // The engine holds what it found in the folder for the notice's seconds; Undo brings it back as it was.
        var back = path;
        ctx.Window.Say(T.SourcesRemoved(Path.GetFileName(path.TrimEnd('\\', '/')) is { Length: > 0 } n ? n : path) + " " + T.SourcesRemovedFilesStay,
            T.ActionUndo, () => ctx.Send(new Command.Local(new LocalCommand.RestoreFolder(back))));
        ctx.LeaveRemoved();
    }
}
