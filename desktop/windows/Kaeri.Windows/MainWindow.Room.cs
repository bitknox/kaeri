using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows;

/// <summary>
/// The room, the window's side: the engine's room events into the player's column, strip and
/// panels, and the player's presses back to the engine as room commands. The room lives in the
/// player; the Watch together place shows it when the player is closed.
/// </summary>
public sealed partial class MainWindow
{
    bool roomWired;
    RoomPanel? roomPanelShown;
    string? roomSourcesFor;

    /// <summary>An invite looked into and not answered yet: Join sends it.</summary>
    internal string? RoomInvite { get; set; }

    void WireRoom()
    {
        if (roomWired)
            return;
        roomWired = true;
        Player.RoomSent += command => ctx.Send(new Command.Room(command));
        Player.RoomAnswered += (join, anyway) =>
        {
            if (join && RoomInvite is { } invite)
                ctx.Send(new Command.Room(new RoomCommand.Lobby(Invite: invite, Anyway: anyway)));
            else
            {
                RoomInvite = null;
                if (ctx.Known.Room is null)
                    Player.EndRoom();
            }
        };
        Player.RoomSourcePicked += pick =>
        {
            if (ctx.Known.Room?.RoomEpisode is { } ep)
                ctx.Send(new Command.PlaySource(ep.Id, ep.Episode, pick.ProviderId, pick.Id));
        };
        Player.RoomGo += Go;
    }

    /// <summary>Starts a room from the episode playing: the player becomes the room.</summary>
    internal void StartRoom(uint id, string episode) =>
        ctx.Send(new Command.Room(new RoomCommand.Lobby(Pick: new RoomEpisode(id, episode))));


    bool askedRoom;

    /// <summary>The bar above the pages while in a room with the player closed: what it watches, a way back and out.</summary>
    internal void UpdateRoomBar()
    {
        if (ctx.Known.Room is not { } room || Player.Showing || ContentFrame.Content is Pages.TogetherPage)
        {
            RoomBar.Visibility = Visibility.Collapsed;
            return;
        }
        var ep = room.RoomEpisode;
        RoomBarTitle.Text = ep?.Title ?? RoomWords.Title;
        RoomBarLine.Text = ep is null ? "" : T.RoomHeaderLine(RoomWords.N(ep.Episode), ctx.Known.RoomMembers.Count(m => m.State != MemberState.Left));
        RoomBar.Visibility = Visibility.Visible;
    }

    void OnRoomBarOpen(object sender, RoutedEventArgs e) => ctx.Send(new Command.Room(new RoomCommand.OpenPlayer()));

    async void OnRoomBarLeave(object sender, RoutedEventArgs e)
    {
        if (await Kaeri.Windows.Player.RoomPopups.AskLeave(Root.XamlRoot))
            ctx.Send(new Command.Room(new RoomCommand.Leave()));
    }

    /// <summary>Closing the window leaves the room: asked first, once per close.</summary>
    bool AskLeavingRoom()
    {
        if (ctx.Known.Room is null || askedRoom)
            return false;
        askedRoom = true;
        AskLeavingRoomNow();
        return true;
    }

    async void AskLeavingRoomNow()
    {
        bool leave;
        try
        {
            if (CloseDialogs())
                await Task.Delay(250);
            leave = await Kaeri.Windows.Player.RoomPopups.AskLeave(Root.XamlRoot);
        }
        catch (COMException)
        {
            leave = false;
        }
        if (!leave)
        {
            askedRoom = false;
            return;
        }
        ctx.Send(new Command.Room(new RoomCommand.Leave()));
        // Running downloads are still asked about, as a close outside a room would.
        if (!HoldForDownloads())
            CloseNow();
    }

