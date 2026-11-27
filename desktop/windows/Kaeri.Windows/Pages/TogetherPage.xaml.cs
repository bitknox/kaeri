using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Watch together, the place (board v5): the room you are in, on top with its line and faces,
/// Open the player and Leave; Join a room with an invite link, and what the link's room watches
/// before Join; and the last ten rooms with Rejoin. A room starts from an episode, never here.
/// </summary>
public sealed partial class TogetherPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    readonly TextBox join = new() { PlaceholderText = T.RoomPlaceLink, Width = 320, FontFamily = new FontFamily("Cascadia Mono, Consolas") };
    /// <summary>The recent room a Rejoin looked into, while its answer is awaited.</summary>
    string? rejoining;
    /// <summary>The link Join tried that failed: it stays in the field, and Try again is the only button.</summary>
    string? failedLink;

    readonly StateHost recentState = new() { Placeholders = () => States.Rows(2, 64, 36, rowHeight: 56, spacing: 0), NeedsConnection = false };
    readonly StackPanel recentRows = new();

    public TogetherPage()
    {
        InitializeComponent();
        recentState.Children.Add(recentRows);
        AutomationProperties.SetName(join, T.RoomPlaceLink);
        join.KeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Enter)
                Join(join.Text);
        };
        join.TextChanged += (_, _) => Show();
        ctx.Hear(this, Heard);
        // Ctrl+V outside a field pastes into Join a room.
        Loaded += (_, _) => KeyboardAccelerators.Add(Paste());
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
    }

    KeyboardAccelerator Paste()
    {
        var key = new KeyboardAccelerator { Key = VirtualKey.V, Modifiers = VirtualKeyModifiers.Control };
        key.Invoked += async (_, e) =>
        {
            if (FocusManager.GetFocusedElement(XamlRoot) is TextBox)
                return;
            e.Handled = true;
            await PasteIntoJoin();
        };
        return key;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.Room(new RoomCommand.LoadRecent()));
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
            if (e is Event.RoomJoined && rejoining is not null)
                rejoining = null;
        if (batch.Any(e => e is Event.RoomJoined or Event.RoomState or Event.RoomMembers or Event.RoomLeft
            or Event.RoomRecent or Event.RoomGlimpse or Event.RoomLookFailed))
        {
            if (rejoining is null && batch.Any(e => e is Event.RoomLeft { Why: not RoomLeftWhy.Left and not RoomLeftWhy.Moved }))
                failedLink = ctx.Window.RoomInvite ?? join.Text.Trim();
            Show();
        }
    }

    async Task PasteIntoJoin()
    {
        var data = Clipboard.GetContent();
        if (data.Contains(StandardDataFormats.Text))
            join.Text = (await data.GetTextAsync()).Trim();
        join.Focus(FocusState.Programmatic);
    }

    /// <summary>An invite link: looked into first, so the room's episode and people show before Join.</summary>
    void Join(string text)
    {
        failedLink = null;
        // A link tried after a Rejoin is its own try: its failure shows here, not on the recent row.
        rejoining = null;
        // Another invite's look-in is not this one's: nothing shows until this link's answer.
        ctx.Known.ForgetRoomGlimpse();
        if (KaeriFfiMethods.ParseRoomTarget(text.Trim()) is not { } target)
        {
            failedLink = text.Trim();
            Show();
            return;
        }
        if (target.Old is not null)
        {
            failedLink = text.Trim();
            Show();
            return;
        }
        ctx.Window.RoomInvite = target.Link;
        ctx.Send(new Command.Room(new RoomCommand.PeekInvite(target.Link)));
        Show();
    }

    // The page's blocks by key, kept across redraws so focus and fades hold.

    readonly Dictionary<string, FrameworkElement> blocks = [];
    List<FrameworkElement> laying = [];

    T Block<T>(string key, Func<T> make, Action<T>? update = null) where T : FrameworkElement
    {
        if (!blocks.TryGetValue(key, out var kept) || kept is not T block)
        {
            block = make();
            blocks[key] = block;
            if (Stack.Children.Count > 0)
            {
                block.Opacity = 0;
                Motion.ShowStandard(block);
            }
        }
        update?.Invoke(block);
        laying.Add(block);
        return block;
    }

    TextBlock Line(string key, string text, string style = "HazeBodyStyle") =>
        Block(key, () => Ui.Text(text, style), t => t.Text = text);

    void Show()
    {
        laying = [];
        Block("heading", () =>
        {
            var made = Ui.Text(RoomWords.Title, "PageTitleStyle");
            AutomationProperties.SetHeadingLevel(made, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
            return made;
        });
        Line("place-line", T.RoomPlaceLine);
        if (ctx.Known.Room is { } room)
            YourRoom(room);
        else if (ctx.Known.RoomGlimpse is { } glimpse && ctx.Window.RoomInvite is not null)
            Invited(glimpse);
        JoinRow();
        Recent();
        var wanted = laying;
        for (var i = Stack.Children.Count - 1; i >= 0; i--)
            if (Stack.Children[i] is not FrameworkElement old || !wanted.Contains(old))
                Stack.Children.RemoveAt(i);
        for (var i = 0; i < wanted.Count; i++)
        {
            if (i < Stack.Children.Count && Stack.Children[i] == wanted[i])
                continue;
            var at = Stack.Children.IndexOf(wanted[i]);
            if (at >= 0)
                Stack.Children.RemoveAt(at);
            Stack.Children.Insert(i, wanted[i]);
        }
        foreach (var key in blocks.Where(b => !wanted.Contains(b.Value)).Select(b => b.Key).ToList())
            blocks.Remove(key);
    }

    /// <summary>The room you are in: its episode, its line, everyone's face and word, Open the player and Leave.</summary>
    void YourRoom(RoomView room)
    {
        var scene = new RoomScene(room, ctx.Known.RoomMembers, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
        var line = Rooms.Line(scene);
        Block("your-room", () => new Border
        {
            Background = Ui.Res<Brush>("RaisedBrush"),
            BorderBrush = Ui.Res<Brush>("HairBrush"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(16),
            Padding = new Thickness(16),
            Margin = new Thickness(0, 8, 0, 0),
        }, card =>
        {
            var grid = new Grid { ColumnSpacing = 18 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var text = new StackPanel();
            text.Children.Add(Ui.Text(T.RoomPlaceYours, "HazeCaptionStyle"));
            text.Children.Add(Ui.Text(room.RoomEpisode?.Title ?? "", "ShowTitleStyle", wrap: false));
            var first = Ui.Text(line.Title, "BodyStrongTextBlockStyle");
            first.Margin = new Thickness(0, 4, 0, 0);
            text.Children.Add(first);
            text.Children.Add(Ui.Text(line.Detail, "HazeCaptionStyle"));
            var people = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 10, 0, 0) };
            foreach (var row in Rooms.Rows(scene).Where(r => !r.Left))
            {
                var person = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                var face = new Kaeri.Windows.Player.RoomFace(22);
                face.Show(row.Name, row.Face);
                person.Children.Add(face);
                var words = new TextBlock { FontSize = 12, VerticalAlignment = VerticalAlignment.Center };
                words.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = row.You ? RoomWords.YouAlone : row.Name });
                words.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = " " + row.Word, Foreground = Ui.Res<Brush>("HazeBrush") });
                person.Children.Add(words);
                AutomationProperties.SetName(person, row.Spoken);
                people.Children.Add(person);
            }
            text.Children.Add(people);
            grid.Children.Add(text);
            var buttons = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
            buttons.Children.Add(Ui.Label(T.RoomOpenPlayer, (_, _) => ctx.Send(new Command.Room(new RoomCommand.OpenPlayer())), accent: true));
            buttons.Children.Add(Ui.Label(T.RoomLeaveEllipsis, async (_, _) =>
            {
                if (XamlRoot is { } root && await Kaeri.Windows.Player.RoomPopups.AskLeave(root))
                    ctx.Send(new Command.Room(new RoomCommand.Leave()));
            }));
            Grid.SetColumn(buttons, 1);
            grid.Children.Add(buttons);
            card.Child = grid;
        });
    }

    /// <summary>An invite looked into: who invited, what the room watches and who is there, with Join and Not now.</summary>
    void Invited(RoomGlimpse glimpse)
    {
        var line = Rooms.Line(Rooms.Invited(glimpse, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()));
        Block("invited", () => new Border
        {
            Background = Ui.Res<Brush>("RaisedBrush"),
            BorderBrush = Ui.Res<Brush>("HairBrush"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(16),
            Padding = new Thickness(16),
            Margin = new Thickness(0, 8, 0, 0),
        }, card =>
        {
            var text = new StackPanel { Spacing = 2 };
            text.Children.Add(Ui.Text(line.Title, "BodyStrongTextBlockStyle"));
            text.Children.Add(Ui.Text(line.Detail, "HazeCaptionStyle"));
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 10, 0, 0) };
            row.Children.Add(Ui.Label(line.AskText ?? RoomWords.JoinRoom, (_, _) =>
            {
                if (ctx.Window.RoomInvite is { } invite)
                    ctx.Send(new Command.Room(new RoomCommand.Lobby(Invite: invite, Anyway: line.Ask == RoomAsk.JoinAnyway)));
            }, accent: true));
            row.Children.Add(Ui.Label(RoomWords.NotNow, (_, _) =>
            {
                ctx.Window.RoomInvite = null;
                ctx.Known.ForgetRoomGlimpse();
                Show();
            }));
            text.Children.Add(row);
            card.Child = text;
        });
    }

    /// <summary>Join a room: the link field with Paste and Join, or after a failed link, the link kept with Try again and why.</summary>
    void JoinRow()
    {
        Block("section-join", () => Section(T.RoomPlaceJoin));
        var failed = failedLink is { } f && f == join.Text.Trim();
        Block("join-row", () => new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 }, row =>
        {
            // The field stays put, so typing keeps its focus, selection and IME composition;
            // only the buttons after it change.
            if (join.Parent is Panel old && old != row)
                old.Children.Remove(join);
            if (row.Children.Count == 0 || row.Children[0] != join)
            {
                row.Children.Remove(join);
                row.Children.Insert(0, join);
            }
            while (row.Children.Count > 1)
                row.Children.RemoveAt(1);
            if (failed)
                row.Children.Add(Ui.Label(T.ActionTryAgain, (_, _) => Join(join.Text)));
            else
            {
                row.Children.Add(Ui.Label(T.RoomPlacePaste, async (_, _) => await PasteIntoJoin()));
                var go = Ui.Label(T.ActionJoin, (_, _) => Join(join.Text));
                go.IsEnabled = join.Text.Trim().Length > 0;
                row.Children.Add(go);
            }
        });
        if (failed)
        {
            var (lead, then) = ctx.Known.RoomLeftWhy is { } left ? RoomWords.LeftLine(left)
                : KaeriFfiMethods.ParseRoomTarget(join.Text.Trim()) is { Old: not null } ? (T.RoomErrorOldInvite, null)
                : RoomWords.LeftLine(new Kaeri.Engine.Model.RoomLeftWhy.Unreachable());
            Block("join-failed", () => Failure(), b => SetFailure(b, lead, then));
        }
    }

    static StackPanel Failure() => new() { Orientation = Orientation.Horizontal, Spacing = 8 };

    static void SetFailure(StackPanel at, string first, string? then)
    {
        at.Children.Clear();
        at.Children.Add(new Microsoft.UI.Xaml.Shapes.Ellipse { Width = 7, Height = 7, Fill = new SolidColorBrush(Ui.Vermilion), VerticalAlignment = VerticalAlignment.Center });
        var text = new TextBlock { FontSize = 13, TextWrapping = TextWrapping.WrapWholeWords, MaxWidth = 620 };
        text.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = first, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        if (then is not null)
            text.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = " " + then, Foreground = Ui.Res<Brush>("HazeBrush") });
        at.Children.Add(text);
    }

    /// <summary>Recent rooms: the last ten on this device, each with Rejoin; one whose look-in found nobody says so.</summary>
    void Recent()
    {
        Block("section-recent", () =>
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 10, 0, 0) };
            var heading = Ui.Text(T.RoomRecent, "BodyStrongTextBlockStyle");
            AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
            row.Children.Add(heading);
            var note = Ui.Text(T.RoomRecentNote, "HazeCaptionStyle");
            note.VerticalAlignment = VerticalAlignment.Bottom;
            row.Children.Add(note);
            return row;
        });
        Block("recent", () => recentState);
        if (!ctx.Known.RoomRecentLoaded)
        {
            recentState.Wait();
            return;
        }
        recentState.Done();
        recentRows.Children.Clear();
        var first = true;
        foreach (var room in ctx.Known.RoomRecent)
        {
            recentRows.Children.Add(RecentRow(room, first));
            first = false;
        }
    }

    FrameworkElement RecentRow(RoomRecent room, bool first)
    {
        // The page column's width, so Rejoin sits at its right edge and never past it.
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 56 };
        if (!first)
        {
            grid.BorderBrush = Ui.Res<Brush>("FaintRuleBrush");
            grid.BorderThickness = new Thickness(0, 1, 0, 0);
        }
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var art = new Border { Width = 64, Height = 36, CornerRadius = new CornerRadius(6), Background = Ui.Res<Brush>("RaisedBrush"), VerticalAlignment = VerticalAlignment.Center };
        if (room.CoverUrl is { Length: > 0 } cover && Uri.TryCreate(cover, UriKind.Absolute, out var uri))
            art.Child = new Image { Source = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(uri), Stretch = Stretch.UniformToFill };
        grid.Children.Add(art);
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Padding = new Thickness(0, 10, 0, 10) };
        text.Children.Add(Ui.Text(room.Title, "BodyStrongTextBlockStyle", wrap: false));
        var names = room.Names ?? [];
        var when = When(room.LastAt);
        text.Children.Add(Ui.Text(names.Length > 0 ? T.RoomRecentRow(RoomWords.N(room.Episode), Rooms.Names(names), when) : T.RoomRecentRowAlone(RoomWords.N(room.Episode), when), "HazeCaptionStyle", wrap: false));
        var failed = rejoining == room.RoomId && ctx.Known.RoomLeftWhy is not null;
        if (failed)
        {
            var why = new StackPanel { Margin = new Thickness(0, 4, 0, 0) };
            var said = Failure();
            SetFailure(said, T.RoomErrorNobodyAnsweredLead, T.RoomErrorNobodyAnsweredLine);
            why.Children.Add(said);
            text.Children.Add(why);
        }
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        var rejoin = Ui.Label(failed ? T.ActionTryAgain : T.RoomRejoin, (_, _) =>
        {
            rejoining = room.RoomId;
            ctx.Send(new Command.Room(new RoomCommand.Rejoin(room.RoomId)));
            Show();
        });
        rejoin.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(rejoin, 2);
        grid.Children.Add(rejoin);
        AutomationProperties.SetName(grid, room.Title);
        return grid;
    }

    /// <summary>When a room was last: the time today, the weekday this week, else the date.</summary>
    static string When(long epochSeconds)
    {
        var at = DateTimeOffset.FromUnixTimeSeconds(epochSeconds).LocalDateTime;
        var today = DateTime.Today;
        if (at.Date == today)
            return at.ToString("t");
        if (at.Date > today.AddDays(-7))
            return at.ToString("dddd");
        return at.ToString("M");
    }

#if KAERI_TESTING
    /// <summary>A shot's: the link pasted into Join a room, as if typed.</summary>
    internal void ShotFill(string link) => join.Text = link;

    /// <summary>A shot's: the link typed into Join a room a character at a time, the field focused.</summary>
    internal async Task ShotType(string link)
    {
        join.Focus(FocusState.Keyboard);
        foreach (var c in link)
        {
            join.Text += c;
            join.SelectionStart = join.Text.Length;
            await Task.Delay(60);
        }
    }
#endif

    static TextBlock Section(string text)
    {
        var block = Ui.Text(text, "BodyStrongTextBlockStyle");
        block.Margin = new Thickness(0, 10, 0, 0);
        AutomationProperties.SetHeadingLevel(block, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        return block;
    }
}
