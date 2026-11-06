using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows;

/// <summary>
/// Watch parties, the window's side: the dialog that starts one or answers an invite link, the
/// dialog for an episode none of the sources has, what the window says as a party starts and
/// ends, and asking before closing the window leaves one.
/// </summary>
public sealed partial class MainWindow
{
    bool inParty, askedParty;

    /// <summary>The dialog that starts a party, or joins one from an invite.</summary>
    internal async void ShowParty(PartyDialog.For what, Action<PartyDialog>? shown = null)
    {
        if (CloseDialogs())
            await Task.Delay(250);
        try
        {
            await new PartyDialog(ctx, Root.XamlRoot, what).ShowAsync(shown);
        }
        catch (COMException)
        {
            // Another dialog came up first.
        }
    }

    /// <summary>An <c>kaeri://party</c> link: its title's page, and the dialog that asks Join.</summary>
    void OpenPartyInvite(string text)
    {
        if (KaeriFfiMethods.ParsePartyTarget(text) is not { } target)
        {
            ShowToast(InfoBarSeverity.Warning, T.PartyLinkIncomplete);
            return;
        }
        if (ctx.Known.Party is { } party && party.Server == target.Server && party.Room == target.Room)
        {
            ShowToast(InfoBarSeverity.Informational, T.PartyAlreadyIn);
            return;
        }
        // The lobby opens in its invited state: it looks in, reads what the room watches and waits for Join.
        var server = target.Server.Length > 0 ? target.Server : ctx.Known.PartyForm?.SuggestedServer ?? "";
        ctx.Send(new Command.Party(new PartyCommand.Lobby(server, target.Room, ctx.Known.PartyForm?.Name, null)));
        Go("together");
    }

    async void ShowPartySource(uint id, string episode, NoSourceWhy why, string reason)
    {
        if (CloseDialogs())
            await Task.Delay(250);
        try
        {
            await new PartySourceDialog(ctx, Root.XamlRoot, id, episode, why, reason).ShowAsync();
        }
        catch (COMException)
        {
        }
    }

    void PartyHeard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.PartyJoined when !inParty:
                    inParty = true;
                    // The player covers the window's toasts: over the video, it says so itself.
                    if (Player.Showing)
                        Player.Say(T.PartyInNote);
                    else
                        ShowToast(InfoBarSeverity.Success, T.PartyInNotePlayer);
                    break;
                case Event.PartyLeft left:
                    // While the dialog is still joining, it says why itself, and over the video
                    // the engine's own notice does.
                    if (inParty && PartyDialog.Showing is null && !closing && !Player.Showing)
                    {
                        if (left.Reason is { } why)
                            ShowToast(InfoBarSeverity.Warning, why.Text());
                        else
                            ShowToast(InfoBarSeverity.Informational, T.PartyLeftNote);
                    }
                    inParty = false;
                    break;
                case Event.PartyMessage { V1: var message }:
                    // What the room did, in the engine's words: over the video it is the player's notice.
                    if (message.Kind is PartyMessageKind.Chat or PartyMessageKind.Corrected or PartyMessageKind.AllReady)
                        break;
                    if (Player.Showing)
                        Player.Say(message.Text);
                    else if (message.Kind is not (PartyMessageKind.Paused or PartyMessageKind.Resumed or PartyMessageKind.Seeked))
                        ShowToast(InfoBarSeverity.Informational, message.Text);
                    break;
                case Event.PartyNoSource none:
                    ShowPartySource(none.Id, none.Episode, none.Why, none.Reason.Text());
                    break;
            }
        }
        inParty = ctx.Known.Party is not null;
        UpdatePartyBar();
    }

    /// <summary>The bar above the pages while in a party: the room's episode, who is ready, a way back and out.</summary>
    internal void UpdatePartyBar()
    {
        var onTogether = ContentFrame.Content is TogetherPage;
        if (ctx.Known.Party is not { } party || onTogether)
        {
            PartyBar.Visibility = Visibility.Collapsed;
            return;
        }
        var members = ctx.Known.PartyMembers ?? [];
        if (party.RoomEpisode is { } room && party.Playing)
        {
            PartyBarTitle.Text = T.PartyBarTitle(room.Title.Length > 0 ? room.Title : room.File, room.Episode);
            PartyBarLine.Text = T.PartyBarLine(members.Length, members.Count(m => m.Ready == true));
        }
        else
        {
            PartyBarTitle.Text = party.RoomEpisode is { } r ? T.PartyBarTitle(r.Title.Length > 0 ? r.Title : r.File, r.Episode) : T.TogetherTitle;
            // Nothing picked yet: Play anywhere is how the room gets its episode.
            PartyBarLine.Text = party.RoomEpisode is null ? T.PartyBarPick : T.PartyBarIdle;
        }
        PartyBar.Visibility = Visibility.Visible;
    }

    void OnPartyBarOpen(object sender, RoutedEventArgs e) => Go("together");

    void OnPartyBarLeave(object sender, RoutedEventArgs e) => ctx.Send(new Command.PartyLeave());

    /// <summary>Closing the window leaves the party: asked first, once per close.</summary>
    bool AskLeavingParty()
    {
        if (ctx.Known.Party is not { } party || askedParty)
            return false;
        askedParty = true;
        AskLeavingParty(party);
        return true;
    }

    async void AskLeavingParty(PartyView party)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = T.PartyLeaveTitle,
            Content = new TextBlock
            {
                Text = T.PartyLeaveDetail(party.Room),
                TextWrapping = TextWrapping.WrapWholeWords,
            },
            PrimaryButtonText = T.PartyLeaveAndClose,
            CloseButtonText = T.PartyStay,
            // Stay is the default, and the one amber button: Enter and Escape both keep the party.
            DefaultButton = ContentDialogButton.Close,
        };
        ContentDialogResult answer;
        try
        {
            if (CloseDialogs())
                await Task.Delay(250);
            answer = await Dialogs.Show(dialog, first: true);
        }
        catch (COMException)
        {
            askedParty = false;
            return;
        }
        if (answer != ContentDialogResult.Primary)
        {
            askedParty = false;
            return;
        }
        ctx.Send(new Command.PartyLeave());
        // Running downloads are still asked about, as a close without a party would.
        if (!HoldForDownloads())
            CloseNow();
    }

