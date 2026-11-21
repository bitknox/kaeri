// Private watching and reading as the window and pages read it: the mode, each title's own flag,
// the line under a main action, the media overlay's words, the downloads no notification names
// and the cast show no file keeps.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class PrivatelyTests
{
    [Fact]
    public void TheModeFollowsTheEngineAndSaysWhenItChanged()
    {
        var known = new Known();
        var changes = 0;
        known.Private.Changed += () => changes++;
        // Launch says off: nothing changed.
        known.Take(new Event.Private(false));
        Assert.False(known.Private.On);
        Assert.Equal(0, changes);
        known.Take(new Event.Private(true));
        Assert.True(known.Private.On);
        Assert.Equal(1, changes);
        known.Take(new Event.Private(false));
        Assert.False(known.Private.On);
        Assert.Equal(2, changes);
    }

    [Fact]
    public void TheLineSaysWhatIsKeptAndTheTitlesOwnFlagComesFirst()
    {
        var known = new Known();
        Assert.Null(known.Private.ShowLine(7));
        known.Take(new Event.Private(true));
        Assert.Equal(new PrivateLine("Private", "Not saved or synced"), known.Private.ShowLine(7));
        // A place held in memory until the app quits.
        known.Take(new Event.Progress(7, null, null, [], PrivateMode.Session, true));
        Assert.Equal(new PrivateLine("Private", "Your place is kept until Kaeri quits"), known.Private.ShowLine(7));
        Assert.Equal("Private, Your place is kept until Kaeri quits", known.Private.ShowLine(7)!.Spoken);
        // Always private wins over the mode, and stays when the mode goes off.
        known.Take(new Event.AlwaysPrivate(new PrivateTitle.Show(7), true));
        Assert.Equal(new PrivateLine("Always private", "Not saved or synced"), known.Private.ShowLine(7));
        known.Take(new Event.Private(false));
        Assert.Equal(new PrivateLine("Always private", "Not saved or synced"), known.Private.ShowLine(7));
        Assert.True(known.Private.ShowAlways(7));
        Assert.False(known.Private.ShowAlways(8));
        known.Take(new Event.AlwaysPrivate(new PrivateTitle.Show(7), false));
        Assert.Null(known.Private.ShowLine(7));
    }

    [Fact]
    public void AShowsFlagIsReadFromItsProgressAndItsSettings()
    {
        var known = new Known();
        known.Take(new Event.Progress(7, null, null, [], PrivateMode.Always, false));
        Assert.True(known.Private.ShowAlways(7));
        known.Take(new Event.ShowSettings(new ShowSettingsView(7, Rows: [], Remembered: [], AlwaysPrivate: false)));
        Assert.False(known.Private.ShowAlways(7));
        known.Take(new Event.ShowSettings(new ShowSettingsView(7, Rows: [], Remembered: [], AlwaysPrivate: true)));
        Assert.True(known.Private.ShowAlways(7));
    }

    [Fact]
    public void WhatTheAppKeepsOfAShowNeverOutlivesAPrivateSession()
    {
        var known = new Known();
        // Until the engine says the show's flag this launch, it may be always private.
        Assert.False(known.Private.MayKeep(7));
        known.Take(new Event.Progress(7, null, null, [], null, false));
        Assert.True(known.Private.MayKeep(7));
        known.Take(new Event.Private(true));
        Assert.False(known.Private.MayKeep(7));
        known.Take(new Event.Private(false));
        known.Take(new Event.AlwaysPrivate(new PrivateTitle.Show(7), true));
        Assert.False(known.Private.MayKeep(7));
        known.Take(new Event.ShowSettings(new ShowSettingsView(7, Rows: [], Remembered: [], AlwaysPrivate: false)));
        Assert.True(known.Private.MayKeep(7));
        Assert.False(known.Private.MayKeep(8));
    }

    [Fact]
    public void APrivateCastLeavesNoShowForTheNextLaunchToName()
    {
        HomeGuard.Check();
        var dir = Path.Combine(Path.GetTempPath(), "kaeri-cast-test-" + Guid.NewGuid().ToString("N"));
        var path = Path.Combine(dir, "windows-cast-last");
        var known = new Known();
        try
        {
            known.Take(new Event.Progress(7, null, null, [], null, false));
            known.Take(new Event.Progress(8, null, null, [], PrivateMode.Always, false));
            var last = new LastCast(path, known.Private);
            last.Playing(7, "Made-up Show");
            Assert.Equal((7u, "Made-up Show"), new LastCast(path, new Known().Private).Remembered());
            // Private mode on: the file goes, and only this launch still names the show.
            known.Take(new Event.Private(true));
            last.Playing(7, "Made-up Show");
            Assert.False(File.Exists(path), "a private cast stays in no file");
            Assert.Equal((7u, "Made-up Show"), last.Remembered());
            Assert.Null(new LastCast(path, new Known().Private).Remembered());
            known.Take(new Event.Private(false));
            // A show always private, or one whose flag is not said yet, leaves none either.
            last.Playing(8, "Harbour Lights");
            Assert.False(File.Exists(path), "an always-private show stays in no file");
            last.Playing(9, "Paper Lanterns");
            Assert.False(File.Exists(path), "a show not known to be kept stays in no file");
            // Nor does an earlier show stay to be named in place of a private one.
            last.Playing(7, "Made-up Show");
            Assert.True(File.Exists(path));
            last.Playing(8, "Harbour Lights");
            Assert.False(File.Exists(path), "the earlier show goes once a private one plays");
        }
        finally
        {
            known.Take(new Event.Private(false));
            if (Directory.Exists(dir))
                Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void ASeriesKeepsItsOwnFlag()
    {
        var known = new Known();
        known.Take(new Event.AlwaysPrivate(new PrivateTitle.Series("a:900301"), true));
        Assert.True(known.Private.SeriesAlways("a:900301"));
        Assert.False(known.Private.SeriesAlways("a:900302"));
        known.Take(new Event.AlwaysPrivate(new PrivateTitle.Series("a:900301"), false));
        Assert.False(known.Private.SeriesAlways("a:900301"));
    }

    [Fact]
    public void ThePageLineFollowsTheModeNowNotWhenTheViewWasSent()
    {
        Assert.Null(Privately.Line(null, false, false));
        Assert.Equal("Private", Privately.Line(PrivateMode.Session, true, false)!.Head);
        // A view sent while private, read after the mode went off, shows no line.
        Assert.Null(Privately.Line(PrivateMode.Session, false, true));
        Assert.Equal("Always private", Privately.Line(PrivateMode.Always, false, false)!.Head);
    }

    [Fact]
    public void TheMediaOverlayNamesNothingWhilePrivate()
    {
        Assert.Equal(("Playing in Kaeri", "", false), Privately.NowPlaying("Lanterns of the Quiet Bay", "Episode 3", neutral: true));
        Assert.Equal(("Episode 3", "Lanterns of the Quiet Bay", true), Privately.NowPlaying("Lanterns of the Quiet Bay", "Episode 3", neutral: false));
        var known = new Known();
        Assert.False(known.Private.Neutral(alwaysPrivate: false));
        Assert.True(known.Private.Neutral(alwaysPrivate: true));
        known.Take(new Event.Private(true));
        Assert.True(known.Private.Neutral(alwaysPrivate: false));
        known.Take(new Event.Private(false));
    }

    [Fact]
    public void ScreenReadersAndTheSettingsLineHaveTheirWords()
    {
        Assert.Equal("Private mode on", Privately.Announcement(true));
        Assert.Equal("Private mode off", Privately.Announcement(false));
        Assert.Equal("Your sources and watch parties still see what you open. Shortcut: Ctrl+Shift+N", Privately.Under);
    }

    [Fact]
    public void APrivateDownloadPostsNoNotification()
    {
        static DownloadRow Row(long id, DownloadState state, bool hidden) =>
            new DownloadRow(id, 7, "Lanterns of the Quiet Bay", id.ToString(), state, 0.5, 0, 0) with { Private = hidden };
        DownloadRow[] before = [Row(1, DownloadState.Active, true), Row(2, DownloadState.Active, false)];
        DownloadRow[] now = [Row(1, DownloadState.Done, true), Row(2, DownloadState.Done, false)];
        var notices = Notices.Downloaded(before, now);
        Assert.Equal(["Episode 2 downloaded."], notices.Select(n => n.Body));
        Assert.Empty(Notices.Downloaded([Row(1, DownloadState.Active, true)], [Row(1, DownloadState.Done, true)]));
    }

    [Fact]
    public void LogLinesLeaveIdsOutWhilePrivate()
    {
        var known = new Known();
        known.Take(new Event.Private(false));
        Assert.Equal("900001", Privately.Logged(900001u));
        known.Take(new Event.Private(true));
        Assert.Equal("(private)", Privately.Logged("cdn.example"));
        known.Take(new Event.Private(false));
    }
}
