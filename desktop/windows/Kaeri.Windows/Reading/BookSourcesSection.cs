using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using FoundServer = Kaeri.Engine.Model.ServerFound;

namespace Kaeri.Windows.Pages;

/// <summary>The words book sources share: a server's kind, a Kavita link, a folder's skipped files.</summary>
static class BookWords
{
    /// <summary>Komga, Kavita, or OPDS catalogue.</summary>
    public static string Kind(BookServerKind kind) => kind switch
    {
        BookServerKind.Komga => "Komga", // i18n-skip: a product's name
        BookServerKind.Kavita => "Kavita", // i18n-skip: a product's name
        _ => T.BookSourcesOpds,
    };

    /// <summary>
    /// A pasted Kavita OPDS link, <c>https://host/api/opds/{key}</c>: its host and the key's last
    /// four characters. The key signs in, so no user or password is asked.
    /// </summary>
    public static (string Host, string Last4)? KavitaLink(string address)
    {
        if (!Uri.TryCreate(address.Trim(), UriKind.Absolute, out var url) || url.Scheme is not ("http" or "https") || url.Host.Length == 0)
            return null;
        var parts = url.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries);
        var at = Array.IndexOf(parts, "opds");
        if (at < 1 || parts[at - 1] != "api" || at + 1 >= parts.Length || parts[at + 1].Length < 4)
            return null;
        return (url.Host, parts[at + 1][^4..]);
    }

    /// <summary>"14 MOBI, 6 AZW3, 3 EPUB locked with DRM".</summary>
    public static string Kinds(FolderReport report)
    {
        var parts = report.Unsupported.Select(f => $"{f.Count} {f.Format}").ToList(); // i18n-skip: a count and a format's name
        var locked = report.Locked.Sum(f => (long)f.Count);
        if (locked > 0)
            parts.Add(T.BookFolderDrm(locked));
        return string.Join(", ", parts); // i18n-skip
    }

    public static long Skipped(FolderReport report) => report.Unsupported.Concat(report.Locked).Sum(f => (long)f.Count);

    /// <summary>A folder's line: where it is, then scanning, missing, or its series and skipped files.</summary>
    public static string FolderLine(BookFolderRow folder)
    {
        var path = folder.Path ?? folder.Name;
        return folder.Scan switch
        {
            BookFolderScan.Scanning scanning => ReadingParts.Joined(path, T.BookSourcesScanning(scanning.Found)),
            BookFolderScan.Missing => ReadingParts.Joined(path, T.BookSourcesMissing),
            _ when folder.Skipped > 0 => T.SourceRowFolder(path, folder.Series, folder.Skipped),
            _ => ReadingParts.Joined(path, T.BookSourcesSeries(folder.Series)),
        };
    }

    /// <summary>A server's line: signed out, empty, or its kind, address, series and progress.</summary>
    public static string ServerLine(BookServerRow server)
    {
        if (server.SignedOut)
            return ReadingParts.Joined(Kind(server.Kind), T.ServersRowSignedOut);
        if (server.Empty)
            return T.SourceRowServerEmpty(Kind(server.Kind));
        return ReadingParts.Joined(
            Kind(server.Kind),
            server.Address != server.Name ? server.Address : null,
            T.BookSourcesSeries(server.Series),
            server.KeyLast4 is { } last ? T.BookServerKey(last) : null,
            server.Off ? T.BookSourcesOff : null,
            server.ReportProgress && !server.Off ? T.BookSourcesProgressBack : null);
    }

    /// <summary>A plugin's line: its store, its version, and Off.</summary>
    public static string PluginLine(Ctx ctx, BookPluginRow plugin) => ReadingParts.Joined(
        plugin.Store is { } store ? T.BookSourcesFromStore(ctx.Known.Stores.FirstOrDefault(s => s.Id == store)?.Name ?? store) : null,
        T.BookSourcesVersion(plugin.Version),
        plugin.Off ? T.BookSourcesOff : null);

    /// <summary>Series in folders whose match needs a look: no match, and not kept unmatched.</summary>
    public static BookGroupRow[] Unmatched(BookSourcesView? view) =>
        view is null ? [] : [.. view.Groups.Where(g => g.Work is null && !g.Kept)];

    /// <summary>A note in a came-in dialog: a glyph by kind, a title, and a line.</summary>
    public static StackPanel Note(string title, string? line, bool failed = false, bool found = false)
    {
        var note = new StackPanel { Spacing = 2 };
        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (failed)
            head.Children.Add(States.ErrorGlyph());
        else if (found)
            head.Children.Add(new FontIcon { Glyph = "", FontSize = 14, Foreground = Ui.Res<Brush>("WaterBrush"), VerticalAlignment = VerticalAlignment.Center });
        var text = Ui.Text(title, "BodyStrongTextBlockStyle");
        head.Children.Add(text);
        note.Children.Add(head);
        if (line is { Length: > 0 })
        {
            var under = Ui.Haze(line);
            under.Margin = new Thickness(failed || found ? 22 : 0, 0, 0, 0);
            note.Children.Add(under);
        }
        AutomationProperties.SetLiveSetting(note, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        return note;
    }

    /// <summary>A rounded square with a glyph, before a row's words.</summary>
    public static Border Glyph(string glyph) => new()
    {
        Width = 32,
        Height = 32,
        CornerRadius = new CornerRadius(8),
        Background = Ui.Res<Brush>("RaisedBrush"),
        VerticalAlignment = VerticalAlignment.Center,
        Child = new FontIcon { Glyph = glyph, FontSize = 15 },
    };
}

