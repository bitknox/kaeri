using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Watch together as a place of its own. Out of a party: start one, join with a link or code, and the
/// last rooms. In one: the lobby, which says what the room is watching, who is ready and what is
/// left to do. One amber on the page, on the next thing to do; once you are ready it turns tonal.
/// </summary>
public sealed partial class TogetherPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    readonly TextBox join = new() { PlaceholderText = T.TogetherJoinField, MinWidth = 280 };
    readonly TextBlock joinProblem = Ui.Text("", "HazeCaptionStyle");
    ChatPane? chat;
    bool inLobby;

    // The last rooms and a party's members, each kept across redraws so its 600 ms is not started
    // again. The rooms are kept on this device, and the party says itself when the connection drops.
    readonly StateHost recentState = new() { Placeholders = () => States.Rows(2, 0, 0, rowHeight: 56, spacing: 14), NeedsConnection = false };
    readonly StackPanel recentRows = new() { Spacing = 14 };
    readonly StateHost membersState = new() { Placeholders = () => States.Rows(2, 32, 32, rowHeight: 44, spacing: 14), NeedsConnection = false };
    readonly StackPanel memberRows = new() { Spacing = 14 };

    public TogetherPage()
    {
        InitializeComponent();
        recentState.Children.Add(recentRows);
        joinProblem.Visibility = Visibility.Collapsed;
        membersState.Children.Add(memberRows);
        AutomationProperties.SetName(join, T.TogetherJoinField);
        join.KeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Enter)
                Join(join.Text);
        };
        ctx.Hear(this, Heard, () => ChatSetting.Changed -= ChatChanged);
        ChatSetting.Changed += ChatChanged;
        // Ctrl+V outside a field pastes into Join.
        Loaded += (_, _) => KeyboardAccelerators.Add(Paste());
        // Not named in a tooltip over the whole page; the paste hint under the field says it.
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
    }

    KeyboardAccelerator Paste()
    {
        var key = new KeyboardAccelerator { Key = VirtualKey.V, Modifiers = VirtualKeyModifiers.Control };
        key.Invoked += async (_, e) =>
        {
            if (FocusManager.GetFocusedElement(XamlRoot) is TextBox || ctx.Known.Party is not null)
                return;
            e.Handled = true;
            await PasteIntoJoin();
        };
        return key;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.LoadPartyForm());
        ctx.Send(new Command.Party(new PartyCommand.LoadRecent()));
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.PartyJoined or Event.PartyState or Event.PartyMembers or Event.PartyLeft
            or Event.PartyRecent or Event.PartyGlimpse or Event.PartyLookFailed or Event.PartyForm or Event.PartyLastOut))
            Show();
        // The conversation changes in place, so its field keeps focus and what is typed.
        else if (batch.Any(e => e is Event.PartyChat or Event.PartyChatState))
            chat?.Refresh();
    }

    void ChatChanged() => chat?.Refresh();

    async Task PasteIntoJoin()
    {
        var data = Clipboard.GetContent();
        if (data.Contains(StandardDataFormats.Text))
            join.Text = (await data.GetTextAsync()).Trim();
        join.Focus(FocusState.Programmatic);
    }

    /// <summary>A link or a code. A code implies the suggested server; a link carries its own.</summary>
    void Join(string text)
    {
        if (KaeriFfiMethods.ParsePartyTarget(text) is not { } target)
        {
            joinProblem.Text = T.PartyCodeShape("kite-7qm4-xr9d");
            joinProblem.Visibility = Visibility.Visible;
            return;
        }
        joinProblem.Visibility = Visibility.Collapsed;
        var server = target.Server.Length > 0 ? target.Server : ctx.Known.PartyForm?.SuggestedServer ?? "";
        ctx.Send(new Command.Party(new PartyCommand.Lobby(server, target.Room, ctx.Known.PartyForm?.Name, null)));
    }

    /// <summary>The party dialog for a room with nothing picked: in it, Play on any episode picks for the room.</summary>
    void StartParty() => ctx.Window.ShowParty(new PartyDialog.For.Room());

    // The page is a column of blocks, each kept under a key across party events (about one a
    // second while the room counts down): a block whose key and look are the same stays the same
    // element, so a click in progress, the keyboard's place and the chat's field are kept. Only a
    // block that changed is made again, and text changes in place.

    readonly Dictionary<string, FrameworkElement> blocks = [];
    List<FrameworkElement> laying = [];

    /// <summary>The block under <paramref name="key"/>: kept, or made, then brought up to date.</summary>
    T Block<T>(string key, Func<T> make, Action<T>? update = null) where T : FrameworkElement
    {
        if (!blocks.TryGetValue(key, out var kept) || kept is not T block)
        {
            block = make();
            blocks[key] = block;
        }
        update?.Invoke(block);
        laying.Add(block);
        return block;
    }

    TextBlock Line(string key, string text, string style = "HazeBodyStyle") =>
        Block(key, () => Ui.Text(text, style), t => t.Text = text);

    /// <summary>A button whose action and words are its key: the same one stays put.</summary>
    Button Act(string key, string text, Action act, bool accent = false) =>
        Block($"{key}|{text}|{accent}", () => Ui.Label(text, (_, _) => act(), accent));

    void Show()
    {
        laying = [];
        Block("heading", () =>
        {
            var made = Ui.Text(T.TogetherTitle, "PageTitleStyle");
            AutomationProperties.SetHeadingLevel(made, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
            return made;
        });
        if (ctx.Known.Party is { } party)
        {
            inLobby = true;
            Lobby(party);
        }
        else
        {
            inLobby = false;
            Place();
        }
        // The column as laid now: what stays is moved only when its place changed.
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

    // --- Out of a party ---

    void Place()
    {
        Line("place-line", T.TogetherLine);

        Block("section-start", () => Section(T.TogetherStart));
        Line("start-line", T.TogetherStartLine);
        Act("start", T.PartyStartParty, StartParty, accent: true);

        Block("section-join", () => Section(T.TogetherJoin));
        Line("join-line", T.TogetherJoinLine);
        Block("join-row", () =>
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            if (join.Parent is Panel old)
                old.Children.Remove(join);
            row.Children.Add(join);
            row.Children.Add(Ui.Label(T.TogetherPaste, async (_, _) => await PasteIntoJoin()));
            row.Children.Add(Ui.Label(T.TogetherJoinAction, (_, _) => Join(join.Text)));
            return row;
        });
        Block("join-problem", () => joinProblem);
        Line("join-hint", T.TogetherPasteShortcutHint);

        Block("section-recent", () => Section(T.TogetherRecent));
        Block("recent", () => recentState);
        if (!ctx.Known.PartyRecentLoaded)
            recentState.Wait();
        else
        {
            recentState.Done();
            var recent = ctx.Known.PartyRecent;
            Rows(recentRows, recent.Length == 0
                ? [("empty", () => Ui.Haze(T.TogetherRecentEmpty))]
                : recent.Select(r => ($"{r.Server}|{r.Room}|{r.Title}|{r.Episode}|{string.Join(",", r.Names)}|{r.LastAt}", (Func<FrameworkElement>)(() => Recent(r)))).ToArray());
        }
        if (ctx.Known.PartyLookFailure is { } failure)
            Block($"look-failure|{failure}", () => new InfoBar { Severity = InfoBarSeverity.Warning, IsOpen = true, IsClosable = false, Message = failure });
    }

    /// <summary>A list's rows by key: a row that is still there stays the same element, in its new place.</summary>
    static void Rows(Panel panel, (string Key, Func<FrameworkElement> Make)[] rows)
    {
        var old = panel.Children.OfType<FrameworkElement>().Where(e => e.Tag is string).ToDictionary(e => (string)e.Tag, e => e);
        var wanted = rows.Select(r =>
        {
            if (old.TryGetValue(r.Key, out var kept))
                return kept;
            var made = r.Make();
            made.Tag = r.Key;
            return made;
        }).ToList();
        for (var i = panel.Children.Count - 1; i >= 0; i--)
            if (panel.Children[i] is not FrameworkElement e || !wanted.Contains(e))
                panel.Children.RemoveAt(i);
        for (var i = 0; i < wanted.Count; i++)
        {
            if (i < panel.Children.Count && panel.Children[i] == wanted[i])
                continue;
            var at = panel.Children.IndexOf(wanted[i]);
            if (at >= 0)
                panel.Children.RemoveAt(at);
            panel.Children.Insert(i, wanted[i]);
        }
    }

    FrameworkElement Recent(PartyRecentRoom room)
    {
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 56 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(room.Title, "BodyStrongTextBlockStyle", wrap: false));
        var when = DateTimeOffset.FromUnixTimeSeconds(room.LastAt).LocalDateTime.ToString("d");
        text.Children.Add(Ui.Text(T.TogetherRecentRow(room.Episode, string.Join(", ", room.Names), when), "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var rejoin = Ui.Label(T.TogetherRejoin, (_, _) =>
            ctx.Send(new Command.Party(new PartyCommand.Lobby(room.Server, room.Room, ctx.Known.PartyForm?.Name, null))));
        Grid.SetColumn(rejoin, 1);
        grid.Children.Add(rejoin);
        var forget = Ui.Label(T.TogetherForgetRoom, (_, _) =>
            ctx.Send(new Command.Party(new PartyCommand.ForgetRoom(room.Server, room.Room))));
        Grid.SetColumn(forget, 2);
        grid.Children.Add(forget);
        return grid;
    }

    // --- In a party: the lobby ---

    void Lobby(PartyView party)
    {
        var members = ctx.Known.PartyMembers ?? [];
        if (party.Connection is PartyConnection.Reconnecting retry)
            Block("reconnecting", () => new InfoBar { Severity = InfoBarSeverity.Warning, IsOpen = true, IsClosable = false },
                bar => bar.Message = T.PartyConnectionLost(retry.Attempt, retry.Of));
        if (party.Starter is { Length: > 0 } starter && starter != party.Name)
            Line("invited-by", T.PartyInvitedBy(starter));

        // What the room is watching, from its playlist: nobody picks a title to join.
        Block("section-watching", () => Section(T.PartyRoomWatching));
        if (party.RoomEpisode is { } room)
        {
            Line("room-title", room.Title.Length > 0 ? room.Title : room.File, "SubtitleTextBlockStyle");
            Line("room-episode", T.EpisodeNumber(long.TryParse(room.Episode, out var n) ? n : 0));
            if (room.Found == PartyFound.Name)
                Line("room-found", T.PartyRoomFoundByName);
            else if (room.Found == PartyFound.FileName)
            {
                Line("room-found", T.PartyRoomMatched);
                Act("not-this", T.PartyRoomNotThis, () => ctx.Send(new Command.Party(new PartyCommand.ChooseTitle(null))));
            }
            else if (room.Found == PartyFound.Unmatched)
            {
                Line("room-found", T.PartyRoomPlainFile(room.PlainBy ?? "", room.File));
                if (party.Guesses.Length > 0)
                    Line("guesses", T.PartyRoomGuesses);
                foreach (var guess in party.Guesses)
                {
                    var id = guess.Id;
                    Act($"guess-{id}", guess.Title, () => ctx.Send(new Command.Party(new PartyCommand.ChooseTitle(id))));
                }
            }
        }
        else
        {
            Line("room-title", T.PartyRoomNothing, "SubtitleTextBlockStyle");
            Line("room-episode", T.PartyRoomNothingLine);
            // Picked as anything is played: browsing the app, where Play is the room's.
            Act("pick", T.PartyRoomPick, () => ctx.Window.Go("home"));
        }

        // Your own sources, checked for that episode. Never anyone else's.
        switch (party.Sources)
        {
            case PartySources.Checking:
                Line("sources", T.PartyLookingIn);
                break;
            case PartySources.Found:
                Line("sources", T.PartyLobbySourcesFound);
                break;
            case PartySources.Missing missing:
                Line("sources", missing.Reason.Text(), "BodyTextBlockStyle");
                Act("sources-missing", T.PartyLobbySourcesMissing, () => ctx.Window.Go("providers"));
                break;
        }

        // Members: you first, then in the order they joined. A state in words, never a source.
        Block("section-members", () => Section(T.PartyWaitingMembers));
        Block("members", () => membersState);
        if (ctx.Known.PartyMembers is null)
            membersState.Wait();
        else
            membersState.Done();
        Rows(memberRows, members.OrderBy(m => m.You ? 0 : 1)
            .Select(m => ($"{m.Name}|{m.You}|{m.Ready}|{m.File is null}", (Func<FrameworkElement>)(() => MemberRow(m)))).ToArray());
        var ready = members.Count(m => m.Ready == true);
        // No count before the room has listed anyone.
        if (ctx.Known.PartyMembers is not null)
            Line("count", T.PartyLobbyCount(ready, members.Length));
        Line("starts", party.StartingIn is { } secs ? T.PartyStarting(secs) : party.Playing || members.Length > 0 && ready == members.Length ? T.PartyAllReady : T.PartyStartsLine);

        // While the room plays, Open player is the way back to it, as on the Mac and Android.
        // Before, Ready is the one amber; once ready it is tonal with a check and the word.
        var actionsKey = party.Playing ? "open" : $"ready-{party.Ready}-{party.CanStartAnyway}";
        Block($"actions|{actionsKey}", () =>
        {
            var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
            if (party.Playing)
                actions.Children.Add(Ui.Label(T.TogetherOpenPlayer, (_, _) => ctx.Send(new Command.Party(new PartyCommand.OpenPlayer())), accent: true));
            else
            {
                var readyButton = party.Ready
                    ? Ui.Label("✓ " + T.PartyReadyOn, (_, _) => ctx.Send(new Command.PartyReady(false)))
                    : Ui.Label(T.PartyReadyAction, (_, _) => ctx.Send(new Command.PartyReady(true)), accent: true);
                actions.Children.Add(readyButton);
                if (party.CanStartAnyway)
                    actions.Children.Add(Ui.Label(T.PartyStartAnyway, (_, _) => ctx.Send(new Command.Party(new PartyCommand.StartAnyway()))));
            }
            return actions;
        });

        // Invite: the https link and the code, for reading out.
        if (party.Link is { } link)
        {
            Block("section-invite", () => Section(T.PartyShareInvite));
            Act($"copy-invite|{link}", T.PartyCopyInvite, () =>
            {
                if (Ui.TryCopy(link))
                    ctx.Window.Say(T.PartyInviteCopied);
            });
            if (party.Code is { } code)
                Line("code", T.PartyCode + " " + code, "BodyStrongTextBlockStyle");
        }
        // The conversation under the room, with its field and, once, where the messages go.
        Block("section-chat", () => Section(T.PartyChatTitle));
        chat ??= new ChatPane(ctx, lobby: true) { MaxWidth = 480, HorizontalAlignment = HorizontalAlignment.Stretch };
        chat.Refresh();
        Block("chat", () => chat);

        var leave = Act("leave", T.PartyLeave, () => ctx.Send(new Command.PartyLeave()));
        leave.Margin = new Thickness(0, 12, 0, 0);
    }

    static FrameworkElement MemberRow(PartyMember member)
    {
        var state = member.Ready switch
        {
            true => "✓ " + T.PartyMemberReady,
            false => T.PartyMemberNotReady,
            null => member.File is null ? T.PartyMemberNoFile : T.PartyMemberJoining,
        };
        var grid = new Grid { ColumnSpacing = 12, Height = 44 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        // Initials on a raised circle, never a colour of their own.
        var circle = new Border
        {
            Width = 32, Height = 32, CornerRadius = new CornerRadius(16), VerticalAlignment = VerticalAlignment.Center,
            Background = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("CardBackgroundFillColorSecondaryBrush"),
            Child = new TextBlock
            {
                Text = member.Name.Length > 0 ? member.Name[..1].ToUpperInvariant() : "?",
                HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
            },
        };
        AutomationProperties.SetAccessibilityView(circle, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        grid.Children.Add(circle);
        var name = Ui.Text(member.You ? T.PartyMemberYou(member.Name) : member.Name, "BodyTextBlockStyle", wrap: false);
        name.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(name, 1);
        grid.Children.Add(name);
        var status = Ui.Haze(state, wrap: false);
        status.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(status, 2);
        grid.Children.Add(status);
        AutomationProperties.SetName(grid, $"{member.Name}, {state}");
        return grid;
    }

#if KAERI_TESTING
    /// <summary>A shot's: the page scrolled to its end, where the lobby's conversation is.</summary>
    internal void ShotScrollToEnd()
    {
        Scroll.UpdateLayout();
        Scroll.ChangeView(null, Scroll.ScrollableHeight, null, true);
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