    /// <summary>An invite link: what the room watches and who is there, with Join, in the place (or over the player).</summary>
    void OpenRoomInvite(string text)
    {
        if (KaeriFfiMethods.ParseRoomTarget(text) is not { } target)
        {
            ShowToast(InfoBarSeverity.Warning, T.RoomErrorNobodyAnswered);
            return;
        }
        if (target.Old is not null)
        {
            ShowToast(InfoBarSeverity.Warning, T.RoomErrorOldInvite);
            return;
        }
        RoomInvite = target.Link;
        ctx.Send(new Command.Room(new RoomCommand.PeekInvite(target.Link)));
        if (!Player.Showing)
            Go("together");
    }
    void RoomHeard(IReadOnlyList<Event> batch)
    {
        WireRoom();
        var touched = false;
        var joined = false;
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.RoomJoined:
                    joined = true;
                    RoomInvite = null;
                    touched = true;
                    break;
                case Event.RoomLeft left:
                    roomPanelShown = null;
                    roomSourcesFor = null;
                    Player.EndRoom();
                    // Said over the video while the player shows, where the window's toasts can't be seen.
                    if (left.Why is not RoomLeftWhy.Left and not RoomLeftWhy.Moved)
                    {
                        if (Player.Showing)
                            Player.Say(RoomWords.Left(left.Why));
                        else
                            ShowToast(InfoBarSeverity.Warning, RoomWords.Left(left.Why));
                    }
                    break;
                case Event.Sources s when roomSourcesFor == $"{s.Id}/{s.Episode}":
                    Player.ShowRoomSources(s.Candidates);
                    break;
                case Event.RoomState or Event.RoomMembers or Event.RoomChat or Event.RoomGlimpse or Event.RoomNoSource
                    or Event.RoomMoveOffer or Event.RoomMoveOfferGone:
                    touched = true;
                    break;
            }
        }
        UpdateRoomBar();
        if (touched)
            ShowRoomNow(morph: joined);
    }

    /// <summary>The room as the engine last said it, into the player, when the player is up.</summary>
    internal void ShowRoomNow(bool morph = false)
    {
#if KAERI_TESTING
        if (holdRoom)
            return;
#endif
        var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var known = ctx.Known;
        RoomScene? scene = known.Room is { } view ? new RoomScene(view, known.RoomMembers, now, Offer: known.RoomMoveOffer)
            : known.RoomGlimpse is { } glimpse && RoomInvite is not null ? Rooms.Invited(glimpse, now)
            : null;
        if (scene is null || !Player.Showing)
        {
            if (scene is null)
                Player.EndRoom();
            return;
        }
        Player.ShowRoom(scene, known.Room is null ? null : known.RoomChat, morph && Motion.Enabled);
        var kind = Rooms.PanelFor(scene);
        var ep = scene.View.RoomEpisode;
        // The episode's name from the room, else from this device's own list.
        var name = ep?.Name ?? (ep is null ? null : known.Episodes.GetValueOrDefault(ep.Id)?.FirstOrDefault(r => r.Number == ep.Episode)?.Title);
        var panel = kind == RoomPanelKind.None ? null : new RoomPanel(kind, ep?.Title ?? "", scene.Episode, name,
            SourceCount: known.Providers.Count(p => !p.IsDemo),
            Pressed: (scene.View.Waiting ?? []).Length > 0 || scene.View.StartsAtMs is not null);
        if (panel == roomPanelShown)
            return;
        roomPanelShown = panel;
        Player.ShowRoomPanel(panel);
        // Choosing lists what each source has for the room's episode, in place.
        var wants = kind == RoomPanelKind.Choosing && ep is not null ? $"{ep.Id}/{ep.Episode}" : null;
        if (wants is not null && wants != roomSourcesFor)
        {
            Player.ShowRoomSources(null);
            ctx.Send(new Command.LoadSources(ep!.Id, ep.Episode));
        }
        roomSourcesFor = wants;
    }
}

#if KAERI_TESTING
/// <summary>
/// --show=room-*: the room's board frames over the fixture episode, from the engine's fake room
/// and its scenes (<see cref="FfiRoomScene"/>). The player, the column, the strip and the popups
/// are the app's own; menus and dialogs open over Watching.
/// </summary>
public sealed partial class MainWindow
{
    FfiFakeRoom? fakeRoom;