/// <summary>
/// Your sources for reading: folders, servers and plugins, each with what it holds and how it is
/// in words, in the order they are asked. Rows open their menu on click, on Shift+F10 and on
/// right click: a folder is looked at again or removed (with Undo), a server and a plugin move up
/// or down, switch off and on, or are removed. Add a source, Look again and Check matches sit
/// above the rows; the reading demo has a row of its own while it is on.
/// </summary>
public sealed partial class BookSourcesSection : UserControl
{
    readonly Ctx ctx = Ctx.Current;
    readonly StateHost state = new() { Padding = new Thickness(0, 0, 0, 48) };
    readonly StackPanel rows = new() { Spacing = 4, MaxWidth = 720, HorizontalAlignment = HorizontalAlignment.Left };
    readonly Button check = new() { MinHeight = 32 };
    readonly StackPanel bar = new() { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 0, 0, 8) };
    bool hearing;

    public BookSourcesSection()
    {
        var add = Ui.Label(T.SourcesAddTitle, (_, _) => ReadingParts.AddSource(ctx, XamlRoot), accent: true);
        var again = Ui.Label(T.BookSourcesScan, (_, _) => LookAgain());
        ToolTipService.SetToolTip(again, "F5"); // i18n-skip
        check.Click += (_, _) => _ = new CheckMatchesDialog(ctx, XamlRoot).ShowAsync();
        bar.Children.Add(add);
        bar.Children.Add(again);
        bar.Children.Add(check);

        state.Children.Add(rows);
        state.NeedsConnection = false;
        state.Placeholders = () =>
        {
            var plates = new StackPanel { Spacing = 4 };
            plates.Children.Add(States.Heading());
            plates.Children.Add(States.Rows(2, 32, 32, rowHeight: 52, spacing: 0));
            plates.Children.Add(States.Heading());
            plates.Children.Add(States.Rows(2, 32, 32, rowHeight: 52, spacing: 0));
            return plates;
        };
        var panel = new StackPanel();
        panel.Children.Add(bar);
        panel.Children.Add(state);
        Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, IsTabStop = false, Padding = new Thickness(0, 0, 12, 0) };

        KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.F5)
            {
                LookAgain();
                e.Handled = true;
            }
        };
        Loaded += (_, _) =>
        {
            if (!hearing)
            {
                ctx.Known.Books.Changed += Show;
                ctx.Batch += Heard;
            }
            hearing = true;
            CameInDesk.Ensure(ctx);
            // A page that keeps the section hidden (Your sources showing shows) asks when it shows it.
            if (Visibility == Visibility.Visible)
                ctx.Known.Books.Load();
            Show();
        };
        Unloaded += (_, _) =>
        {
            ctx.Known.Books.Changed -= Show;
            ctx.Batch -= Heard;
            hearing = false;
        };
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Stores or Event.Demo))
            Show();
    }

    /// <summary>Every folder looked through again, and servers that showed no series asked again.</summary>
    void LookAgain()
    {
        var books = ctx.Known.Books;
        books.Scan();
        if (books.View?.Servers.Any(s => s.Empty) == true)
            books.Recheck();
    }

    void Show()
    {
        var books = ctx.Known.Books;
        var view = books.View;
        var unmatched = BookWords.Unmatched(view).Length;
        check.Content = T.BookSourcesCheckMatches(unmatched);
        check.Visibility = unmatched > 0 ? Visibility.Visible : Visibility.Collapsed;
        if (view is null)
        {
            state.Wait();
            return;
        }
        var demo = ctx.Known.Demo.Reading;
        var mark = Ui.MarkFocus(rows);
        rows.Children.Clear();
        var folders = view.Folders.Where(f => !f.Removed).ToArray();
        if (folders.Length == 0 && view.Servers.Length == 0 && view.Plugins.Length == 0 && !demo)
        {
            bar.Visibility = Visibility.Collapsed;
            state.Empty(ReadingParts.PlainEmpty(T.SourcesAddTitle, T.BookAddLine, T.SourcesAddTitle, () => ReadingParts.AddSource(ctx, XamlRoot)));
            return;
        }
        bar.Visibility = Visibility.Visible;

        if (folders.Length > 0)
        {
            rows.Children.Add(Ui.Heading(T.BookSourcesFolders));
            rows.Children.Add(List(T.BookSourcesFolders, folders.Select(FolderRow)));
        }
        // Servers and plugins in the order they are asked.
        var order = view.Order.ToList();
        int Place(string id)
        {
            var at = order.IndexOf(id);
            return at < 0 ? int.MaxValue : at;
        }
        if (view.Servers.Length > 0)
        {
            rows.Children.Add(Ui.Heading(T.BookSourcesServers));
            rows.Children.Add(List(T.BookSourcesServers, view.Servers.OrderBy(s => Place(s.Id)).Select(s => ServerRow(s, order))));
        }
        if (view.Plugins.Length > 0)
        {
            rows.Children.Add(Ui.Heading(T.BookSourcesPlugins));
            rows.Children.Add(List(T.BookSourcesPlugins, view.Plugins.OrderBy(p => Place(p.Id)).Select(p => PluginRow(p, order))));
            var footer = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4, Margin = new Thickness(0, 4, 0, 0) };
            footer.Children.Add(new TextBlock { Text = T.BookSourcesPluginsFooter, Style = Ui.Res<Style>("HazeCaptionStyle"), VerticalAlignment = VerticalAlignment.Center });
            var stores = new HyperlinkButton { Content = T.NavPluginStores, Padding = new Thickness(0), FontSize = 12 };
            stores.Click += (_, _) => ctx.Window.Go("stores");
            footer.Children.Add(stores);
            rows.Children.Add(footer);
        }
        if (demo)
        {
            rows.Children.Add(Ui.Heading(T.SourcesDemoName));
            var remove = Ui.Label(T.SourcesDemoRemove, (_, _) => ctx.Send(new Command.RemoveDemo()));
            rows.Children.Add(List(T.SourcesDemoName, [Row("", T.SourcesDemoName, T.SourcesDemoBooksLine, null, remove)]));
        }
        state.Done();
        Ui.RestoreFocus(rows, mark);
    }

    static ListView List(string name, IEnumerable<ListViewItem> items)
    {
        var list = new ListView { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true, Padding = new Thickness(0) };
        AutomationProperties.SetName(list, name);
        foreach (var item in items)
            list.Items.Add(item);
        list.ItemClick += (_, e) =>
        {
            // A click opens the row's menu, as Shift+F10 does.
            if (list.ContainerFromItem(e.ClickedItem) is ListViewItem { ContextFlyout: { } menu } item)
                menu.ShowAt(item, new FlyoutShowOptions { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft });
            else if (e.ClickedItem is ListViewItem { ContextFlyout: { } own } self)
                own.ShowAt(self, new FlyoutShowOptions { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft });
        };
        return list;
    }

    /// <summary>A row: a glyph, the name over its line, and what stands at its end.</summary>
    static ListViewItem Row(string glyph, string title, string line, MenuFlyout? menu, UIElement? end = null)
    {
        var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 8, 0, 8) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(BookWords.Glyph(glyph));
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = title, Style = Ui.Res<Style>("BodyTextBlockStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
        words.Children.Add(new TextBlock { Text = line, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        if (end is FrameworkElement endElement)
        {
            endElement.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(endElement, 2);
            grid.Children.Add(endElement);
        }
        var item = new ListViewItem { Content = grid, HorizontalContentAlignment = HorizontalAlignment.Stretch, ContextFlyout = menu };
        AutomationProperties.SetName(item, ReadingParts.Joined(title, line));
        return item;
    }

    ListViewItem FolderRow(BookFolderRow folder)
    {
        var books = ctx.Known.Books;
        var menu = new MenuFlyout();
        menu.Items.Add(ReadingParts.Item(T.BookSourcesScan, books.Scan));
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(ReadingParts.Item(T.ActionRemove, () =>
        {
            books.RemoveFolder(folder.Id);
            ctx.Window.Say(T.SourcesRemoved(folder.Name), T.ActionUndo, () => books.RestoreFolder(folder.Id));
        }));
        var glyph = folder.Scan is BookFolderScan.Missing ? "" : "";
        return Row(glyph, folder.Name, BookWords.FolderLine(folder), menu);
    }

    /// <summary>Move up and Move down, for a server or a plugin in the order they are asked.</summary>
    void OrderItems(MenuFlyout menu, List<string> order, string id)
    {
        var at = order.IndexOf(id);
        if (at < 0)
            return;
        void Move(int step)
        {
            var moved = order.ToList();
            moved.RemoveAt(at);
            moved.Insert(at + step, id);
            ctx.Known.Books.SetOrder(moved);
        }
        if (at > 0)
            menu.Items.Add(ReadingParts.Item(T.SourcesMoveUp, () => Move(-1)));
        if (at < order.Count - 1)
            menu.Items.Add(ReadingParts.Item(T.SourcesMoveDown, () => Move(1)));
    }

    /// <summary>Alt with the up or down arrow moves the row, as on the shows' sources.</summary>
    void OrderKeys(ListViewItem item, List<string> order, string id)
    {
        item.KeyDown += (_, e) =>
        {
            var alt = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(global::Windows.System.VirtualKey.Menu)
                .HasFlag(global::Windows.UI.Core.CoreVirtualKeyStates.Down);
            if (!alt || e.Key is not (global::Windows.System.VirtualKey.Up or global::Windows.System.VirtualKey.Down))
                return;
            var at = order.IndexOf(id);
            var to = at + (e.Key == global::Windows.System.VirtualKey.Up ? -1 : 1);
            if (at < 0 || to < 0 || to >= order.Count)
                return;
            var moved = order.ToList();
            moved.RemoveAt(at);
            moved.Insert(to, id);
            ctx.Known.Books.SetOrder(moved);
            e.Handled = true;
        };
    }

    ListViewItem ServerRow(BookServerRow server, List<string> order)
    {
        var books = ctx.Known.Books;
        var menu = new MenuFlyout();
        var use = new ToggleMenuFlyoutItem { Text = T.BookSourcesUse, IsChecked = !server.Off };
        use.Click += (_, _) => books.SetOff(server.Id, !use.IsChecked);
        menu.Items.Add(use);
        var report = new ToggleMenuFlyoutItem { Text = T.BookSourcesReportProgress, IsChecked = server.ReportProgress };
        report.Click += (_, _) => books.SetReportProgress(server.Id, report.IsChecked);
        menu.Items.Add(report);
        if (server.Empty)
            menu.Items.Add(ReadingParts.Item(T.BookSourcesScan, books.Recheck));
        if (order.Count > 1)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            OrderItems(menu, order, server.Id);
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(ReadingParts.Item(T.ActionRemove, async () =>
        {
            if (await Ui.Confirm(XamlRoot, T.StoresRemovePluginTitle(server.Name), T.ServersRemoveLine, T.ActionRemove))
                books.RemoveServer(server.Id);
        }));
        var row = Row(server.SignedOut || server.Empty ? "" : "", server.Name, BookWords.ServerLine(server), menu);
        OrderKeys(row, order, server.Id);
        return row;
    }

    ListViewItem PluginRow(BookPluginRow plugin, List<string> order)
    {
        var books = ctx.Known.Books;
        var on = new ToggleSwitch { IsOn = !plugin.Off, OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(on, T.WindowsReadSourcesUsePlugin);
        on.Toggled += (_, _) =>
        {
            if (on.IsOn == plugin.Off)
                books.SetOff(plugin.Id, !on.IsOn);
        };
        var menu = new MenuFlyout();
        var use = new ToggleMenuFlyoutItem { Text = T.WindowsReadSourcesUsePlugin, IsChecked = !plugin.Off };
        use.Click += (_, _) => books.SetOff(plugin.Id, !use.IsChecked);
        menu.Items.Add(use);
        if (order.Count > 1)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            OrderItems(menu, order, plugin.Id);
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(ReadingParts.Item(T.ActionRemove, async () =>
        {
            if (await Ui.Confirm(XamlRoot, T.StoresRemovePluginTitle(plugin.Name), T.SourcesRemoveDetail, T.ActionRemove))
                ctx.Send(new Command.RemovePlugin(plugin.Id));
        }));
        var row = Row("", plugin.Name, BookWords.PluginLine(ctx, plugin), menu, on);
        OrderKeys(row, order, plugin.Id);
        return row;
    }
}

/// <summary>
/// Add a source, for reading: a folder on this PC or a drive, a server, or a plugin store. Each
/// kind is a row with its line and a chevron; picking one closes this dialog and opens the next
/// step (the system folder picker, the server step, or Join a plugin store).
/// </summary>
sealed class AddBookSourceDialog
{
    enum Kind { None, Folder, Server, Store }

    readonly Ctx ctx;
    readonly XamlRoot root;
    readonly ContentDialog dialog;
    Kind picked;

    public AddBookSourceDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        this.root = root;
        var body = new StackPanel { Spacing = 8, MinWidth = 440 };
        body.Children.Add(Ui.Haze(T.BookAddLine));
        body.Children.Add(Choice("", T.BookAddFolder, T.BookAddFolderLine(T.DeviceKindPc), Kind.Folder));
        body.Children.Add(Choice("", T.BookAddServer, T.BookAddServerLine, Kind.Server));
        body.Children.Add(Choice("", T.BookAddStore, T.BookAddStoreLine, Kind.Store));
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.SourcesAddTitle,
            Content = body,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.None,
        };
    }

    Button Choice(string glyph, string title, string line, Kind kind)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(BookWords.Glyph(glyph));
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(Ui.Text(title, "BodyStrongTextBlockStyle"));
        words.Children.Add(new TextBlock { Text = line, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.WrapWholeWords });
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        var chevron = new FontIcon { Glyph = "", FontSize = 12, Foreground = Ui.Res<Brush>("HazeBrush"), VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(chevron, 2);
        grid.Children.Add(chevron);
        var button = new Button
        {
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(12, 10, 12, 10),
        };
        AutomationProperties.SetName(button, ReadingParts.Joined(title, line));
        button.Click += (_, _) =>
        {
            picked = kind;
            dialog.Hide();
        };
        return button;
    }

    public async Task ShowAsync()
    {
        CameInDesk.Ensure(ctx);
        await Dialogs.Show(dialog);
        switch (picked)
        {
            case Kind.Folder:
                await CameInDesk.PickFolder(ctx);
                break;
            case Kind.Server:
                await new BookServerDialog(ctx, root).ShowAsync();
                break;
            case Kind.Store:
                ctx.Window.ShowJoinStore();
                break;
        }
    }
}