#if KAERI_TESTING
    /// <summary>
    /// The chat states the watch together chat board draws, over the playing party: Sol comes
    /// in, then the lines, reactions, typing or panel the state is for, all within about 3 s.
    /// </summary>
    async Task ChatScene(string state, FfiFakeParty party)
    {
        // The joins said, so the shot shows its own lines.
        await Task.Delay(3000);
        party.AddMember(FixtureRoom, "Sol");
        await Task.Delay(600);
        (string Who, string Text)[] lines =
        [
            ("Mika", "ok starting"),
            ("Ren", "the handoff in the rain, I knew it"),
            ("Sol", "wait, who is the new runner"),
        ];
        async Task Say(IEnumerable<(string Who, string Text)> said, int gap)
        {
            foreach (var (who, text) in said)
            {
                party.MemberSays(who, text);
                await Task.Delay(gap);
            }
        }
        switch (state)
        {
            case "party-chat-fold":
                await Say(lines.Concat([("Mika", "this part again"), ("Ren", "ready"), ("Sol", "one sec")]), 150);
                break;
            case "party-chat-moment":
                party.MemberSays("Mika", "ok starting");
                await Task.Delay(300);
                party.MemberReacts("Ren", Reaction.Gasp, 1.5);
                party.MemberReacts("Mika", Reaction.Laugh, 3.0);
                party.MemberReacts("Sol", Reaction.Laugh, 3.1);
                await Task.Delay(400);
                Player.ShotTickHover(3.0);
                // A moment holds 1.5 s: the room keeps reacting, so a shot finds some up.
                for (var round = 0; round < 12; round++)
                {
                    await Task.Delay(700);
                    if (round % 2 == 0)
                        party.MemberReacts("Ren", Reaction.Gasp, 1.5);
                    else
                        party.MemberReacts("Sol", round % 3 == 0 ? Reaction.Clap : Reaction.Laugh, 3.1);
                }
                break;
            case "party-chat-off" or "party-chat-hidden":
                party.MemberSays("Mika", "ok starting");
                await Task.Delay(400);
                Player.ShowParty();
                break;
            case "party-chat-field":
                party.MemberSays("Mika", lines[0].Text);
                await Task.Delay(300);
                Player.ShotChatType("the handoff in the rain");
                break;
            case "party-chat-count":
                await Task.Delay(300);
                Player.ShotChatType("the handoff in the rain, I knew it");
                break;
            case "party-chat-failed":
                await Task.Delay(300);
                Player.ShotChatType("anyone there");
                Player.ShotChatSend();
                await Task.Delay(6000);
                Player.ShowParty();
                break;
            case "party-chat-popover" or "party-chat-muted":
                await Say(lines, 200);
                party.MemberReacts("Ren", Reaction.Laugh, 3.0);
                if (state == "party-chat-muted")
                    ctx.Send(new Command.Party(new PartyCommand.Mute("Sol", true)));
                await Task.Delay(400);
                Player.ShowParty();
                break;
            default:
                await Say(lines, 300);
                break;
        }
    }

    FfiFakeParty? fakeParty;
    const string FixtureRoom = "quiet-harbor-k7m2xq9ad4tnwe3p";

    /// <summary>
    /// --show=party-*: a party on the fake server beside the engine, with members the run plays,
    /// in the state the screenshot is for.
    /// </summary>
    async void RehearseParty(string state)
    {
        await Task.Delay(1500);
        switch (state)
        {
            case "party-nosource" or "party-choose":
            {
                // An episode the sources were asked for and didn't have, in the room.
                fakeParty = ctx.Session.Engine.StartFakeParty();
                ctx.Send(new Command.PartyJoin(fakeParty.Server(), FixtureRoom, "Ada"));
                for (var i = 0; i < 50 && ctx.Known.Party is null; i++)
                    await Task.Delay(200);
                ShowPartySource(900001, "7", NoSourceWhy.NotFound, "could not match this title: sample-source has no episode 7");
                if (state == "party-choose")
                {
                    for (var i = 0; i < 50 && PartySourceDialog.Showing is null; i++)
                        await Task.Delay(200);
                    await Task.Delay(800);
                    PartySourceDialog.Showing?.RehearseChoose();
                }
                return;
            }
            case "party-form":
                ShowParty(new PartyDialog.For.Episode(900001, "7"));
                return;
            case "party-joining":
                // An address nothing answers on: the dialog waits, then says why.
                ShowParty(new PartyDialog.For.Episode(900001, "7"), d => d.Rehearse("10.255.255.1:8999", start: true));
                return;
            case "party-refused":
                ShowParty(new PartyDialog.For.Episode(900001, "7"), d => d.Rehearse("127.0.0.1:9", start: true));
                return;
        }
        fakeParty = ctx.Session.Engine.StartFakeParty();
        var server = fakeParty.Server();
        if (state is "party-invite" or "party-nosource-unaired")
        {
            var (id, episode) = state == "party-invite" ? (900001u, "7") : (900002u, "6");
            OpenLink(KaeriFfiMethods.PartyInviteLink(new PartyInvite(Server: server, Room: FixtureRoom, Id: id, Episode: episode)));
            if (state != "party-invite")
            {
                await Task.Delay(2500);
                PartyDialog.Showing?.Rehearse(null, start: true);
            }
            return;
        }
        if (state is "party-follow" or "party-follow-ready" or "party-follow-plain" or "party-reopen")
        {
            // Somebody else's room: Mika picks the episode and plays it while this device is in the lobby.
            fakeParty.AddMember(FixtureRoom, "Mika");
            ctx.Send(new Command.PartyJoin(server, FixtureRoom, "Ada"));
            for (var i = 0; i < 50 && ctx.Known.Party is null; i++)
                await Task.Delay(200);
            Go("together");
            await Task.Delay(1500);
            // Another Kaeri tags its file with the title and episode; a plain Syncplay client has a file name.
            var picked = state == "party-follow-plain" ? "Stardust Wayfarer - 07 (1080p).mkv" : "Stardust Wayfarer, episode 7 [anilist:900001:7]";
            fakeParty.MemberSetsPlaylist("Mika", [picked]);
            fakeParty.MemberSetsIndex("Mika", 0);
            fakeParty.MemberOpens("Mika", picked, 1400);
            fakeParty.MemberReady("Mika", true);
            await Task.Delay(2500);
            if (state is "party-follow-ready" or "party-reopen")
                ctx.Send(new Command.PartyReady(true));
            fakeParty.MemberPlays("Mika");
            if (state == "party-reopen")
            {
                // The room plays on while this device closes its player and goes back to the lobby.
                for (var i = 0; i < 50 && !Player.Showing; i++)
                    await Task.Delay(200);
                await Task.Delay(1500);
                Player.Close();
                Go("together");
            }
            return;
        }
        if (state is "party-lobby" or "party-lobby-ready" or "party-bar" or "party-chat-lobby")
        {
            // A party this device started for episode 7, with two friends in the lobby.
            ctx.Send(new Command.Party(new PartyCommand.Lobby(server, FixtureRoom, "Ada", new PartyEpisode(900001, "7"))));
            for (var i = 0; i < 50 && ctx.Known.Party is null; i++)
                await Task.Delay(200);
            await Task.Delay(1000);
            fakeParty.AddMember(FixtureRoom, "Mika");
            fakeParty.MemberOpens("Mika", "Stardust Wayfarer · Episode 7", 1400);
            fakeParty.MemberReady("Mika", true);
            fakeParty.AddMember(FixtureRoom, "Ren");
            fakeParty.MemberOpens("Ren", "Stardust Wayfarer · Episode 7", 1400);
            fakeParty.MemberReady("Ren", false);
            if (state == "party-chat-lobby")
            {
                ChatSetting.Set(false);
                fakeParty.MemberSays("Mika", "snacks ready?");
                fakeParty.MemberSays("Ren", "two minutes, making tea");
                fakeParty.MemberSays("Mika", "the soundtrack is at music.example/ost");
            }
            await Task.Delay(1200);
            if (state == "party-lobby-ready")
            {
                fakeParty.MemberReady("Ren", true);
                ctx.Send(new Command.PartyReady(true));
                await Task.Delay(600);
            }
            Go(state == "party-bar" ? "home" : "together");
            if (state == "party-chat-lobby")
            {
                // The conversation sits under the room: scrolled down to it.
                await Task.Delay(1200);
                (ContentFrame.Content as TogetherPage)?.ShotScrollToEnd();
            }
            return;
        }
        Player.HoldUpNext = state != "party-next";
        // The server as the chat shot needs it, before the join reads its features.
        if (state.StartsWith("party-chat", StringComparison.Ordinal))
            ChatSetting.Set(state == "party-chat-hidden");
        switch (state)
        {
            case "party-chat-count":
                fakeParty.SetChat(true, 40);
                break;
            case "party-chat-off":
                fakeParty.SetChat(false, null);
                break;
            case "party-chat-failed":
                fakeParty.SwallowChat(true);
                break;
        }
        ctx.Send(new Command.PartyJoin(server, FixtureRoom, "Ada"));
        ctx.Play(900001, "7", fromStart: true);
        // Until this device's member has the episode open, for the others to open the same.
        PartyMember? you = null;
        for (var i = 0; i < 100 && you?.File is null; i++)
        {
            await Task.Delay(200);
            you = ctx.Known.PartyMembers?.FirstOrDefault(m => m.You);
        }
        for (var i = 0; i < 50 && you?.Duration is null; i++)
        {
            await Task.Delay(200);
            you = ctx.Known.PartyMembers?.FirstOrDefault(m => m.You);
        }
        var file = you?.File ?? "Stardust Wayfarer · Episode 7";
        var length = you?.Duration ?? 30;
        // Held at the start, as a room waiting to be ready is, and clear of Up next.
        ctx.Send(new Command.Player(new PlayerCommand.TogglePause()));
        fakeParty.AddMember(FixtureRoom, "Mika");
        fakeParty.MemberOpens("Mika", file, length);
        fakeParty.AddMember(FixtureRoom, "Ren");
        fakeParty.MemberOpens("Ren", state == "party-panel" ? "Stardust Wayfarer · Episode 8" : file, length);
        fakeParty.MemberReady("Mika", true);
        fakeParty.MemberReady("Ren", false);
        await Task.Delay(800);
        switch (state)
        {
            case "party-panel":
                Player.ShowParty();
                break;
            case "party-ready":
                fakeParty.MemberReady("Ren", true);
                await Task.Delay(400);
                ctx.Send(new Command.PartyReady(true));
                await Task.Delay(600);
                Player.ShowParty();
                break;
            case "party-notice":
                // Mika pauses once the room is playing: everyone's player says so in its top corner.
                fakeParty.MemberReady("Ren", true);
                ctx.Send(new Command.PartyReady(true));
                await Task.Delay(5500);
                fakeParty.MemberPauses("Mika");
                break;
            case "party-next":
                // Near the end, playing, with the room's Up Next countdown on the card.
                fakeParty.MemberReady("Ren", true);
                ctx.Send(new Command.PartyReady(true));
                await Task.Delay(1500);
                ctx.Send(new Command.Player(new PlayerCommand.SeekTo(Math.Max(0, length - 12))));
                await Task.Delay(500);
                fakeParty.MemberPlays("Mika");
                break;
            case "party-chat":
                fakeParty.MemberSays("Mika", "Starting in a minute, grabbing tea");
                break;
            case { } chat when chat.StartsWith("party-chat-", StringComparison.Ordinal):
                await ChatScene(chat, fakeParty);
                break;
            case "party-left":
                Player.ShowParty();
                await Task.Delay(1500);
                ctx.Send(new Command.PartyLeave());
                break;
            case "party-lost":
                fakeParty.GoDown(true);
                await Task.Delay(2500);
                Player.ShowParty();
                break;
            case "party-close":
                // What the title bar's close does in a party (a window's Close() skips it).
                AskLeavingParty();
                break;
        }
    }
#endif
}
