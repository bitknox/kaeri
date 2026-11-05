using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Player;

/// <summary>
/// The party, from the player's people button: how the connection stands, who is in the room
/// and who is ready, this member's own ready switch, the invite link to copy, the server and
/// room as text for someone joining with Syncplay itself, and Leave party. Under Ready, the
/// party's conversation and its field. A member's row offers Mute from its context menu. It
/// follows the engine while it is open, and each part keeps its place as the room changes.
/// </summary>
sealed class PartyPanel
{
    readonly Ctx ctx;
    public Flyout Flyout { get; }
    readonly TextBlock connection = Ui.Text("", "HazeCaptionStyle");
    readonly ProgressRing reconnecting = new() { Width = 12, Height = 12, IsActive = false, Opacity = 0 };
    readonly StackPanel members = new() { Spacing = 2 };
    readonly TextBlock readyLine = Ui.Text("", "HazeCaptionStyle");
    readonly ToggleSwitch ready = new() { OnContent = T.PartyReadyAction, OffContent = T.PartyReadyAction };
    readonly Button copy = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    readonly TextBlock copyText = new() { Text = T.PartyCopyInvite };
    readonly TextBlock inviteNote = Ui.Text("", "HazeCaptionStyle");
    readonly TextBlock place = new() { IsTextSelectionEnabled = true, TextWrapping = TextWrapping.Wrap };
    readonly TextBlock motd = Ui.Text("", "HazeCaptionStyle");
    readonly Button leave = new() { Content = T.PartyLeave, HorizontalAlignment = HorizontalAlignment.Stretch };
    readonly ChatPane chat;
    bool fromEngine, leaving;
    int copied;

