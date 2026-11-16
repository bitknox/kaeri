// The cast session on Windows: the mini and the controller drawn from the engine's view without
// amplifying it, and the title bar's button from the session and the TVs found.

using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class CastPainterTests
{
    DateTimeOffset now = new(2026, 11, 15, 20, 0, 0, TimeSpan.Zero);

    ulong rev;

    static FfiCastItem Item(double position = 100, double? duration = 1440, bool paused = false, bool waiting = false, bool loading = false) =>
        new(Id: 900_001, Title: "Made-up Show", Episode: "7", EpisodeTitle: "A Quiet Harbour", Image: null,
            Position: position, Duration: duration, Paused: paused, Waiting: waiting ? 0u : null, Loading: loading);

    /// <summary>A view as the engine sends it: a higher rev each time unless one is given.</summary>
    FfiCastSessionView View(FfiCastItem? item, FfiCastSessionState state = FfiCastSessionState.Playing, Msg? notice = null, ulong? sameRev = null, bool reconnecting = false) =>
        new(State: state, Button: state switch
            {
                FfiCastSessionState.Connecting => FfiCastButton.Connecting,
                FfiCastSessionState.Connected or FfiCastSessionState.Playing => FfiCastButton.Connected,
                _ => FfiCastButton.Hidden,
            },
            Rev: sameRev ?? ++rev, DeviceKey: "cast:living", DeviceName: "Living room", Item: item, Queue: [], Notice: notice,
            Note: null, Level: null, Muted: null, VolumeControl: FfiVolumeControl.Level, OwnReceiver: false, Reconnecting: reconnecting);

    CastPainter Painter() => new(() => now);

    [Fact]
    public void EchoesChangeOnlyWhatTheyChanged()
    {
        var painter = Painter();
        painter.Take(View(Item()));
        var playing = painter.Draw("Living room")!;
        Assert.Equal(CastLine.Playing, playing.Line);
        Assert.False(playing.Dim);
        Assert.Equal("Episode 7 · A Quiet Harbour", playing.EpisodeLine);
        Assert.Equal("1:40", playing.Elapsed);
        Assert.Equal("-22:20", playing.Remaining);
        Assert.Null(playing.Waiting);

        // Paused on the TV: the glyph and the line, nothing else.
        painter.Take(View(Item(paused: true)));
        var paused = painter.Draw("Living room")!;
        Assert.True(paused.Paused);
        Assert.Equal(CastLine.Paused, paused.Line);
        Assert.Equal("Paused on Living room", paused.Status);
        Assert.Equal(playing with { Line = CastLine.Paused, Status = paused.Status, Paused = true }, paused);

        // Playing again, then a status without the media's length: the times stay.
        painter.Take(View(Item()));
        painter.Take(View(Item(duration: null)));
        Assert.Equal(playing, painter.Draw("Living room"));

        // The TV waits for the stream: nothing dims, the playhead holds, and the line comes after 4 s.
        painter.Take(View(Item(waiting: true)));
        Assert.Equal(playing, painter.Draw("Living room"));
        now += TimeSpan.FromSeconds(5);
        var waiting = painter.Draw("Living room")!;
        Assert.Equal(playing with { Waiting = waiting.Waiting }, waiting);
        Assert.Equal("Waiting for Living room · 5 s", waiting.Waiting);
    }

    [Fact]
    public void OnlyLoadingDims()
    {
        var painter = Painter();
        painter.Take(View(Item(position: 0, duration: null, loading: true)));
        var starting = painter.Draw("Living room")!;
        Assert.True(starting.Dim);
        Assert.Equal(CastLine.Starting, starting.Line);
        Assert.Equal("Starting on Living room", starting.Status);
        // The episode line is the episode's from the start, never swapped for a starting line.
        Assert.Equal("Episode 7 · A Quiet Harbour", starting.EpisodeLine);
        painter.Take(View(Item(position: 0)));
        Assert.False(painter.Draw("Living room")!.Dim);
    }

    [Fact]
    public void ThePlayheadRunsOnWhilePlayingAndHoldsWhilePaused()
    {
        var painter = Painter();
        painter.Take(View(Item()));
        now += TimeSpan.FromSeconds(3);
        Assert.Equal(103, painter.Playhead());
        // The same view again, as the watch may send for another change: the playhead keeps going.
        painter.Take(View(Item()));
        now += TimeSpan.FromSeconds(1);
        Assert.Equal(104, painter.Playhead());
        painter.Take(View(Item(position: 104, paused: true)));
        now += TimeSpan.FromSeconds(10);
        Assert.Equal(104, painter.Playhead());
    }

    [Fact]
    public void ASeekAndAPressHoldUntilANewerView()
    {
        var painter = Painter();
        painter.Take(View(Item()));
        painter.HoldSeek(600);
        painter.HoldPause(true);
        Assert.Equal(600, painter.Playhead());
        Assert.True(painter.Draw("Living room")!.Paused);
        Assert.Equal("10:00", painter.Draw("Living room")!.Elapsed);
        // An echo of the state before the seek, at the same rev: the seek and the press stay.
        painter.Take(View(Item(), sameRev: painter.Rev));
        Assert.Equal(600, painter.Playhead());
        Assert.True(painter.Paused);
        painter.Take(View(Item(position: 598, paused: true)));
        Assert.Equal(598, painter.Playhead());
        Assert.True(painter.Paused);
    }

    [Fact]
    public void ACastThatFailedSaysWhy()
    {
        var painter = Painter();
        painter.Take(View(Item()));
        painter.Take(View(Item(), FfiCastSessionState.Error, new Msg.NoticeCastRefused("Living room")));
        var failed = painter.Draw("Living room")!;
        Assert.Equal(CastLine.Failed, failed.Line);
        Assert.Equal("Couldn't start on Living room", failed.Status);
        Assert.False(string.IsNullOrEmpty(failed.Reason));
    }

    [Fact]
    public void NothingLoadedDrawsNothingUnlessJoiningAgain()
    {
        var painter = Painter();
        painter.Take(View(null, FfiCastSessionState.Connected));
        Assert.Null(painter.Draw("Living room"));
        painter.Take(View(null, FfiCastSessionState.Connecting));
        var joining = painter.Draw("Living room", rejoining: true)!;
        Assert.Equal(CastLine.Rejoining, joining.Line);
        Assert.Equal("Reconnecting to Living room", joining.Status);
        Assert.True(joining.Dim);
    }

    [Fact]
    public void TheButtonFollowsTheSessionAndTheTvsFound()
    {
        Assert.Equal(CastButtonState.Hidden, CastPainter.Button(View(null, FfiCastSessionState.Idle), tvsFound: false, rejoining: false, driving: false));
        Assert.Equal(CastButtonState.Available, CastPainter.Button(View(null, FfiCastSessionState.Idle), tvsFound: true, rejoining: false, driving: false));
        Assert.Equal(CastButtonState.Connecting, CastPainter.Button(View(null, FfiCastSessionState.Connecting), tvsFound: true, rejoining: false, driving: false));
        Assert.Equal(CastButtonState.Connecting, CastPainter.Button(View(null, FfiCastSessionState.Idle), tvsFound: false, rejoining: true, driving: false));
        Assert.Equal(CastButtonState.Connected, CastPainter.Button(View(null, FfiCastSessionState.Connected), tvsFound: true, rejoining: false, driving: false));
        Assert.Equal(CastButtonState.Connected, CastPainter.Button(View(Item()), tvsFound: true, rejoining: false, driving: false));
        Assert.Equal(CastButtonState.Connected, CastPainter.Button(View(null, FfiCastSessionState.Idle), tvsFound: true, rejoining: false, driving: true));
    }

    [Fact]
    public void AReconnectHoldsTheClockAndRejoinsInPlace()
    {
        var painter = Painter();
        painter.Take(View(Item()));
        now += TimeSpan.FromSeconds(4);
        // The connection closes: the clock holds where it stood, the line says so, nothing dims as a start does.
        painter.Take(View(Item(), reconnecting: true));
        now += TimeSpan.FromSeconds(10);
        var reconnecting = painter.Draw("Living room")!;
        Assert.Equal(CastLine.Reconnecting, reconnecting.Line);
        Assert.Equal("Reconnecting to Living room", reconnecting.Status);
        Assert.False(reconnecting.Dim);
        Assert.Equal(104, reconnecting.Position);
        Assert.Null(reconnecting.Waiting);
        // Joined again: the TV's own playhead, playing on.
        painter.Take(View(Item(position: 115)));
        var back = painter.Draw("Living room")!;
        Assert.Equal(CastLine.Playing, back.Line);
        Assert.Equal(115, back.Position);
    }

    [Fact]
    public void AnEndFromOutsideIsCastingStoppedWithTheEpisodeKept()
    {
        foreach (Msg why in new Msg[] { new Msg.NoticeCastLost("Living room"), new Msg.NoticeCastStoppedElsewhere("Living room"), new Msg.NoticeCastTakenOver("Living room") })
        {
            var painter = Painter();
            painter.Take(View(Item()));
            now += TimeSpan.FromSeconds(2);
            painter.Take(View(Item(), reconnecting: true));
            now += TimeSpan.FromSeconds(20);
            painter.Take(View(null, FfiCastSessionState.Error, why));
            var stopped = painter.Draw("Living room")!;
            Assert.Equal(CastLine.Stopped, stopped.Line);
            Assert.Equal("Casting stopped", stopped.Status);
            Assert.Equal(why.Text(), stopped.Reason);
            Assert.Equal("Made-up Show", stopped.Title);
            Assert.Equal(102, stopped.Position);
            Assert.Equal(1440, stopped.Duration);
        }
    }

    [Fact]
    public void ATvHeldWithNothingOnItDrawsOnlyForTheOpenController()
    {
        var painter = Painter();
        painter.Take(View(null, FfiCastSessionState.Connected));
        Assert.Null(painter.Draw("Living room"));
        var idle = painter.Draw("Living room", idle: true)!;
        Assert.Equal(CastLine.Idle, idle.Line);
        Assert.Equal("Connected to Living room", idle.Status);
        Assert.Null(idle.Id);
        Assert.Null(idle.Duration);
    }

    [Fact]
    public void TheFooterKeepsItsPlacesAndOnlyStopCastingTakesInputWhileReconnecting()
    {
        var playing = CastFooter.For(CastLine.Playing);
        Assert.Equal((CastAction.PlayHere, CastAction.StopCasting), (playing.First, playing.Second));
        Assert.False(playing.FirstAccent);
        Assert.Equal(new[] { CastAction.ChooseAnother, CastAction.StopCasting }, playing.More);

        var reconnecting = CastFooter.For(CastLine.Reconnecting);
        Assert.Equal((CastAction.PlayHere, CastAction.StopCasting), (reconnecting.First, reconnecting.Second));
        Assert.False(reconnecting.FirstLive);
        Assert.False(reconnecting.MoreLive);

        var stopped = CastFooter.For(CastLine.Stopped);
        Assert.Equal((CastAction.PlayHere, CastAction.ChooseAnother), (stopped.First, stopped.Second));
        Assert.True(stopped.FirstAccent);
        Assert.Equal(new[] { CastAction.ChooseAnother }, stopped.More);

        var idle = CastFooter.For(CastLine.Idle);
        Assert.Equal((CastAction.ChooseAnother, CastAction.Disconnect), (idle.First, idle.Second));
        Assert.DoesNotContain(CastAction.StopCasting, idle.More);
        Assert.Equal("Disconnect", CastFooter.Words(CastAction.Disconnect));
    }
}
