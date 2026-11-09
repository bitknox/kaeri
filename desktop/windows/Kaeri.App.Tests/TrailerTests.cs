// Trailers: the flow that decides when the video shows, the engine's page under the app's identity,
// and who may autoplay.

using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class TrailerTests
{
    static readonly Trailer Clip = new(TrailerHost.YouTube, "clip0001");

    static Entry Titled(uint id, Trailer? trailer = null) => new(Id: id, Title: "Lantern Road", Trailer: trailer);

    [Fact]
    public void TheVideoShowsOnlyOnceAFrameIsPlaying()
    {
        var flow = new TrailerFlow();
        Assert.True(flow.Start());
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.None, flow.Heard(new TrailerMessage.Ready()));
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.ShowVideo, flow.Heard(new TrailerMessage.Playing()));
        Assert.True(flow.Showing);
    }

    [Fact]
    public void AnEndOrAnErrorLetsGoForGoodOnThisVisit()
    {
        foreach (TrailerMessage over in new TrailerMessage[] { new TrailerMessage.Ended(), new TrailerMessage.Error(150) })
        {
            var flow = new TrailerFlow();
            flow.Start();
            flow.Heard(new TrailerMessage.Playing());
            var effect = flow.Heard(over);
            Assert.True(effect.HasFlag(TrailerEffect.Release) && effect.HasFlag(TrailerEffect.HideVideo));
            Assert.Equal(TrailerPhase.Done, flow.Phase);
            Assert.False(flow.Start());
        }
    }

    [Fact]
    public void AnErrorBeforeAnyFrameLeavesTheBannerAloneAndSaysNothing()
    {
        var flow = new TrailerFlow();
        flow.Start();
        var effect = flow.Heard(new TrailerMessage.Error(0));
        Assert.Equal(TrailerEffect.Release, effect);
        Assert.False(flow.CanStart);
    }

    [Fact]
    public void ScrollingAwayPausesAndTheVideoComesBackWhenItPlaysAgain()
    {
        var flow = new TrailerFlow();
        flow.Start();
        flow.Heard(new TrailerMessage.Playing());
        Assert.Equal(TrailerEffect.PausePlayer | TrailerEffect.HideVideo, flow.Pause());
        Assert.True(flow.Active);
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.PlayPlayer, flow.Resume());
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.ShowVideo, flow.Heard(new TrailerMessage.Playing()));
    }

    [Fact]
    public void GivingUpWhileLoadingMayTryAgainButStoppingAPlayingTrailerFadesTheVideoAway()
    {
        var flow = new TrailerFlow();
        flow.Start();
        Assert.Equal(TrailerEffect.Release, flow.GiveUp());
        Assert.True(flow.CanStart);
        flow.Start();
        flow.Heard(new TrailerMessage.Playing());
        Assert.Equal(TrailerEffect.None, flow.GiveUp());
        Assert.Equal(TrailerEffect.Release | TrailerEffect.HideVideo, flow.Stop());
        Assert.Equal(TrailerEffect.None, flow.Stop());
    }

    [Fact]
    public void TheEnginesPageIsLoadedUnderTheAppsOriginAndTheBannerOneIsMuted()
    {
        var entry = Titled(1, Clip);
        var banner = Trailers.Page(entry, EmbedStyle.Banner);
        Assert.NotNull(banner);
        Assert.Contains(Trailers.Origin, banner);
        Assert.Contains("clip0001", banner);
        Assert.Equal("https://app.kaeri.windows/", KaeriFfiMethods.TrailerBaseUrl(Trailers.Identity));
        Assert.Equal("app.kaeri.windows", Trailers.Host);
        Assert.Null(Trailers.Page(Titled(2), EmbedStyle.Banner));
        Assert.Null(Trailers.Page(Titled(3, new Trailer(TrailerHost.Dailymotion, "x1")), EmbedStyle.Banner));
    }

    [Fact]
    public void ThePageFollowsTheSubtitleLanguageAndCanSwitchTrailers()
    {
        var entry = Titled(1, Clip);
        Assert.Contains("\"de\"", Trailers.Page(entry, EmbedStyle.Banner, "ger"));
        Assert.NotNull(KaeriFfiMethods.TrailerLoadScript(Clip));
        Assert.Contains("youtube", KaeriFfiMethods.TrailerChromeScript());
        Assert.Equal("a bad video id", Trailers.Meaning(2));
    }

    [Fact]
    public void AKeptPageStartsWithNoVideoAndOnlyTheEnginesPagesAreSwitched()
    {
        var warm = Trailers.WarmPage("eng");
        Assert.NotNull(warm);
        Assert.Contains(Trailers.Origin, warm);
        Assert.Contains("videoId: \"\"", warm);
        Assert.Equal(KaeriFfiMethods.TrailerLoadScript(Clip), Trailers.Switch(Titled(1, Clip)));
        Assert.Null(Trailers.Switch(Titled(2)));
        // Not YouTube's: the app does not play it.
        Assert.Null(Trailers.Switch(Titled(3, new Trailer(TrailerHost.Dailymotion, "clip0002"))));
#if KAERI_TESTING
        // The fixture scene's trailers play their own pages, which have no player to switch.
        Assert.Null(Trailers.Switch(Titled(4, new Trailer(TrailerHost.YouTube, Trailers.FixtureId))));
        Assert.Null(Trailers.Switch(Titled(5, new Trailer(TrailerHost.YouTube, Trailers.FixtureStuckId))));
#endif
    }

    [Fact]
    public void AHeldTrailerPlaysUnseenAndShowsAsSoonAsItIsUnheld()
    {
        var flow = new TrailerFlow();
        flow.Hold();
        Assert.True(flow.Start());
        Assert.True(flow.Held);
        Assert.Equal(TrailerEffect.None, flow.Heard(new TrailerMessage.Playing()));
        Assert.Equal(TrailerPhase.Playing, flow.Phase);
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.ShowVideo, flow.Unhold());
        Assert.True(flow.Showing);
        Assert.Equal(TrailerEffect.None, flow.Unhold());
    }

    [Fact]
    public void AHeldTrailerUnheldBeforeItPlaysShowsWhenAFrameDoes()
    {
        var flow = new TrailerFlow();
        flow.Hold();
        flow.Start();
        Assert.Equal(TrailerEffect.None, flow.Unhold());
        Assert.Equal(TrailerEffect.ShowVideo, flow.Heard(new TrailerMessage.Playing()));
    }

    [Fact]
    public void LettingGoOfAHeldTrailerHasNoVideoToHide()
    {
        var flow = new TrailerFlow();
        flow.Hold();
        flow.Start();
        flow.Heard(new TrailerMessage.Playing());
        Assert.Equal(TrailerEffect.Release, flow.Stop());
        Assert.False(flow.Held);
        // The next visit is seen from the start unless it is held again.
        Assert.True(flow.Start());
        Assert.Equal(TrailerEffect.ShowVideo, flow.Heard(new TrailerMessage.Playing()));
    }

    sealed class Page(EmbedStyle style, string subtitles = "", bool broken = false) : IWarmPage
    {
        public EmbedStyle Style => style;
        public string Subtitles => subtitles;
        public bool Broken => broken;
    }

    [Fact]
    public void TheShelfKeepsTwoPagesOfAStyleAndHandsOneBackToSwitch()
    {
        var shelf = new TrailerShelf<Page>();
        Assert.True(shelf.Warm());
        Assert.False(shelf.Warm());
        var (a, b, c) = (new Page(EmbedStyle.Banner), new Page(EmbedStyle.Banner), new Page(EmbedStyle.Banner));
        Assert.True(shelf.Give(a));
        Assert.False(shelf.Give(a));
        Assert.True(shelf.Give(b));
        Assert.False(shelf.Give(c));
        Assert.True(shelf.Give(new Page(EmbedStyle.Full)));
        var drop = new List<Page>();
        Assert.Same(a, shelf.Take(EmbedStyle.Banner, "", drop));
        Assert.Same(b, shelf.Take(EmbedStyle.Banner, "", drop));
        Assert.Null(shelf.Take(EmbedStyle.Banner, "", drop));
        Assert.Empty(drop);
    }

    [Fact]
    public void EmptyingTheShelfLetsEveryPageGoAndWarmsAgain()
    {
        var shelf = new TrailerShelf<Page>();
        Assert.True(shelf.Warm());
        var (a, b) = (new Page(EmbedStyle.Banner), new Page(EmbedStyle.Full));
        shelf.Give(a);
        shelf.Give(b);
        var drop = new List<Page>();
        shelf.Empty(drop);
        Assert.Equal([a, b], drop);
        Assert.Empty(shelf.Idle);
        Assert.False(shelf.Warmed);
        Assert.True(shelf.Warm());
    }

    [Fact]
    public void ThePagesOfAnotherSubtitleLanguageOrBrokenOnesAreLetGo()
    {
        var shelf = new TrailerShelf<Page>();
        var english = new Page(EmbedStyle.Banner, "eng");
        var german = new Page(EmbedStyle.Banner, "ger");
        shelf.Give(english);
        shelf.Give(german);
        Assert.False(shelf.Give(new Page(EmbedStyle.Banner, "eng", broken: true)));
        var drop = new List<Page>();
        Assert.Same(german, shelf.Take(EmbedStyle.Banner, "ger", drop));
        Assert.Equal([english], drop);
        Assert.Empty(shelf.Idle);
    }

    [Fact]
    public void TheTrailerIsCuedWhileThePointerRestsAndLetGoWhenItLeavesBeforeTheCard()
    {
        Assert.Equal(TimeSpan.FromMilliseconds(150), HoverCue.After(TimeSpan.FromMilliseconds(450)));
        // Scanning a row, the card opens as soon as a cue would.
        Assert.Null(HoverCue.After(TimeSpan.FromMilliseconds(150)));
        var cue = new HoverCue();
        Assert.Equal(HoverTrailerStep.Start, cue.Cue(7));
        Assert.Equal(HoverTrailerStep.None, cue.Cue(7));
        Assert.Equal(HoverTrailerStep.None, cue.Uncue(8));
        Assert.Equal(HoverTrailerStep.Stop, cue.Uncue(7));
        Assert.Null(cue.Id);
        Assert.Equal(HoverTrailerStep.Start, cue.Cue(8));
        Assert.Equal(HoverTrailerStep.Replace, cue.Cue(9));
        Assert.Equal(9u, cue.Id);
    }

    [Fact]
    public void TheCardTakesTheTrailerCuedForItAndStartsOneOtherwise()
    {
        var cue = new HoverCue();
        cue.Cue(7);
        Assert.Equal(HoverTrailerStep.Adopt, cue.Open(7, live: true, plays: true));
        Assert.True(cue.Adopted);
        // The open card keeps its trailer: leaving the cover for the card lets nothing go.
        Assert.Equal(HoverTrailerStep.None, cue.Uncue(7));
        Assert.Equal(HoverTrailerStep.Stop, cue.Closed());
        Assert.Equal(HoverTrailerStep.None, cue.Closed());

        // Cued, but it gave up or another cover's was cued: the card starts its own.
        cue.Cue(7);
        Assert.Equal(HoverTrailerStep.Replace, cue.Open(7, live: false, plays: true));
        cue.Closed();
        cue.Cue(8);
        Assert.Equal(HoverTrailerStep.Replace, cue.Open(7, live: true, plays: true));
        cue.Closed();
        Assert.Equal(HoverTrailerStep.Start, cue.Open(7, live: false, plays: true));
        cue.Closed();

        // A card with no trailer that may play lets a cued one go.
        cue.Cue(7);
        Assert.Equal(HoverTrailerStep.Stop, cue.Open(7, live: true, plays: false));
        Assert.Null(cue.Id);
        Assert.Equal(HoverTrailerStep.None, cue.Open(7, live: false, plays: false));
    }

    [Fact]
    public void ACueWhileACardHasItsTrailerStartsWhenThatCardCloses()
    {
        var cue = new HoverCue();
        cue.Cue(7);
        cue.Open(7, live: true, plays: true);
        // Straight from the card onto a neighbour: its cue comes before the card has closed.
        Assert.Equal(HoverTrailerStep.None, cue.Cue(8));
        Assert.Equal(8u, cue.Queued);
        Assert.Equal(7u, cue.Id);
        Assert.Equal(HoverTrailerStep.Replace, cue.Closed());
        Assert.Equal(8u, cue.Id);
        Assert.Null(cue.Queued);
        Assert.False(cue.Adopted);

        // Leaving the neighbour before then lets the queued cue go.
        cue.Open(8, live: true, plays: true);
        cue.Cue(9);
        cue.Uncue(9);
        Assert.Equal(HoverTrailerStep.Stop, cue.Closed());
        Assert.Null(cue.Id);
    }

    [Fact]
    public void AScrollOrStoppingEveryTrailerLeavesNothingCued()
    {
        var cue = new HoverCue();
        cue.Cue(7);
        Assert.Equal(HoverTrailerStep.Stop, cue.Scrolled());
        Assert.Null(cue.Id);
        Assert.Equal(HoverTrailerStep.None, cue.Scrolled());

        // A scroll closes the open card by itself; only the queued cue goes here.
        cue.Cue(7);
        cue.Open(7, live: true, plays: true);
        cue.Cue(8);
        Assert.Equal(HoverTrailerStep.None, cue.Scrolled());
        Assert.Null(cue.Queued);
        Assert.Equal(HoverTrailerStep.Stop, cue.Closed());

        cue.Cue(7);
        cue.Open(7, live: true, plays: true);
        cue.Cue(8);
        Assert.Equal(HoverTrailerStep.Stop, cue.Reset());
        Assert.Null(cue.Id);
        Assert.Null(cue.Queued);
        Assert.False(cue.Adopted);
        Assert.Equal(HoverTrailerStep.None, cue.Closed());
    }

    [Fact]
    public void MessagesFromThePageAreReadByTheEngine()
    {
        Assert.IsType<TrailerMessage.Playing>(KaeriFfiMethods.TrailerMessage("playing"));
        Assert.Equal(new TrailerMessage.Error(101), KaeriFfiMethods.TrailerMessage("error:101"));
        Assert.Null(KaeriFfiMethods.TrailerMessage("hello"));
    }

    [Fact]
    public void TrailersWaitForTheEnginesWordAndFollowIt()
    {
        var known = new Known();
        var entry = Titled(1, Clip);
        Assert.False(Trailers.MayAutoplay(known, entry));
        known.Take(new Event.Autoplay(new AutoplayView(true)));
        Assert.True(Trailers.MayAutoplay(known, entry));
        Assert.False(Trailers.MayAutoplay(known, Titled(2)));
        known.Take(new Event.Autoplay(new AutoplayView(false, AutoplayHold.Network)));
        Assert.False(Trailers.MayAutoplay(known, entry));
    }

    [Fact]
    public void ThePreviewLeavesOutWhatASpoilerTagMarks()
    {
        var entry = Titled(1) with
        {
            Synopsis = "A courier crosses the lake. She dies at the end.",
            SynopsisSpans = [new SynopsisSpan("A courier crosses the lake. "), new SynopsisSpan("She dies at the end.", true)],
        };
        Assert.Equal("A courier crosses the lake.", Trailers.SpoilerFree(entry));
        Assert.Equal("Plain.", Trailers.SpoilerFree(Titled(2) with { Synopsis = "Plain." }));
    }

    [Fact]
    public void PlayTrailersOffersTheThreeChoicesTheEngineKeeps()
    {
        var options = SettingsCatalog.Options(SettingKey.Trailers);
        Assert.Equal(["always", "Wi-Fi only", "off"], options.Select(o => o.Label));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(new SettingRow(SettingKey.Trailers, "always", new SettingChange.Steps(true, true))));
    }
}