    public PartyPanel(Ctx ctx)
    {
        this.ctx = ctx;
        var body = new StackPanel { Spacing = 10, Width = 320 };

        var head = new StackPanel { Spacing = 2 };
        head.Children.Add(Ui.Text(T.PartyTitle, "BodyStrongTextBlockStyle"));
        // The ring follows the line, so the line starts where the title does.
        var status = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        status.Children.Add(connection);
        reconnecting.VerticalAlignment = VerticalAlignment.Center;
        status.Children.Add(reconnecting);
        head.Children.Add(status);
        body.Children.Add(head);

        body.Children.Add(members);
        body.Children.Add(readyLine);
        AutomationProperties.SetLiveSetting(readyLine, AutomationLiveSetting.Polite);
        AutomationProperties.SetName(ready, T.PartyReadyAction);
        ready.Toggled += (_, _) =>
        {
            if (!fromEngine)
                ctx.Send(new Command.PartyReady(ready.IsOn));
        };
        body.Children.Add(ready);

        body.Children.Add(Rule());
        chat = new ChatPane(ctx, lobby: false);
        body.Children.Add(chat);
        body.Children.Add(Rule());

        var copyContent = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        copyContent.Children.Add(new FontIcon { Glyph = "", FontSize = 14 });
        copyContent.Children.Add(copyText);
        copy.Content = copyContent;
        copy.Click += (_, _) => Copy();
        body.Children.Add(copy);
        body.Children.Add(inviteNote);

        var plain = new StackPanel { Spacing = 2 };
        plain.Children.Add(Ui.Text(T.PartyOtherApp, "CaptionTextBlockStyle"));
        place.Style = Ui.Res<Style>("HazeCaptionStyle");
        plain.Children.Add(place);
        body.Children.Add(plain);
        body.Children.Add(motd);

        leave.Click += (_, _) =>
        {
            leaving = true;
            leave.Content = T.PartyLeaving;
            leave.IsEnabled = false;
            ctx.Send(new Command.PartyLeave());
        };
        body.Children.Add(leave);

        // The whole flyout hangs under the people button: when the window is short it scrolls
        // rather than moving up over the top bar and the button that opened it.
        var scroll = new ScrollViewer { Content = body, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
        Flyout = new Flyout { Content = scroll, Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        Flyout.Opening += (_, _) =>
        {
            if (Flyout.Target is { XamlRoot: { } root } target)
            {
                var below = target.TransformToVisual(null).TransformPoint(new global::Windows.Foundation.Point(0, target.ActualHeight)).Y;
                // The presenter's padding and border, and a gap above the window's edge.
                scroll.MaxHeight = Math.Max(160, root.Size.Height - below - 48);
            }
            leaving = false;
            leave.Content = T.PartyLeave;
            Show();
            ctx.Batch += Heard;
        };
        Flyout.Closed += (_, _) =>
        {
            ctx.Batch -= Heard;
            ChatSetting.Changed -= chat.Refresh;
        };
        Flyout.Opened += (_, _) => ChatSetting.Changed += chat.Refresh;
    }

    static Border Rule() => new() { Height = 1, Background = Ui.Res<Brush>("DividerStrokeColorDefaultBrush"), Margin = new Thickness(0, 2, 0, 2) };

    /// <summary>The conversation's field, for the player to focus.</summary>
    internal ChatField Field => chat.Field;

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.PartyLeft))
        {
            Flyout.Hide();
            return;
        }
        if (batch.Any(e => e is Event.PartyState or Event.PartyJoined or Event.PartyMembers))
            Show();
        else if (batch.Any(e => e is Event.PartyChatState))
            ShowMembers();
        if (batch.Any(e => e is Event.PartyChat or Event.PartyChatState))
            chat.Refresh();
    }

    void Show()
    {
        if (ctx.Known.Party is not { } party)
            return;
        var lost = party.Connection is PartyConnection.Reconnecting;
        connection.Text = Parties.ConnectionLine(party);
        reconnecting.IsActive = lost;
        reconnecting.Opacity = lost ? 1 : 0;

        ShowMembers();
        chat.Refresh();

        fromEngine = true;
        ready.IsOn = party.Ready;
        fromEngine = false;
        ready.IsEnabled = !lost && !leaving;

        copy.IsEnabled = party.Invite is not null && !leaving;
        inviteNote.Text = party.Invite is null
            ? T.PartyInviteAfterPlay
            : T.PartyInviteDetail;
        place.Text = T.PartyOtherAppDetail(party.Server, party.Room);
        motd.Text = party.Motd ?? "";
        motd.Visibility = party.Motd is { Length: > 0 } ? Visibility.Visible : Visibility.Collapsed;
        leave.IsEnabled = !leaving;
    }

    void ShowMembers()
    {
        members.Children.Clear();
        if (ctx.Known.PartyMembers is { } list)
        {
            var muted = ctx.Known.PartyChatState?.Muted ?? [];
            foreach (var member in list)
                members.Children.Add(Row(member, muted.Contains(member.Name)));
            readyLine.Text = Parties.ReadyLine(list);
        }
        else
        {
            // Before the room lists its members: one row's height, waiting.
            var wait = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, MinHeight = 36 };
            wait.Children.Add(new ProgressRing { Width = 16, Height = 16, IsActive = true });
            wait.Children.Add(Ui.Text(T.PartyWaitingMembers, "HazeBodyStyle"));
            members.Children.Add(wait);
            readyLine.Text = "";
        }
    }

    /// <summary>
    /// A member: ready or not, and Muted when this device mutes them. Another member's row
    /// offers Mute or Unmute from its context menu (a secondary click, Shift+F10 or the menu key).
    /// </summary>
    FrameworkElement Row(PartyMember member, bool muted)
    {
        var row = new Grid { ColumnSpacing = 10, MinHeight = 36, Padding = new Thickness(0, 2, 0, 2) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(20) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var isReady = member.Ready == true;
        var mark = new FontIcon
        {
            // A filled check for ready, an empty circle for not yet.
            Glyph = isReady ? "" : "",
            FontSize = 14,
            Opacity = isReady ? 1 : 0.6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        row.Children.Add(mark);
        var names = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        names.Children.Add(Ui.Text(Parties.MemberName(member), wrap: false));
        if (Parties.MemberFile(member) is { } file)
            names.Children.Add(Ui.Text(file, "HazeCaptionStyle", wrap: false));
        Grid.SetColumn(names, 1);
        row.Children.Add(names);
        var said = muted ? T.PartyMemberMuted : Parties.MemberState(member);
        var state = Ui.Text(said, isReady && !muted ? "CaptionTextBlockStyle" : "HazeCaptionStyle", wrap: false);
        state.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(state, 2);
        row.Children.Add(state);
        AutomationProperties.SetName(row, $"{Parties.MemberName(member)}, {said}");
        if (!member.You && (ctx.Known.PartyChatState?.Allowed ?? true) && !ChatSetting.Hidden)
        {
            var name = member.Name;
            var item = new MenuFlyoutItem { Text = muted ? T.PartyUnmute(name) : T.PartyMute(name) };
            item.Click += (_, _) => ctx.Send(new Command.Party(new PartyCommand.Mute(name, !muted)));
            var menu = new MenuFlyout();
            menu.Items.Add(item);
            row.ContextFlyout = menu;
            // Reachable from the keyboard: the row takes focus, and its menu opens on Shift+F10.
            row.IsTabStop = true;
            row.UseSystemFocusVisuals = true;
        }
        return row;
    }

    async void Copy()
    {
        if (ctx.Known.Party?.Invite is not { } link)
            return;
        if (!Pages.Ui.TryCopy(link))
            return;
        var mine = ++copied;
        copyText.Text = T.ActionCopied;
        await Task.Delay(2000);
        if (mine == copied)
            copyText.Text = T.PartyCopyInvite;
    }
}
