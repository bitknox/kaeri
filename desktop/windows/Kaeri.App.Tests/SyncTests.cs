using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class SyncTests
{
    const long Now = 1_800_000_000;

    static SyncDeviceView Device(long? seen, string? remoteKey = null, SyncFolderKind? folder = SyncFolderKind.Dropbox) =>
        new("b", "Mika's phone", false, seen, [], [], folder, [], SyncDeviceKind.Phone, RemoteKey: remoteKey);

    static SyncStatus Status(SyncProblem? problem = null, bool syncing = false, SyncFolderKind kind = SyncFolderKind.Dropbox, params SyncCloudView[] clouds) =>
        new(true, syncing, "Study PC", new SyncFolderView(kind, "Dropbox", null), [], Now - 120, null, [], [], clouds, problem);

    [Fact]
    public void RecoveryKeysReadInGroupsOfFourUpperCase()
    {
        Assert.Equal(new[] { "ABCD", "EFGH", "JKLM", "NPQR", "STUV", "WXYZ" }, SyncWords.KeyGroups("abcd-efgh jklm\nnpqr stuv wxyz"));
        Assert.True(SyncWords.KeyReady("ABCD EFGH JKLM NPQR STUV WXYZ"));
        Assert.False(SyncWords.KeyReady("ABCD EFGH JKLM NPQR STUV WXY"));
    }

    [Fact]
    public void PairingCodesAreSixDigitsSpacedThreeAndThree()
    {
        Assert.Equal("042 917", SyncWords.SpacedCode("042917"));
        Assert.Equal("04291", SyncWords.SpacedCode("04291"));
        Assert.True(SyncWords.CodeReady("042 917"));
        Assert.False(SyncWords.CodeReady("04291a"));
        Assert.Equal("042917", SyncWords.CodeDigits("0-4 2a917"));
        Assert.Equal("kaeri://pair/042917@192.0.2.4:47812", SyncWords.PairLink("042917", "192.0.2.4:47812"));
    }

    [Fact]
    public void TheCountdownReadsMinutesAndSecondsAndStopsAtNothing()
    {
        Assert.Equal("1:48", SyncWords.MinutesSeconds(108));
        Assert.Equal("0:05", SyncWords.MinutesSeconds(5));
        Assert.Equal("0:00", SyncWords.MinutesSeconds(-3));
    }

    [Fact]
    public void ADeviceUnseenForAMonthIsMarkedAmber()
    {
        Assert.Equal(SyncMark.Water, SyncWords.SeenLine(Device(Now - 300), Now).Mark);
        var (line, mark) = SyncWords.SeenLine(Device(Now - 31 * 86_400), Now);
        Assert.Equal(SyncMark.Amber, mark);
        Assert.Contains("31", line);
        Assert.Equal(("", SyncMark.None), SyncWords.SeenLine(Device(null), Now));
    }

    [Fact]
    public void RemoteJoinsTheLineOnlyForATvPair()
    {
        Assert.DoesNotContain(T.DevicesRemoteSwitch, SyncWords.DeviceLine(Device(Now - 60), Now, false).Line);
        Assert.EndsWith($" · {T.DevicesRemoteSwitch}", SyncWords.DeviceLine(Device(Now - 60, "0a1b"), Now, false).Line);
        Assert.Equal(T.DevicesRemoteOnly, SyncWords.DeviceLine(Device(null, folder: null), Now, true).Line);
    }

    [Fact]
    public void TheCloudCardSaysWhatTheFolderNeeds()
    {
        Assert.IsType<CloudState.Syncing>(SyncWords.State(Status(syncing: true), Now, false));
        Assert.IsType<CloudState.Synced>(SyncWords.State(Status(), Now, false));
        var signedOut = new SyncProblem.SignInNeeded(SyncFolderKind.Dropbox);
        // Signed out by hand is the person's own doing; otherwise the service signed Kaeri out.
        Assert.IsType<CloudState.SignedOut>(SyncWords.State(Status(signedOut), Now, byHand: true));
        var problem = Assert.IsType<CloudState.Problem>(SyncWords.State(Status(signedOut), Now, false));
        Assert.Equal(ProblemFix.SignInAgain, problem.Fix);
        Assert.Contains("PC", problem.Line);
        Assert.IsType<CloudState.Offline>(SyncWords.State(Status(new SyncProblem.Unreachable()), Now, false));
        var gone = Assert.IsType<CloudState.Problem>(SyncWords.State(Status(new SyncProblem.Unreachable(), kind: SyncFolderKind.Directory), Now, false));
        Assert.Equal(ProblemFix.ChooseFolder, gone.Fix);
    }

    [Fact]
    public void ThePickerOffersOnlyWhatSignsInHereAlphabetically()
    {
        var status = Status(clouds:
        [
            new(SyncFolderKind.OneDrive, new SyncCloudWay.Browser(), false),
            new(SyncFolderKind.GoogleDrive, new SyncCloudWay.Unavailable(CloudUnavailable.NoClientId), false),
            new(SyncFolderKind.Dropbox, new SyncCloudWay.DeviceCode(), false),
        ]);
        Assert.Equal(new[] { SyncFolderKind.Nextcloud, SyncFolderKind.OneDrive }, SyncWords.Offered(status).Select(o => o.Kind));
    }

    [Fact]
    public void ProblemsSayWhatWentWrongForTheServiceChosen()
    {
        Assert.Contains("cloud.example", SyncWords.ProblemText(new SyncProblem.Unreachable(), SyncFolderKind.Nextcloud, "cloud.example"));
        Assert.Equal(T.CloudErrWebdav, SyncWords.ProblemText(new SyncProblem.Unreachable(), SyncFolderKind.WebDav, ""));
        Assert.Equal(T.RkeyWrong, SyncWords.ProblemText(new SyncProblem.WrongRecoveryKey(), null, ""));
        Assert.Equal("cloud.example:8443", SyncWords.HostOf(" https://cloud.example:8443/remote.php/dav "));
    }

    [Fact]
    public void ASignInThatFailsSaysWhichStepFailed()
    {
        var refused = SyncWords.SignInError(new SyncProblem.SignInDenied(SyncFolderKind.GoogleDrive), SyncFolderKind.GoogleDrive, "", false);
        Assert.Equal(T.CloudErrSigninTitle("Google Drive"), refused.Title);
        Assert.Equal(T.CloudErrDeniedShort("Google Drive"), refused.Line);
        var late = SyncWords.SignInError(new SyncProblem.SignInExpired(SyncFolderKind.GoogleDrive), SyncFolderKind.GoogleDrive, "", false);
        Assert.Equal(T.CloudErrTimeout, late.Line);
        var other = SyncWords.SignInError(new SyncProblem.Other("details"), SyncFolderKind.GoogleDrive, "details", false);
        Assert.Equal(T.CloudErrSigninOther("Google Drive"), other.Line);
        // After the folder was reached the title says the set-up failed, with the problem's own line.
        var full = SyncWords.SignInError(new SyncProblem.Other("full"), SyncFolderKind.GoogleDrive, "full", true);
        Assert.Equal((T.CloudErrSetupTitle, "full"), full);
    }

    [Fact]
    public void SyncOnWithNothingToSyncWithReadsAsOff()
    {
        Assert.True(SyncWords.Bare(new SyncStatus(false, false, Devices: [], Notes: [], StoresToJoin: [], Clouds: [])));
        var alone = Status() with { Folder = null, Devices = [Device(Now) with { ThisDevice = true }] };
        Assert.True(SyncWords.Bare(alone));
        Assert.False(SyncWords.Bare(Status()));
        Assert.False(SyncWords.Bare(alone with { Devices = [Device(Now)] }));
    }

    [Fact]
    public void CopySignInsAsksForTrackersAndEachCloud()
    {
        var device = Device(Now) with { Trackers = ["anilist", "mal"], SignIns = [SyncFolderKind.Dropbox] };
        Assert.Equal("AniList, MyAnimeList, Dropbox", SyncWords.SignInNames(device));
        var copy = SyncWords.CopyAll(device);
        Assert.IsType<SyncCopy.Trackers>(copy[0]);
        Assert.Equal(SyncFolderKind.Dropbox, Assert.IsType<SyncCopy.Cloud>(copy[1]).Service);
    }

    [Fact]
    public void KnownKeepsTheGroupTheInviteAndTheProblemsADialogAnswers()
    {
        var known = new Known();
        known.Take(new Event.SyncProblem(new SyncProblem.WrongRecoveryKey()));
        known.Take(new Event.SyncStatus(Status()));
        // A wrong key is the restore dialog's to clear, not a pass that worked.
        Assert.IsType<SyncProblem.WrongRecoveryKey>(known.SyncTrouble);
        known.Take(new Event.SyncProblem(new SyncProblem.Unreachable()));
        known.Take(new Event.SyncStatus(Status()));
        Assert.Null(known.SyncTrouble);
        Assert.Equal("Study PC", known.SyncGroup?.DeviceName);
        known.Take(new Event.SyncInvite("482913", 47_812, 120));
        Assert.Equal("482913", known.PairInvite?.Code);
        known.Take(new Event.SyncSignIn(SyncFolderKind.Dropbox, new SyncSignInStep.Browser("https://auth.example")));
        Assert.Equal(SyncFolderKind.Dropbox, known.SyncSigningIn?.Service);
        known.Take(new Event.SyncRecoveryKey("K7QF2M9XHD4RTB6WPN3ZC8JA"));
        Assert.Null(known.SyncSigningIn);
        Assert.Equal("K7QF2M9XHD4RTB6WPN3ZC8JA", known.RecoveryKey);
    }
}
