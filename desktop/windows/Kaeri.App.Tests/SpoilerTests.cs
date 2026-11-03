// Spoiler protection: whether it is on for a show, which stills it blurs, and that both follow a
// change in Settings or on the show's own sheet the moment the engine says it.

using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class SpoilerTests
{
    const uint Show = 41;

    static Event Settings(bool on) => new Event.Settings(new SettingsView(
        Translation: default, Quality: 1080, Theme: default, Sources: new ConfiguredSources([], []), PresenceClient: false,
        Torrent: new TorrentView(), Rows: [],
        NativeRows: [new SettingRow(SettingKey.SpoilerProtection, on ? "on" : "off", new SettingChange.Steps(true, true))]));

    static Event Sheet(bool? own, bool settings) => new Event.ShowSettings(new ShowSettingsView(Show,
        Rows: [new ShowSettingRow(ShowSettingKey.SpoilerProtection,
            Value: (own ?? settings) ? "on" : "off", Default: settings ? "on" : "off",
            Own: own is { } value ? new SettingValue.Bool(value) : null)],
        Remembered: []));

    static EpisodeRow Row(string number, bool completed = false) => new(number, Completed: completed);

    [Fact]
    public void ProtectionIsOnUntilTheEngineSaysAndThenFollowsSettingsBothWays()
    {
        var known = new Known();
        Assert.True(Spoilers.On(known, Show));
        known.Take(Settings(false));
        Assert.False(Spoilers.On(known, Show));
        known.Take(Settings(true));
        Assert.True(Spoilers.On(known, Show));
        known.Take(Settings(false));
        Assert.False(Spoilers.On(known, Show));
    }

    [Fact]
    public void AShowsOwnValueWinsAndFollowingSettingsAgainTracksEveryLaterChange()
    {
        var known = new Known();
        known.Take(Settings(true));
        known.Take(Sheet(own: false, settings: true));
        Assert.False(Spoilers.On(known, Show));
        Assert.True(Spoilers.On(known, Show + 1));

        // Following Settings, the sheet's value in effect is as old as the sheet: Settings' row decides.
        known.Take(Sheet(own: null, settings: true));
        known.Take(Settings(false));
        Assert.False(Spoilers.On(known, Show));
        known.Take(Settings(true));
        Assert.True(Spoilers.On(known, Show));

        known.Take(Sheet(own: true, settings: false));
        known.Take(Settings(false));
        Assert.True(Spoilers.On(known, Show));
    }

    [Fact]
    public void OnlyStillsAfterTheNextEpisodeBlurAndNoneWhileProtectionIsOff()
    {
        const uint next = 3;
        Assert.False(Spoilers.StillHidden(Row("2"), next, on: true));
        Assert.False(Spoilers.StillHidden(Row("3"), next, on: true));
        Assert.True(Spoilers.StillHidden(Row("4"), next, on: true));
        Assert.False(Spoilers.StillHidden(Row("4", completed: true), next, on: true));
        Assert.False(Spoilers.StillHidden(Row("OVA"), next, on: true));
        Assert.False(Spoilers.StillHidden(Row("4"), next, on: false));
        Assert.False(Spoilers.StillHidden(Row("9"), next, on: false));
    }
}
