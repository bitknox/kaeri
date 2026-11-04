// The party's conversation in words: names, times, ticks on the seek bar, muted runs, new
// lines, the field's count and how long a line holds on the video.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

[Collection("Strings")]
public class ChatTests
{
    public ChatTests()
    {
        T.Resolver = null;
        T.Language = "en";
    }

    static PartyChatLine Said(ulong id, string from, string text, bool you = false, bool muted = false) =>
        new(id, from, you, new PartyChatBody.Said(text), muted);

    static PartyChatLine Reacted(ulong id, string from, Reaction reaction, double at, params string[] with) =>
        new(id, from, false, new PartyChatBody.Reacted(reaction, at, with), false);

    [Fact]
    public void NamesReadAsASentence()
    {
        Assert.Equal("Ren", Chat.Names("Ren", []));
        Assert.Equal("Ren and Mika", Chat.Names("Ren", ["Mika"]));
        Assert.Equal("Ren, Mika and Sol", Chat.Names("Ren", ["Mika", "Sol"]));
        Assert.Equal("Ren, Mika and 2 others", Chat.Names("Ren", ["Mika", "Sol", "Ada"]));
    }

    [Fact]
    public void TimesRoundToTheSecondAndShowHoursOnlyPastTheFirst()
    {
        Assert.Equal("0:00", Chat.Time(-3));
        Assert.Equal("0:03", Chat.Time(2.5));
        Assert.Equal("12:04", Chat.Time(724.2));
        Assert.Equal("1:00:05", Chat.Time(3605));
    }

    [Fact]
    public void AReactionLineNamesEveryoneAndWhen()
    {
        Assert.Equal("Ren and Mika laughed at 12:04", Chat.ReactionLine(Reacted(4, "Ren", Reaction.Laugh, 724, "Mika")));
        Assert.Null(Chat.ReactionLine(Said(5, "Ren", "hi")));
        Assert.Equal("You", Chat.Who(Said(6, "Ada", "hi", you: true)));
    }

    [Fact]
    public void MarksCloseTogetherShareATick()
    {
        // 600 s over 600 px: a second is a pixel, and marks within 4 px share the tick before them.
        PartyReactionMark[] marks =
        [
            new(100, "Mika", Reaction.Laugh),
            new(10, "Ren", Reaction.Gasp),
            new(103, "Sol", Reaction.Laugh),
            new(104, "Mika", Reaction.Clap),
            new(200, "Ren", Reaction.Cry),
        ];
        var ticks = Chat.Ticks(marks, 600, 600, 4);
        Assert.Equal([10.0, 100.0, 200.0], ticks.Select(t => t.At));
        Assert.Equal(["Mika", "Sol"], ticks[1].Who);
        Assert.Equal("1:40 · Mika and Sol", ticks[1].Label());
        Assert.Empty(Chat.Ticks(marks, 0, 600, 4));
    }

    [Fact]
    public void AMutedMembersRunIsOneLine()
    {
        PartyChatLine[] lines =
        [
            Said(1, "Mika", "ok starting"),
            Said(2, "Sol", "one", muted: true),
            Said(3, "Sol", "two", muted: true),
            Said(4, "Ren", "ready"),
            Said(5, "Sol", "three", muted: true),
            Reacted(6, "Ren", Reaction.Gasp, 90),
        ];
        var entries = Chat.Entries(lines);
        Assert.Equal(
            [
                new Chat.Entry.Said("Mika", false, "ok starting"),
                new Chat.Entry.Muted("Sol"),
                new Chat.Entry.Said("Ren", false, "ready"),
                new Chat.Entry.Muted("Sol"),
                new Chat.Entry.Reacted("Ren gasped at 1:30"),
            ],
            entries);
    }

    [Fact]
    public void FreshLinesAreTheTypedUnmutedOnesAfterTheLastSeen()
    {
        PartyChatLine[] lines =
        [
            Said(1, "Mika", "ok starting"),
            Said(2, "Sol", "muted", muted: true),
            Reacted(3, "Ren", Reaction.Laugh, 3),
            Said(4, "Ren", "ready"),
        ];
        var (fresh, newest) = Chat.Fresh(lines, 1);
        Assert.Equal([4UL], fresh.Select(l => l.Id));
        Assert.Equal(4UL, newest);
        Assert.Equal(7UL, Chat.Fresh([], 7).Newest);
    }

    [Fact]
    public void TheFieldsEndCountsDownTheLastCharactersOrSaysTheWait()
    {
        Assert.Null(Chat.Tail("hello", 150, 0));
        Assert.Equal("20", Chat.Tail(new string('a', 130), 150, 0));
        Assert.Equal("0", Chat.Tail(new string('a', 40), 40, 0));
        Assert.Null(Chat.Tail(new string(' ', 140), 150, 0));
        Assert.Equal("Wait 3 s", Chat.Tail("hello", 150, 3));
    }

    [Fact]
    public void ALineHoldsFourSecondsAndLongerForLongLines()
    {
        Assert.Equal(TimeSpan.FromSeconds(4), Chat.Hold("ok starting"));
        Assert.Equal(TimeSpan.FromSeconds(4), Chat.Hold(string.Join(' ', Enumerable.Repeat("word", 12))));
        Assert.Equal(TimeSpan.FromSeconds(5), Chat.Hold(string.Join(' ', Enumerable.Repeat("word", 22))));
    }
}
