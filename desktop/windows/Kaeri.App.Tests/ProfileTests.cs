using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

public class ProfileTests
{
    static ProfileView Profile(string id, string name, bool active = false, bool kids = false, string initials = "") =>
        new(id, name, initials, kids, active);

    [Fact]
    public void OneProfileMeetsNoPickerAndNoName()
    {
        ProfileView[] one = [Profile("a", "", active: true)];
        Assert.False(ProfileWords.Picking(one));
        Assert.Null(ProfileWords.Active(one));
        Assert.False(ProfileWords.Picking([]));
    }

    [Fact]
    public void TwoProfilesNameTheOneInUse()
    {
        ProfileView[] two = [Profile("a", "Alex"), Profile("b", "Kim", active: true)];
        Assert.True(ProfileWords.Picking(two));
        Assert.Equal("Kim", ProfileWords.Active(two)?.Name);
    }

    [Fact]
    public void InitialsAreTheEnginesElseTheFirstLetter()
    {
        Assert.Equal("SM", ProfileWords.Initials(Profile("a", "study mac", initials: "SM")));
        Assert.Equal("K", ProfileWords.Initials(Profile("a", " kim")));
        Assert.Equal("", ProfileWords.Initials(Profile("a", " ")));
    }

    [Fact]
    public void NamesAreTrimmedAndNeverEmpty()
    {
        Assert.Equal("Sam", ProfileWords.Typed("  Sam "));
        Assert.Null(ProfileWords.Typed("   "));
    }

    [Fact]
    public void ARelaunchWaitsForThisCopyAndDropsLinksAndShots()
    {
        var command = ProfileWords.Relaunch(4242, @"C:\Users\o'neil\Kaeri\Kaeri.exe", ["--scale=1.5", "kaeri:open/7", "--show=profiles-add"]);
        Assert.StartsWith("-NoProfile -NonInteractive -WindowStyle Hidden -Command \"Wait-Process -Id 4242 ", command);
        Assert.Contains(@"Start-Process -FilePath 'C:\Users\o''neil\Kaeri\Kaeri.exe' -ArgumentList '--scale=1.5'", command);
        Assert.DoesNotContain("kaeri:", command);
        Assert.DoesNotContain("--show", command);
        Assert.EndsWith("\"", command);
    }

    [Fact]
    public void ARelaunchWithNoArgumentsPassesNone()
    {
        Assert.DoesNotContain("-ArgumentList", ProfileWords.Relaunch(1, @"C:\Kaeri.exe", []));
    }

    [Fact]
    public void KnownKeepsTheProfilesTheEngineSaid()
    {
        var known = new Known();
        known.Take(new Event.Profiles(new ProfilesView([Profile("a", "Alex", active: true), Profile("b", "Sam", kids: true)])));
        Assert.Equal(2, known.Profiles.Length);
        Assert.True(known.Profiles[1].Kids);
    }
}