/// <summary>
/// Add a server: the address, then a user and password, or a Kavita OPDS link alone (its key
/// shows as a chip with the last four characters, and the sign-in fields go). What the server
/// holds is looked at 500 ms after the typing stops, and said before anything is added: its
/// libraries to pick, or why nothing came in. Add server is the one accent action; after a failed
/// look it is Try again.
/// </summary>
sealed class BookServerDialog
{
    static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(500), Slow = TimeSpan.FromMilliseconds(600), Hold = TimeSpan.FromMilliseconds(300);

    readonly Ctx ctx;
    readonly ContentDialog dialog;
    // A made-up host, not words.
    readonly TextBox address = new() { PlaceholderText = "books.home.example" }; // i18n-skip
    readonly TextBlock chipText = new() { FontFamily = new FontFamily("Cascadia Mono, Consolas"), FontSize = 12, VerticalAlignment = VerticalAlignment.Center };
    readonly Border chip;
    readonly StackPanel notes = new() { Spacing = 4, Margin = new Thickness(0, 8, 0, 0) };
    readonly StackPanel signIn = new() { Spacing = 6, Margin = new Thickness(0, 12, 0, 0) };
    readonly TextBox user = new();
    readonly PasswordBox password = new();
    readonly ContentControl result = new() { HorizontalContentAlignment = HorizontalAlignment.Stretch, Margin = new Thickness(0, 14, 0, 0), IsTabStop = false };
    readonly HashSet<string> unticked = [];
    bool reportProgress = true, slow, adding;
    int asking;
    string? foundFor;
    CameIn? outcome;
    DateTime slowSince;
    CancellationTokenSource? typing, waiting;