    /// <summary>A shot's: the room joined but not shown yet, for the morph to start on the second.</summary>
    bool holdRoom;

    /// <summary>The board's frame names, by the engine's scenes.</summary>
    static FfiRoomScene SceneFor(string state) => state switch
    {
        "room-before" => FfiRoomScene.BeforePlay,
        "room-before-wait" => FfiRoomScene.BeforePlayWait,
        "room-before-cap" => FfiRoomScene.BeforePlayCap,
        "room-before-count" => FfiRoomScene.Starting,
        "room-invited" => FfiRoomScene.Invited,
        "room-invited-over" => FfiRoomScene.InvitedFull,
        "room-paused" => FfiRoomScene.Paused,
        "room-wait" or "room-full-wait" => FfiRoomScene.WaitLoading,
        "room-cap" or "room-full-cap" => FfiRoomScene.WaitCap,
        "room-carried" => FfiRoomScene.CarriedOn,
        "room-self-wait" => FfiRoomScene.YouLoading,
        "room-buffering" => FfiRoomScene.Buffering,
        "room-buffering-six" => FfiRoomScene.BufferingSix,
        "room-choosing" => FfiRoomScene.Choosing,
        "room-nosource" => FfiRoomScene.NoSource,
        "room-closed" => FfiRoomScene.PlayerClosed,
        "room-reconnecting" => FfiRoomScene.Reconnecting,
        "room-behind" => FfiRoomScene.Behind,
        "room-other" => FfiRoomScene.OtherEpisode,
        "room-left" => FfiRoomScene.Left,
        "room-own-choosing" => FfiRoomScene.YouChoosing,
        "room-own-nosource" => FfiRoomScene.YouNoSource,
        "room-self-behind" => FfiRoomScene.YouBehind,
        "room-self-reconnecting" => FfiRoomScene.YouReconnecting,
        "room-six" => FfiRoomScene.Six,
        "room-relay-trouble" or "room-details-trouble" => FfiRoomScene.RelayTrouble,
        "room-upnext" => FfiRoomScene.UpNext,
        "room-upnext-waiting" => FfiRoomScene.UpNextWaiting,
        "room-skip" => FfiRoomScene.SkippedOpening,
        "room-chat" => FfiRoomScene.Chat,
        "room-move-offer" => FfiRoomScene.MoveOffer,
        "room-move-after" => FfiRoomScene.MovedAway,
        "room-unreachable" => FfiRoomScene.Unreachable,
        _ => FfiRoomScene.Watching,
    };

    /// <summary>Waits until the process has run this long, so a run's shots land on a moment.</summary>
    static async Task AtUptime(double seconds)
    {
        var started = System.Diagnostics.Process.GetCurrentProcess().StartTime;
        var wait = started.AddSeconds(seconds) - DateTime.Now;
        if (wait > TimeSpan.Zero)
            await Task.Delay(wait);
    }

