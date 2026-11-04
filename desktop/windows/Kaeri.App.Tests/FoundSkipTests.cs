// What the found skip times need of the app: the audio tap turned on, the switch, and the
// engine's fixture states and timings the card, the review and Settings are drawn from.

using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class FoundSkipTests
{
    [Fact]
    public void KaerisOwnLibmpvHandsItsAudioOver()
    {
        // The Windows libmpv Kaeri ships has the demuxer tap; the engine is told so at its start.
        Assert.Equal(KaeriFfiMethods.PlayerAudioTap(), EngineSession.AppStart().AudioTap);
        if (OperatingSystem.IsWindows())
            Assert.True(KaeriFfiMethods.PlayerAudioTap(), "the libmpv beside the engine has no mpv_set_audio_tap");
    }

    [Fact]
    public void FindingSkipTimesIsASwitch()
    {
        Assert.True(SettingsCatalog.IsSwitch(SettingKey.FindSkipTimes));
    }

    [Fact]
    public void EveryFixtureStateCrossesWithWhatTheScreensDrawFrom()
    {
        var prompt = KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Prompt);
        var asks = Assert.IsType<SkipPrompt.FoundEdges>(prompt.Prompt);
        Assert.Equal(SkipKind.Opening, asks.Kind);
        Assert.Equal(3u, asks.Episode);
        Assert.IsType<SkipPrompt.FoundSaved>(KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Saved).Prompt);
        Assert.IsType<SkipPrompt.FoundNever>(KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Never).Prompt);

        var review = KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Review).Mark!;
        Assert.True(review.Found);
        Assert.Equal(new SkipMarkNote[] { new SkipMarkNote.Found(), new SkipMarkNote.Shares() }, review.Notes);

        Assert.All(KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.Panel).Marks, m => Assert.Equal(SkipOrigin.Found, m.Origin));
        Assert.Equal(SkipFindPause.LowPower, KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.PanelPaused).Paused);
        Assert.Equal(SkipFindPause.Storage, KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.SettingsStorage).Paused);
        Assert.Null(KaeriFfiMethods.FoundSkipScene(FoundSkipFixture.SettingsOn).Paused);
    }
}