    public BookServerDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(address, T.BookServerAddress);
        AutomationProperties.SetName(user, T.BookServerUsername);
        AutomationProperties.SetName(password, T.BookServerPassword);
        var keyRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        keyRow.Children.Add(new FontIcon { Glyph = "", FontSize = 11 });
        keyRow.Children.Add(chipText);
        chip = new Border
        {
            Child = keyRow,
            Background = Ui.Res<Brush>("RaisedBrush"),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(8, 3, 8, 3),
            Margin = new Thickness(0, 0, 8, 0),
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed,
        };
        var field = new Grid();
        field.Children.Add(address);
        field.Children.Add(chip);

        signIn.Children.Add(Ui.Text(T.BookServerUsername, "HazeCaptionStyle"));
        signIn.Children.Add(user);
        var passwordLabel = Ui.Text(T.BookServerPassword, "HazeCaptionStyle");
        passwordLabel.Margin = new Thickness(0, 6, 0, 0);
        signIn.Children.Add(passwordLabel);
        signIn.Children.Add(password);

        var body = new StackPanel { Width = 460 };
        body.Children.Add(Ui.Text(T.BookServerAddress, "HazeCaptionStyle"));
        field.Margin = new Thickness(0, 6, 0, 0);
        body.Children.Add(field);
        body.Children.Add(notes);
        body.Children.Add(signIn);
        body.Children.Add(result);

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.BookAddServerTitle,
            Content = new ScrollViewer { Content = body, MaxHeight = 560, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            PrimaryButtonText = T.BookAddServerButton,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = false,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;

        address.TextChanged += (_, _) => Typed();
        user.TextChanged += (_, _) => Typed();
        password.PasswordChanged += (_, _) => Typed();
        password.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && Ready)
            {
                e.Handled = true;
                Look();
            }
        };
        Fields();
    }

    public async Task ShowAsync()
    {
        CameInDesk.Ensure(ctx);
        CameInDesk.Server = this;
        try
        {
            await Dialogs.Show(dialog, () => address.Focus(FocusState.Programmatic));
        }
        finally
        {
            if (CameInDesk.Server == this)
                CameInDesk.Server = null;
            typing?.Cancel();
            waiting?.Cancel();
        }
    }

    string Trimmed => address.Text.Trim();

    (string Host, string Last4)? Link => BookWords.KavitaLink(address.Text);

    bool Ready => Link is not null || (Trimmed.Length > 0 && user.Text.Trim().Length > 0 && password.Password.Length > 0);

    /// <summary>What a look found that can be added: the server, or one that shows no series yet.</summary>
    FoundServer? Found => outcome switch
    {
        CameIn.Found found => found.V1,
        CameIn.Empty { V1: NothingCameIn.EmptyCatalogue empty } => empty.Found,
        _ => null,
    };

    bool Failed => outcome is CameIn.Empty { V1: not NothingCameIn.EmptyCatalogue };

    /// <summary>The address's notes, the key chip, and the sign-in fields, for what is typed.</summary>
    void Fields()
    {
        var link = Link;
        chip.Visibility = link is null ? Visibility.Collapsed : Visibility.Visible;
        if (link is { } l)
        {
            chipText.Text = T.BookServerKey(l.Last4);
            AutomationProperties.SetHelpText(address, T.BookServerKey(l.Last4));
            address.Padding = new Thickness(10, 5, 112, 6);
        }
        else
        {
            AutomationProperties.SetHelpText(address, "");
            address.ClearValue(Control.PaddingProperty);
        }
        notes.Children.Clear();
        if (address.Text.Length == 0)
            notes.Children.Add(Ui.Haze(T.BookServerAddressHint));
        else if (link is not null)
        {
            notes.Children.Add(Ui.Haze(T.BookServerKavitaKey));
            notes.Children.Add(Ui.Haze(T.BookServerKeyKept(T.SecureStoreWindows)));
        }
        notes.Visibility = notes.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        signIn.Visibility = link is null && address.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>The look runs once the typing has paused for 500 ms.</summary>
    async void Typed()
    {
        Fields();
        typing?.Cancel();
        if (!Ready)
            return;
        var mine = typing = new CancellationTokenSource();
        try
        {
            await Task.Delay(Settle, mine.Token);
        }
        catch (TaskCanceledException)
        {
            return;
        }
        Look();
    }

    async void Look()
    {
        typing?.Cancel();
        if (!Ready || adding)
            return;
        asking++;
        var link = Link is not null;
        ctx.Known.Books.Probe(Trimmed, link ? null : user.Text.Trim(), link ? null : password.Password);
        dialog.IsPrimaryButtonEnabled = false;
        // The waiting line only after 600 ms, so a quick answer comes straight in.
        waiting?.Cancel();
        var mine = waiting = new CancellationTokenSource();
        try
        {
            await Task.Delay(Slow, mine.Token);
        }
        catch (TaskCanceledException)
        {
            return;
        }
        if (asking == 0)
            return;
        slow = true;
        slowSince = DateTime.UtcNow;
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
        line.Children.Add(Ui.Haze(T.BookServerLooking));
        AutomationProperties.SetLiveSetting(line, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        result.Content = line;
        Motion.FadeIn(line);
    }

    /// <summary>The answer to a look; only the last look's answer is drawn.</summary>
    public async void Heard(CameIn came)
    {
        if (asking > 0)
            asking--;
        if (asking > 0)
            return;
        waiting?.Cancel();
        // The waiting line, once up, stays 0.3 s so it never flickers.
        if (slow && DateTime.UtcNow - slowSince is var shown && shown < Hold)
            await Task.Delay(Hold - shown);
        if (asking > 0)
            return;
        slow = false;
        outcome = came;
        if (Found is { } found && found.Pending != foundFor)
        {
            foundFor = found.Pending;
            unticked.Clear();
        }
        Draw();
    }

    /// <summary>The server was added: the dialog closes, and the window says so.</summary>
    public void Added()
    {
        if (adding)
            dialog.Hide();
    }

    void Draw()
    {
        var panel = new StackPanel { Spacing = 12 };
        switch (outcome)
        {
            case CameIn.Found { V1: var found }:
                panel.Children.Add(BookWords.Note(T.BookServerFoundTitle(found.Series ?? 0, found.Libraries.Length),
                    T.BookServerFoundLine(BookWords.Kind(found.Kind), found.Host), found: true));
                if (found.Libraries.Length > 0)
                    panel.Children.Add(Libraries(found));
                panel.Children.Add(Progress(found.Host));
                break;
            case CameIn.Empty { V1: var why }:
                switch (why)
                {
                    case NothingCameIn.Unreachable u:
                        panel.Children.Add(BookWords.Note(T.BookServerErrUnreachable(u.Host), T.BookServerErrUnreachableLine, failed: true));
                        break;
                    case NothingCameIn.WrongPassword p:
                        panel.Children.Add(BookWords.Note(T.BookServerErrPassword(p.Host), T.BookServerErrPasswordLine(BookWords.Kind(p.Kind)), failed: true));
                        break;
                    case NothingCameIn.WrongKey k:
                        panel.Children.Add(BookWords.Note(T.BookServerErrKey(k.Host), T.BookServerErrKeyLine, failed: true));
                        break;
                    case NothingCameIn.NotABookServer:
                        panel.Children.Add(BookWords.Note(T.BookSourcesNotABookServer, T.BookServerNotBooksLine, failed: true));
                        break;
                    case NothingCameIn.EmptyCatalogue e:
                        panel.Children.Add(BookWords.Note(T.BookServerEmpty(e.Found.Host), T.BookServerEmptyLine));
                        panel.Children.Add(Ui.Haze(T.BookServerEmptyRecheck));
                        panel.Children.Add(Progress(e.Found.Host));
                        break;
                }
                break;
        }
        result.Content = panel;
        Motion.FadeIn(panel);
        Buttons();
    }

    void Buttons()
    {
        if (Failed)
        {
            dialog.PrimaryButtonText = T.ActionTryAgain;
            dialog.IsPrimaryButtonEnabled = Ready;
            return;
        }
        dialog.PrimaryButtonText = T.BookAddServerButton;
        dialog.IsPrimaryButtonEnabled = !adding && Found is { } found
            && (found.Libraries.Length == 0 || found.Libraries.Any(l => !unticked.Contains(l.Id)));
    }

    /// <summary>One check box per library, all ticked to start with.</summary>
    UIElement Libraries(FoundServer found)
    {
        var list = new StackPanel { Spacing = 0 };
        foreach (var library in found.Libraries)
        {
            var words = new StackPanel { Spacing = 0 };
            words.Children.Add(Ui.Text(library.Name, "BodyTextBlockStyle"));
            if (library.Series is { } series)
                words.Children.Add(Ui.Text(T.BookServerLibraryLine(series), "HazeCaptionStyle"));
            var box = new CheckBox { Content = words, IsChecked = !unticked.Contains(library.Id), Padding = new Thickness(8, 4, 0, 4) };
            AutomationProperties.SetName(box, library.Series is { } count ? ReadingParts.Joined(library.Name, T.BookServerLibraryLine(count)) : library.Name);
            box.Checked += (_, _) => { unticked.Remove(library.Id); Buttons(); };
            box.Unchecked += (_, _) => { unticked.Add(library.Id); Buttons(); };
            list.Children.Add(box);
        }
        return Ui.Card(list, 8);
    }

    /// <summary>Send progress to the server, on to start with.</summary>
    UIElement Progress(string host)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 2 };
        words.Children.Add(Ui.Text(T.BookServerSyncProgress, "BodyTextBlockStyle"));
        words.Children.Add(Ui.Text(T.BookServerSyncProgressLine(host), "HazeCaptionStyle"));
        grid.Children.Add(words);
        var on = new ToggleSwitch { IsOn = reportProgress, OnContent = "", OffContent = "", MinWidth = 0, VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetName(on, T.BookServerSyncProgress);
        on.Toggled += (_, _) => reportProgress = on.IsOn;
        Grid.SetColumn(on, 1);
        grid.Children.Add(on);
        return Ui.Card(grid, 12);
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        args.Cancel = true;
        if (Failed)
        {
            Look();
            return;
        }
        if (Found is not { } found)
            return;
        // Every library ticked asks for all of them, so libraries added later come in too.
        var libraries = unticked.Count == 0 ? [] : found.Libraries.Where(l => !unticked.Contains(l.Id)).Select(l => l.Id);
        adding = true;
        dialog.IsPrimaryButtonEnabled = false;
        ctx.Known.Books.AddServer(found.Pending, libraries, reportProgress);
    }
}

