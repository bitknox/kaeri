// How long a notice stays: by its kind, doubled for a screen reader, and for good with a button.

using Kaeri.App;
using Xunit;

namespace Kaeri.App.Tests;

public class NoticeTimeTests
{
    [Fact]
    public void ANoticeStaysByItsKindAndLength()
    {
        Assert.Equal(TimeSpan.FromSeconds(4), NoticeTime.Stays("Added to Planning", alert: false, hasAction: false, screenReader: false));
        Assert.Equal(TimeSpan.FromSeconds(8), NoticeTime.Stays("Could not reach the list", alert: true, hasAction: false, screenReader: false));
        Assert.Equal(TimeSpan.FromSeconds(5), NoticeTime.Stays(string.Join(' ', Enumerable.Repeat("word", 25)), alert: false, hasAction: false, screenReader: false));
    }

    [Fact]
    public void AScreenReaderDoublesTheTime()
    {
        Assert.Equal(TimeSpan.FromSeconds(8), NoticeTime.Stays("Added to Planning", alert: false, hasAction: false, screenReader: true));
        Assert.Equal(2, NoticeTime.Scale(screenReader: true));
        Assert.Equal(1, NoticeTime.Scale(screenReader: false));
    }

    [Fact]
    public void ANoticeWithAButtonStaysUntilDismissed()
    {
        Assert.Null(NoticeTime.Stays("Skipped opening", alert: false, hasAction: true, screenReader: false));
        Assert.Null(NoticeTime.Stays("Skipped opening", alert: true, hasAction: true, screenReader: true));
    }
}