    /// <summary>--show=room-*: the fixture episode playing, then the engine's fake room in the frame's state.</summary>
    async void RehearseRoom(string state)
    {
        // A -rm suffix is the same frame with Reduce Motion on.
        if (state.EndsWith("-rm", StringComparison.Ordinal))
        {
            Motion.Override = false;
            state = state[..^3];
        }
        WireRoom();
        // Settings' frames are the page's own (--page=settings --pane=playback).
        if (state.StartsWith("room-settings", StringComparison.Ordinal))
            return;
        if (state.StartsWith("room-place", StringComparison.Ordinal))
        {
            await RehearsePlace(state);
            return;
        }
        await Task.Delay(1500);
        ctx.Play(900001, "9", fromStart: true);
        for (var i = 0; i < 100 && !Player.Playing; i++)
            await Task.Delay(200);
        await Task.Delay(600);
        // The morph is staged held back, then shown at 14 s, so the frames land on it.
        holdRoom = state == "room-morph";
        var scene = SceneFor(state);
        var fake = fakeRoom = ctx.Session.Engine.StartFakeRoom();
        if (scene is FfiRoomScene.Invited or FfiRoomScene.InvitedFull)
            RoomInvite = fake.Invite("Mika");
        // The fake blocks while it stages: off the window's thread.
        var staged = await Task.Run(() => fake.Stage(state == "room-wait-story" ? FfiRoomScene.Watching : scene));
        if (!staged)
            System.Diagnostics.Debug.WriteLine("room fixture: the device never got into the room");
        await Task.Delay(800);
        if (state is "room-morph" or "room-wait-story")
            await AtUptime(14);
        if (holdRoom)
        {
            holdRoom = false;
            ShowRoomNow(morph: true);
        }
        switch (state)
        {
            case "room-full-wait" or "room-full-cap":
                Fullscreen(true);
                await Task.Delay(900);
                Player.ShotHideControls();
                break;
            case "room-to-full":
                await AtUptime(14);
                Fullscreen(true);
                await AtUptime(18);
                Fullscreen(false);
                break;
            case "room-invite":
                Player.ShotRoomInvite(false);
                break;
            case "room-move-start":
                Player.ShotRoomInvite(true);
                break;
            case "room-move-pick":
                Player.ShotRoomMove();
                break;
            case "room-details" or "room-details-trouble":
                Player.ShotRoomDetails(false);
                break;
            case "room-details-edit":
                Player.ShotRoomDetails(true);
                break;
            case "room-leave":
                Player.ShotRoomLeave();
                break;
            case "room-close":
                Player.ShotRoomClose();
                break;
            case "room-own-choosing" or "room-own-nosource" or "room-before" or "room-before-wait" or "room-before-cap"
                or "room-before-count" or "room-invited" or "room-invited-over" or "room-self-wait":
                Player.ShotHideControls();
                break;
            case "room-wait-story":
                await Task.Run(() => RoomWaitStory(fake));
                break;
        }
    }


    /// <summary>
    /// The place's frames (--page=together): <c>room-place</c> with a recent room,
    /// <c>room-place-room</c> in a room with the player closed (Keep the room), and
    /// <c>room-place-fail</c> a pasted link none of whose relays answer.
    /// </summary>
    async Task RehearsePlace(string state)
    {
        await Task.Delay(1500);
        var fake = fakeRoom = ctx.Session.Engine.StartFakeRoom();
        switch (state)
        {
            case "room-place-fail":
                var link = fake.Invite("Mika");
                RoomInvite = link;
                (ContentFrame.Content as Pages.TogetherPage)?.ShotFill(link);
                await Task.Run(() => fake.Stage(FfiRoomScene.Unreachable));
                break;
            case "room-place-room":
                await Task.Run(() => fake.Stage(FfiRoomScene.Watching));
                await Task.Delay(1500);
                ctx.Send(new Command.Room(new RoomCommand.ClosePlayer()));
                Player.Close();
                Go("together");
                break;
            default:
                await Task.Run(() => fake.Stage(FfiRoomScene.Watching));
                await Task.Delay(1500);
                ctx.Send(new Command.Room(new RoomCommand.Leave()));
                Player.Close();
                await Task.Delay(500);
                Go("together");
                ctx.Send(new Command.Room(new RoomCommand.LoadRecent()));
                break;
        }
    }
    /// <summary>The board's wait story at 1×: Ren loads at 1 s, the cap at 20 s, Mika carries on at 23 s, Ren lands at 27 s.</summary>
    static async Task RoomWaitStory(FfiFakeRoom fake)
    {
        await Task.Delay(1000);
        fake.MemberSeeks("Mika", 35);
        fake.MemberLoads("Ren", 0);
        await Task.Delay(22_000);
        fake.MemberCarriesOn("Mika");
        await Task.Delay(4_000);
        fake.MemberReady("Ren");
    }
}
#endif