/// <summary>
/// What came in after a source was added, said wherever the window is: the folder report, the
/// server step's answers, the store report (only while this device reads), and the notices
/// "Added X · N series" and "Installed N plugins". Known does not route <c>Event.CameIn</c>, so
/// this hears every batch once <see cref="Ensure"/> has run.
/// </summary>
static class CameInDesk
{
    static Ctx? heard;

    /// <summary>The server step that is open, which takes the probe's answers.</summary>
    internal static BookServerDialog? Server { get; set; }

    /// <summary>The store report that is open, which takes Installed and redraws on Stores.</summary>
    static StoreReportDialog? store;

    public static void Ensure(Ctx ctx)
    {
        if (heard == ctx)
            return;
        heard = ctx;
        ctx.Batch += batch => Heard(ctx, batch);
    }

    static XamlRoot? Root(Ctx ctx) => ctx.Window.Content?.XamlRoot;

    static void Heard(Ctx ctx, IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is Event.Stores)
                store?.Draw();
            if (e is not Event.CameIn { V1: var came })
                continue;
            switch (came)
            {
                case CameIn.Found:
                case CameIn.Empty { V1: NothingCameIn.Unreachable or NothingCameIn.WrongPassword or NothingCameIn.WrongKey or NothingCameIn.NotABookServer or NothingCameIn.EmptyCatalogue }:
                    Server?.Heard(came);
                    break;
                case CameIn.ServerAdded { V1: var added }:
                    Server?.Added();
                    ctx.Window.Say(T.SourceAdded(added.Name, added.Series ?? 0));
                    break;
                case CameIn.Folder { V1: var report }:
                    _ = ShowFolder(ctx, report, false);
                    break;
                case CameIn.Empty { V1: NothingCameIn.UnsupportedFiles u }:
                    _ = ShowFolder(ctx, u.Folder, true);
                    break;
                case CameIn.Empty { V1: NothingCameIn.EmptyFolder f }:
                    _ = ShowFolder(ctx, f.Folder, true);
                    break;
                case CameIn.Empty { V1: NothingCameIn.FolderUnreadable r }:
                    _ = ShowFolder(ctx, r.Folder, true, unreadable: true);
                    break;
                case CameIn.Installed { V1: var done }:
                    if (store is { } open)
                        open.Take(came);
                    else
                        ctx.Window.Say(T.SourceAddedPlugins(done.Installed, done.Name));
                    break;
                case CameIn.Store:
                case CameIn.Empty { V1: NothingCameIn.EmptyStore or NothingCameIn.OtherMedium or NothingCameIn.StoreUnreachable }:
                    // Sent on every join; the report is reading's, so only a device that reads shows it.
                    if (ctx.Known.Medium.Reads)
                        _ = ShowStore(ctx, came);
                    break;
            }
        }
    }

    /// <summary>
    /// The system folder picker, then the folder scanned for reading; its report follows, and the
    /// folder stays only once Add folder is chosen there.
    /// </summary>
    public static async Task PickFolder(Ctx ctx)
    {
        if (await ctx.Window.PickFolder() is { Length: > 0 } path)
            Add(ctx, path);
    }

    /// <summary>Folders picked again while already in Your sources: Cancel on their report leaves them there.</summary>
    static readonly HashSet<string> already = new(StringComparer.OrdinalIgnoreCase);

    static void Add(Ctx ctx, string path)
    {
        if (ctx.Known.Books.View?.Folders.Any(f => string.Equals(f.Path, path, StringComparison.OrdinalIgnoreCase)) == true)
            already.Add(path);
        var name = System.IO.Path.GetFileName(path.TrimEnd('\\', '/'));
        ctx.Known.Books.AddFolder(path, name.Length > 0 ? name : path);
    }

    /// <summary>
    /// What a folder's scan found, as the board draws it: the folder with Change, the series and
    /// files or why nothing can be read, the files Kaeri can't open with Show files, and the
    /// formats it opens; then Add folder, or Choose another folder when nothing was found, and
    /// Cancel. The folder stays only on Add folder: Cancel or Esc takes it back out, as a form's
    /// cancel would.
    /// </summary>
    static async Task ShowFolder(Ctx ctx, FolderReport report, bool nothing, bool unreadable = false)
    {
        if (Root(ctx) is not { } root)
            return;
        var kept = report.Path is { } picked && already.Remove(picked);
        ContentDialog? shown = null;
        // Another folder in this one's place: picked over the dialog, which closes only once one is.
        async Task Change()
        {
            if (await ctx.Window.PickFolder() is not { Length: > 0 } path)
                return;
            ctx.Known.Books.RemoveFolder(report.Id);
            // Already taken back out: closing must not remove it again.
            kept = true;
            shown?.Hide();
            Add(ctx, path);
        }
        var body = new StackPanel { Spacing = 12, Width = 460 };
        var where = new Grid { ColumnSpacing = 12 };
        where.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        where.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        where.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        where.Children.Add(BookWords.Glyph(""));
        var at = new TextBlock { Text = report.Path ?? report.Name, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
        Grid.SetColumn(at, 1);
        where.Children.Add(at);
        var change = new Button { Content = T.BookFolderChange, VerticalAlignment = VerticalAlignment.Center };
        change.Click += async (_, _) => await Change();
        Grid.SetColumn(change, 2);
        where.Children.Add(change);
        body.Children.Add(where);
        body.Children.Add(unreadable
            ? BookWords.Note(T.BookFolderUnreadable(report.Name), T.BookFolderUnreadableLine, failed: true)
            : nothing
            ? BookWords.Note(T.BookFolderEmpty(report.Name), T.BookFolderEmptyLine(report.Files))
            : BookWords.Note(T.BookFolderFound(report.Series, report.Files), T.BookFolderFoundLine(report.Path ?? report.Name, report.Matched), found: true));
        var skipped = BookWords.Skipped(report);
        if (skipped > 0)
        {
            var card = new StackPanel { Spacing = 8 };
            var head = new Grid { ColumnSpacing = 12 };
            head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = new StackPanel { Spacing = 2 };
            words.Children.Add(Ui.Text(T.BookFolderUnsupported(skipped), "BodyTextBlockStyle"));
            words.Children.Add(Ui.Text(T.BookFolderUnsupportedLine(BookWords.Kinds(report)), "HazeCaptionStyle"));
            head.Children.Add(words);
            card.Children.Add(head);
            if (report.Listed.Length > 0)
            {
                var files = new StackPanel { Spacing = 4, Visibility = Visibility.Collapsed };
                foreach (var file in report.Listed)
                    files.Children.Add(new TextBlock { Text = file, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
                var chevron = new FontIcon { Glyph = "", FontSize = 10, RenderTransformOrigin = new global::Windows.Foundation.Point(0.5, 0.5), RenderTransform = new RotateTransform() };
                var label = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                label.Children.Add(new TextBlock { Text = T.BookFolderShowFiles, FontSize = 12 });
                label.Children.Add(chevron);
                var show = new ToggleButton { Content = label, VerticalAlignment = VerticalAlignment.Center, MinHeight = 28, Padding = new Thickness(10, 2, 10, 2) };
                AutomationProperties.SetName(show, T.BookFolderShowFiles);
                show.Click += (_, _) =>
                {
                    var open = show.IsChecked == true;
                    ((RotateTransform)chevron.RenderTransform).Angle = open ? 90 : 0;
                    files.Visibility = open ? Visibility.Visible : Visibility.Collapsed;
                    if (open)
                        Motion.FadeIn(files);
                };
                Grid.SetColumn(show, 1);
                head.Children.Add(show);
                card.Children.Add(new ScrollViewer { Content = files, MaxHeight = 180, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });
            }
            body.Children.Add(Ui.Card(card, 12));
        }
        if (!unreadable)
            body.Children.Add(Ui.Haze(T.BookFolderOpens));
        var dialog = shown = new ContentDialog
        {
            XamlRoot = root,
            Title = T.SourcesAddFolderButton,
            Content = new ScrollViewer { Content = body, MaxHeight = 560, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            PrimaryButtonText = nothing ? T.BookFolderChooseOther : T.BookFolderAdd,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonClick += async (_, e) =>
        {
            if (!nothing)
            {
                kept = true;
                return;
            }
            // Choose another folder: the dialog stays up while the picker is.
            e.Cancel = true;
            await Change();
        };
        // A dialog put aside for another before it showed leaves the folder as it was added.
        var opened = false;
        await Dialogs.Show(dialog, () => opened = true);
        if (opened && !kept)
            ctx.Known.Books.RemoveFolder(report.Id);
    }

    static async Task ShowStore(Ctx ctx, CameIn came)
    {
        if (Root(ctx) is not { } root)
            return;
        // A second report for the same join replaces the first rather than queueing behind it.
        if (store is { } open)
        {
            open.Take(came);
            return;
        }
        var report = store = new StoreReportDialog(ctx, root, came);
        try
        {
            await report.ShowAsync();
        }
        finally
        {
            if (store == report)
                store = null;
        }
    }
}

/// <summary>
/// What a joined plugin store holds for this device: the plugins for reading, then for watching,
/// each with Install or Installed, a count of the rest, and Install all for reading when the
/// store's report offers it; or why nothing is offered.
/// </summary>
sealed class StoreReportDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly HashSet<string> installing = [];
    CameIn came;
    bool installingAll;

    public StoreReportDialog(Ctx ctx, XamlRoot root, CameIn came)
    {
        this.ctx = ctx;
        this.came = came;
        dialog = new ContentDialog { XamlRoot = root, CloseButtonText = T.ActionDone };
        dialog.PrimaryButtonClick += OnPrimary;
        Draw();
    }

    public async Task ShowAsync() => await Dialogs.Show(dialog);

    public void Take(CameIn now)
    {
        came = now;
        installingAll = false;
        Draw();
    }

    StoreRow? Row(string store) => ctx.Known.Stores.FirstOrDefault(s => s.Id == store);

    public void Draw()
    {
        var body = new StackPanel { Spacing = 12, Width = 480 };
        dialog.PrimaryButtonText = "";
        dialog.DefaultButton = ContentDialogButton.Close;
        dialog.IsPrimaryButtonEnabled = true;
        switch (came)
        {
            case CameIn.Store { V1: var report }:
                dialog.Title = T.StoreJoinedTitle(report.Name);
                body.Children.Add(Ui.Haze(T.StoreJoinedLine(report.Owner ?? report.Name, report.ForReading.Length + report.ForWatching.Length)));
                Group(body, report, report.ForReading, T.StoreGroupReading);
                Group(body, report, report.ForWatching, T.StoreGroupWatching);
                if (report.OtherMedium > 0)
                    body.Children.Add(Ui.Haze(T.StoreOtherMediumMore(report.OtherMedium)));
                if (report.InstallAll)
                {
                    dialog.PrimaryButtonText = T.StoreInstallAllReading;
                    dialog.IsPrimaryButtonEnabled = !installingAll;
                }
                break;
            case CameIn.Installed { V1: var done }:
                dialog.Title = T.StoreInstalled(done.Installed);
                break;
            case CameIn.Empty { V1: NothingCameIn.OtherMedium other }:
                dialog.Title = T.StoreOtherMedium(other.Store.Name);
                body.Children.Add(Ui.Haze(T.StoreOtherMediumLine));
                dialog.PrimaryButtonText = T.StoreTurnOnWatching;
                break;
            case CameIn.Empty { V1: NothingCameIn.EmptyStore empty }:
                dialog.Title = T.StoreEmpty(empty.Store.Name);
                body.Children.Add(Ui.Haze(T.StoreEmptyLine(empty.Store.Owner ?? empty.Store.Name)));
                break;
            case CameIn.Empty { V1: NothingCameIn.StoreUnreachable unreachable }:
                dialog.Title = BookWords.Note(T.StoreErrUnreachable(unreachable.Host), null, failed: true);
                body.Children.Add(Ui.Haze(T.StoreErrUnreachableLine(unreachable.Host)));
                break;
        }
        if (dialog.PrimaryButtonText.Length > 0)
        {
            dialog.DefaultButton = ContentDialogButton.Primary;
            dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        }
        dialog.Content = body.Children.Count > 0
            ? new ScrollViewer { Content = body, MaxHeight = 520, VerticalScrollBarVisibility = ScrollBarVisibility.Auto }
            : null;
    }

    void Group(StackPanel body, StoreReport report, string[] ids, string title)
    {
        if (ids.Length == 0)
            return;
        var heading = Ui.Text(title, "BodyStrongTextBlockStyle");
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        body.Children.Add(heading);
        var rows = new StackPanel { Spacing = 0 };
        var row = Row(report.Store);
        foreach (var id in ids)
        {
            var plugin = row?.Plugins.FirstOrDefault(p => p.Id == id || p.Name == id);
            var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 6, 0, 6) };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.Children.Add(BookWords.Glyph(""));
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            var name = plugin?.Name ?? id;
            words.Children.Add(Ui.Text(name, "BodyTextBlockStyle"));
            if (plugin is not null)
                words.Children.Add(new TextBlock { Text = T.StorePluginRow(plugin.Version, Stores.ContactsLine(plugin)), Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            Grid.SetColumn(words, 1);
            grid.Children.Add(words);
            UIElement end;
            if (plugin?.Status is StorePluginStatus.Installed)
                end = new TextBlock { Text = T.StoreInstalledRow, Style = Ui.Res<Style>("HazeCaptionStyle"), VerticalAlignment = VerticalAlignment.Center };
            else
            {
                var pluginId = plugin?.Id ?? id;
                var install = new Button { Content = T.StoreInstall, MinHeight = 28, VerticalAlignment = VerticalAlignment.Center, IsEnabled = !installing.Contains(pluginId) && !installingAll };
                AutomationProperties.SetName(install, ReadingParts.Joined(T.StoreInstall, name));
                install.Click += (_, _) =>
                {
                    installing.Add(pluginId);
                    install.IsEnabled = false;
                    ctx.Send(new Command.InstallPlugin(report.Store, pluginId));
                };
                end = install;
            }
            Grid.SetColumn((FrameworkElement)end, 2);
            grid.Children.Add(end);
            rows.Children.Add(grid);
        }
        body.Children.Add(Ui.Card(rows, 12));
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        switch (came)
        {
            case CameIn.Store { V1: var report } when report.InstallAll:
                // Stays open: Installed takes its place when the plugins are in.
                args.Cancel = true;
                installingAll = true;
                sender.IsPrimaryButtonEnabled = false;
                ctx.Send(new Command.InstallAllForReading(report.Store));
                Draw();
                break;
            case CameIn.Empty { V1: NothingCameIn.OtherMedium }:
                ctx.Known.Medium.Choose(KaeriIsFor.Both);
                break;
        }
    }
}

/// <summary>
/// Check matches: the series in folders with no match that are not kept unmatched. Each has Keep
/// unmatched and Find; Find searches by name (the folder's name to start with) and lists the
/// choices with how sure each is, one of which is chosen. Back returns to the list.
/// </summary>
sealed class CheckMatchesDialog
{
    static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(400);

    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly ContentControl body = new() { HorizontalContentAlignment = HorizontalAlignment.Stretch, IsTabStop = false };
    readonly TextBox query = new() { PlaceholderText = T.SearchPlaceholder };
    readonly StackPanel choices = new() { Spacing = 2 };
    readonly StateHost choicesState = new();
    BookGroupRow? finding;
    CancellationTokenSource? typing;

    public CheckMatchesDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(query, T.SearchPlaceholder);
        choicesState.Children.Add(choices);
        choicesState.NeedsConnection = true;
        choicesState.Placeholders = () => States.Rows(4, 32, 48, rowHeight: 56, spacing: 0);
        query.TextChanged += (_, _) => Typed();
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.BookSourcesCheckMatchesTitle,
            Content = new ScrollViewer { Content = body, MaxHeight = 560, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            CloseButtonText = T.ActionDone,
            DefaultButton = ContentDialogButton.Close,
        };
    }

    public async Task ShowAsync()
    {
        ctx.Known.Books.Changed += Changed;
        try
        {
            DrawList();
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Known.Books.Changed -= Changed;
            typing?.Cancel();
        }
    }

    void Changed()
    {
        if (finding is null)
            DrawList();
        else
            DrawChoices();
    }

    void DrawList()
    {
        var panel = new StackPanel { Spacing = 0, Width = 480 };
        var groups = BookWords.Unmatched(ctx.Known.Books.View);
        if (groups.Length == 0)
            panel.Children.Add(Ui.Haze(T.BookSourcesUnmatchedNote));
        foreach (var group in groups)
        {
            var grid = new Grid { ColumnSpacing = 8, Padding = new Thickness(0, 8, 0, 8) };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(new TextBlock { Text = group.Name, Style = Ui.Res<Style>("BodyTextBlockStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            words.Children.Add(Ui.Text(group.MatchedTitle ?? T.BookSourcesNoMatch, "HazeCaptionStyle"));
            grid.Children.Add(words);
            var keep = Ui.Label(T.BookSourcesKeep, (_, _) => ctx.Known.Books.Keep(group.Group));
            AutomationProperties.SetName(keep, ReadingParts.Joined(T.BookSourcesKeep, group.Name));
            keep.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(keep, 1);
            grid.Children.Add(keep);
            var find = Ui.Label(T.BookSourcesFind, (_, _) => Find(group));
            AutomationProperties.SetName(find, ReadingParts.Joined(T.BookSourcesFind, group.Name));
            find.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(find, 2);
            grid.Children.Add(find);
            panel.Children.Add(grid);
        }
        if (groups.Length > 0)
        {
            var note = Ui.Haze(T.BookSourcesUnmatchedNote);
            note.Margin = new Thickness(0, 12, 0, 0);
            panel.Children.Add(note);
        }
        var mark = Ui.MarkFocus(body);
        body.Content = panel;
        Ui.RestoreFocus(body, mark);
    }

    void Find(BookGroupRow group)
    {
        finding = group;
        var panel = new StackPanel { Spacing = 12, Width = 480 };
        var back = new Button { Content = T.ActionBack, MinHeight = 32 };
        back.Click += (_, _) =>
        {
            finding = null;
            typing?.Cancel();
            DrawList();
        };
        panel.Children.Add(back);
        panel.Children.Add(Ui.Text(group.Name, "BodyStrongTextBlockStyle"));
        panel.Children.Add(query);
        panel.Children.Add(choicesState);
        body.Content = panel;
        query.Text = group.Name;
        query.SelectAll();
        query.Focus(FocusState.Programmatic);
        Ask();
    }

    async void Typed()
    {
        typing?.Cancel();
        if (finding is null)
            return;
        var mine = typing = new CancellationTokenSource();
        try
        {
            await Task.Delay(Settle, mine.Token);
        }
        catch (TaskCanceledException)
        {
            return;
        }
        Ask();
    }

    void Ask()
    {
        if (finding is not { } group || query.Text.Trim() is not { Length: > 0 } text)
            return;
        ctx.Known.Books.Matches.Remove(group.Group);
        choicesState.Wait(choices.Children.Count > 0);
        ctx.Known.Books.FindMatch(group.Group, text);
    }

    void DrawChoices()
    {
        if (finding is not { } group || !ctx.Known.Books.Matches.TryGetValue(group.Group, out var found))
            return;
        choices.Children.Clear();
        if (found.Length == 0)
        {
            choicesState.Empty(ReadingParts.PlainEmpty(T.BookSourcesNoMatch, null));
            return;
        }
        foreach (var choice in found)
        {
            var line = ReadingParts.Joined(choice.Format, choice.Year?.ToString(System.Globalization.CultureInfo.CurrentCulture), T.LibraryMatchPercent(choice.Confidence));
            var words = new StackPanel { Spacing = 2 };
            words.Children.Add(new TextBlock { Text = choice.Title, Style = Ui.Res<Style>("BodyTextBlockStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            words.Children.Add(Ui.Text(line, "HazeCaptionStyle"));
            var pick = new Button
            {
                Content = words,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Padding = new Thickness(12, 8, 12, 8),
            };
            AutomationProperties.SetName(pick, ReadingParts.Joined(choice.Title, line));
            var anilist = choice.Anilist;
            pick.Click += (_, _) =>
            {
                ctx.Known.Books.ChooseMatch(group.Group, anilist);
                finding = null;
                DrawList();
            };
            choices.Children.Add(pick);
        }
        choicesState.Done();
    }
}
