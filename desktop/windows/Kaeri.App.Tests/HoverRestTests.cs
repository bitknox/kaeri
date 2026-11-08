// When a cover under the pointer cues its trailer and opens its card.

using Kaeri.App;
using Xunit;

namespace Kaeri.App.Tests;

public class HoverRestTests
{
    static HoverRest Entered(long at = 0)
    {
        var rest = new HoverRest();
        rest.Enter(10, 10, at, HoverRest.Dwell);
        return rest;
    }

    [Fact]
    public void AStillPointerCuesAt150AndOpensAt450()
    {
        var rest = Entered();
        Assert.Equal(HoverRestStep.None, rest.Step(100));
        Assert.Equal(HoverRestStep.Cue, rest.Step(150));
        Assert.Equal(HoverRestStep.None, rest.Step(300));
        Assert.Equal(HoverRestStep.Open, rest.Step(450));
        Assert.Equal(HoverRestStep.None, rest.Step(600));
    }

    [Fact]
    public void SlowDriftInsideTheCoverDoesNotRestartTheWait()
    {
        var rest = Entered();
        // 5 px every 50 ms: far past the old 4 px rest, but slow.
        for (long t = 50; t <= 450; t += 50)
            rest.Move(10 + t / 10.0, 10, t);
        Assert.Equal(HoverRestStep.Open, rest.Step(450));
    }

    [Fact]
    public void APointerStillTravellingWaitsUntilItSlows()
    {
        var rest = Entered();
        for (long t = 20; t <= 460; t += 20)
            rest.Move(10 + t, 10, t);
        Assert.Equal(HoverRestStep.None, rest.Step(460));
        // Stopped: a lone sample older than the window is at rest.
        Assert.Equal(HoverRestStep.Open, rest.Step(600));
    }

    [Fact]
    public void ACoverThatComesToRestUnderAStillPointerOpens()
    {
        var rest = Entered();
        rest.Scrolled(200);
        rest.Scrolled(260);
        Assert.Equal(HoverRestStep.None, rest.Step(400));
        Assert.Equal(HoverRestStep.Cue, rest.Step(410));
        Assert.Equal(HoverRestStep.Open, rest.Step(710));
    }

    [Fact]
    public void AScrollClosesAnOpenCardAndTheCoverUnderThePointerOpensAgain()
    {
        var rest = Entered();
        Assert.Equal(HoverRestStep.Cue, rest.Step(150));
        Assert.Equal(HoverRestStep.Open, rest.Step(450));
        rest.Scrolled(1000);
        Assert.Equal(HoverRestStep.Cue, rest.Step(1150));
        Assert.Equal(HoverRestStep.Open, rest.Step(1450));
    }

    [Fact]
    public void ACoverFoundUnderThePointerAfterAScrollWaitsFromTheScroll()
    {
        var rest = new HoverRest();
        rest.Enter(10, 10, 400, HoverRest.Dwell, start: 250);
        Assert.Equal(HoverRestStep.Cue, rest.Step(400));
        Assert.Equal(HoverRestStep.Open, rest.Step(700));
    }

    [Fact]
    public void AKeyPressHoldsEverythingUntilThePointerMoves()
    {
        var rest = Entered();
        rest.Hold();
        Assert.Equal(HoverRestStep.None, rest.Step(1000));
        rest.Move(10.5, 10, 1000);
        Assert.Equal(HoverRestStep.None, rest.Step(1000));
        rest.Move(12, 10, 1010);
        Assert.Equal(HoverRestStep.Open, rest.Step(1100));
    }

    [Fact]
    public void LeavingTheCoverStopsTheWait()
    {
        var rest = Entered();
        rest.Leave();
        Assert.False(rest.Waiting);
        Assert.Equal(HoverRestStep.None, rest.Step(1000));
    }

    [Fact]
    public void ScanningARowOpensTheNextCardSoonerWithNoSeparateCue()
    {
        Assert.Equal(HoverRest.NextDwell, HoverRest.DwellAfter(closedAt: 1000, now: 1500));
        Assert.Equal(HoverRest.Dwell, HoverRest.DwellAfter(closedAt: 1000, now: 2500));
        var rest = new HoverRest();
        rest.Enter(10, 10, 0, HoverRest.NextDwell);
        Assert.Equal(HoverRestStep.None, rest.Step(100));
        Assert.Equal(HoverRestStep.Open, rest.Step(150));
    }
}
