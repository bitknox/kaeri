// Credit pages, chapter parts, the group picker and Choose chapters' first ticks: the rules the
// Windows reader and series page draw from, and when a kept page reaches the engine.

using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;
using Medium = Kaeri.Engine.Core.Medium;

namespace Kaeri.App.Tests;

public class CreditPagesTests
{
    static PageFile File(string name = "") => new(name, new PageEncoding.Original("image/png"), 1, 1);

    static CreditPage Credit(uint page, bool atEnd, ulong fingerprint) => new(page, File($"credit-{fingerprint}.png"), fingerprint, atEnd);

    static ChapterPart Part(uint number, uint firstPage, string? group = null, bool other = false) => new(number, firstPage, group, other);

    // Parts

    [Fact]
    public void ThePreviewNamesThePartOnItsTickOnly()
    {
        var parts = new ReaderParts([Part(1, 1), Part(2, 25)], 2, []);
        Assert.Equal("Page 25 · part 2", ReaderSession.TipWords(parts, 25, 46));
        Assert.Null(ReaderSession.TipWords(parts, 24, 46));
        Assert.Null(ReaderSession.TipWords(parts, 1, 46));
        Assert.Equal("Part 2 starts at page 25", ReaderSession.PartWords(parts, 25, 46));
    }

    [Fact]
    public void ThePreviewLeavesAnotherGroupOut()
    {
        var parts = new ReaderParts([Part(1, 1, "Saltmarsh Team"), Part(2, 25, "Bluefinch Group", true)], 2, []);
        Assert.Equal("Page 25 · part 2", ReaderSession.TipWords(parts, 25, 46));
    }

    [Fact]
    public void AMissingPartIsNamedWhereTheNextOneStarts()
    {
        Assert.Equal(14.0, ReaderSession.MissingAt([Part(1, 1), Part(4, 14)], 3, 20));
        Assert.Equal(20.0, ReaderSession.MissingAt([Part(1, 1), Part(4, 14)], 5, 20));
        var parts = new ReaderParts([Part(1, 1), Part(4, 14)], 4, [3]);
        Assert.Equal("Page 14 · part 4 · part 3 missing", ReaderSession.TipWords(parts, 14, 20));
        Assert.Equal("Part 4 starts at page 14, part 3 missing", ReaderSession.PartWords(parts, 14, 20));
        Assert.Equal([14.0, 14.0], ReaderSession.Ticks(parts, 20));
    }

    [Fact]
    public void AMissingPartAfterTheLastSitsAtTheEnd()
    {
        var parts = new ReaderParts([Part(1, 1), Part(2, 10)], 3, [3]);
        Assert.Equal("Page 20 · part 3 missing", ReaderSession.TipWords(parts, 20, 20));
        Assert.Equal("Part 3 missing", ReaderSession.PartWords(parts, 20, 20));
    }

    // Keeping a page

    [Fact]
    public void AKeptPageTakesItsNumberLessThePagesStillLeftOutBeforeIt()
    {
        // Five pages: page 0 and page 4 left out, three in between.
        CreditPage[] credits = [Credit(0, false, 1), Credit(4, true, 2)];
        Assert.Equal(0u, ReaderSession.KeptNumber(credits, 1));
        Assert.Equal(3u, ReaderSession.KeptNumber(credits, 2));
        Assert.Equal(0u, ReaderSession.KeptNumber([Credit(0, false, 1), Credit(1, false, 2)], 2));
        Assert.Null(ReaderSession.KeptNumber(credits, 9));
    }

    [Fact]
    public void AHeldKeepCountsAtOnce()
    {
        // Kept at the start, on the kept page: page 1 of 38 where 37 were.
        Assert.Equal((1, 38), ReaderSession.HeldCounts(1, 37, false, true));
        // On a story page after it: one on.
        Assert.Equal((6, 38), ReaderSession.HeldCounts(5, 37, false, null));
        // Kept at the end, on it: the last page.
        Assert.Equal((38, 38), ReaderSession.HeldCounts(37, 37, true, true));
        Assert.Equal((5, 38), ReaderSession.HeldCounts(5, 37, true, null));
        // Another credit page on screen keeps its place.
        Assert.Equal((1, 38), ReaderSession.HeldCounts(1, 37, false, false));
        Assert.Equal((0, 0), ReaderSession.HeldCounts(0, 0, false, null));
    }

    [Fact]
    public void ANewLayoutKeepsThePageThatWasShowingOneOnFromAKeptPageBeforeIt()
    {
        var before = Leaves(5);
        var after = Leaves(6);
        // Page 3 showed; page 1 came back before it: page 3 is now page 4.
        Assert.Equal(4, ReaderSession.RelaidLeaf(before, 3, after, 1));
        // A page kept after it moves nothing.
        Assert.Equal(3, ReaderSession.RelaidLeaf(before, 3, after, 5));
        // The end page stays the end page.
        Assert.Equal(6, ReaderSession.RelaidLeaf(Leaves(5, end: true), 5, Leaves(6, end: true), 0));
    }

    [Fact]
    public void TurnsGoThroughShownCreditPages()
    {
        CreditPage[] credits = [Credit(0, false, 1), Credit(1, false, 2), Credit(9, true, 3)];
        // From the first start page: on to the second, back past the start.
        Assert.Equal(new CreditTurn(CreditTurnKind.Credit, 1), ReaderSession.CreditStep(credits, credits[0], true, false, 0, false, 6, 7, true));
        Assert.Equal(new CreditTurn(CreditTurnKind.PastStart), ReaderSession.CreditStep(credits, credits[0], true, false, 0, false, 6, 7, false));
        // From the last start page on to page 1.
        Assert.Equal(new CreditTurn(CreditTurnKind.Leaf, 0), ReaderSession.CreditStep(credits, credits[1], true, false, 0, false, 6, 7, true));
        // Back from page 1 once they show.
        Assert.Equal(new CreditTurn(CreditTurnKind.Credit, 1), ReaderSession.CreditStep(credits, null, true, false, 0, false, 6, 7, false));
        Assert.Equal(CreditTurn.None, ReaderSession.CreditStep(credits, null, false, false, 0, false, 6, 7, false));
        // The end: on from the last page, on to the end page, back to the last page.
        Assert.Equal(new CreditTurn(CreditTurnKind.Credit, 2), ReaderSession.CreditStep(credits, null, false, true, 6, false, 6, 7, true));
        Assert.Equal(new CreditTurn(CreditTurnKind.Leaf, 7), ReaderSession.CreditStep(credits, credits[2], false, true, 6, false, 6, 7, true));
        Assert.Equal(new CreditTurn(CreditTurnKind.Continue), ReaderSession.CreditStep(credits, credits[2], false, true, 6, false, 6, null, true));
        Assert.Equal(new CreditTurn(CreditTurnKind.Leaf, 6), ReaderSession.CreditStep(credits, credits[2], false, true, 6, false, 6, 7, false));
        Assert.Equal(new CreditTurn(CreditTurnKind.Credit, 2), ReaderSession.CreditStep(credits, null, false, true, 7, true, 6, 7, false));
    }

    static Leaf[] Leaves(int pages, bool end = false)
    {
        var leaves = Enumerable.Range(0, pages)
            .Select(i => new Leaf((uint)i, [new LeafPart((uint)i, PageKind.Story, new UnitRect(0, 0, 1, 1))]))
            .ToList();
        if (end)
            leaves.Add(new Leaf((uint)pages, [], EndPage: true));
        return [.. leaves];
    }

    // The session

    sealed class Open
    {
        public readonly List<Command> Sent = [];
        public readonly ReaderSession Reader;

        public Open(int pages = 5)
        {
            Reader = new ReaderSession { Send = Sent.Add, KeptHold = 60 };
            Reader.SurfaceChanged(new ReaderSurface(800, 1200, 1));
            Reader.Open("work-tidebell", "ch-1");
            Reader.Apply(new ReaderEvent.Opened(View("ch-1")));
            Reader.Apply(new ReaderEvent.Leaves(7, "ch-1", Leaves(pages, end: true)));
            Reader.Apply(new ReaderEvent.CreditPages(7, "ch-1", [Credit(0, false, 11), Credit(6, true, 22)], new Msg.Reading(new ReadingMsg.ReaderCreditPagesSkipped(2))));
        }

        public static ReaderView View(string chapter) => new(7, "work-tidebell", "Tidebell Cartographer", Medium.Manga,
            new ChapterRow(chapter, "Chapter 1", new ChapterReadState.Unread()), ReaderMode.Paged, ReadDirection.RightToLeft,
            new ReadPlace("work-tidebell", chapter));

        public List<ulong> Kept => [.. Sent.OfType<Command.Reader>().Select(c => c.V1).OfType<ReaderCommand.NotACreditPage>().Select(k => k.Fingerprint)];
    }

    [Fact]
    public void AKeepReachesTheEngineOnceWhenUndoRunsOut()
    {
        var open = new Open();
        var reader = open.Reader;
        reader.ShowCredit(reader.ShowCredits(false)!);
        Assert.Equal(ReaderLocal.NotACredit, reader.Notice?.Local);
        reader.ActLocal(ReaderLocal.NotACredit);
        Assert.Equal(ReaderLocal.Undo, reader.Notice?.Local);
        Assert.Equal("This page will show from now on", reader.Notice?.Text);
        Assert.True(reader.Held(11));
        Assert.Empty(open.Kept);
        // Counted at once: page 1 of 6 where 5 were.
        Assert.Equal((1, 6), reader.HeldPages(1, 5));
        // A turn sends nothing.
        reader.LeaveCredit();
        Assert.Empty(open.Kept);
        Assert.True(SpinWait.SpinUntil(() => open.Kept.Count > 0, 2_000));
        Assert.Equal([11ul], open.Kept);
        Assert.Null(reader.Notice);
        Assert.False(reader.KeepHeld);
    }

    [Fact]
    public void UndoSendsNothing()
    {
        var open = new Open();
        var reader = open.Reader;
        reader.ShowCredit(reader.ShowCredits(false)!);
        reader.ActLocal(ReaderLocal.NotACredit);
        Assert.True(reader.UndoKeep());
        // The page is a credit page again, with its own notice back.
        Assert.Equal(ReaderLocal.NotACredit, reader.Notice?.Local);
        Thread.Sleep(200);
        Assert.Empty(open.Kept);
        Assert.False(reader.UndoKeep());
    }

    [Fact]
    public void ASecondKeepSendsTheFirst()
    {
        var open = new Open();
        var reader = open.Reader;
        reader.Keep(11);
        reader.Keep(22);
        Assert.Equal([11ul], open.Kept);
        Assert.True(reader.Held(22));
    }

    [Fact]
    public void ClosingOrAnotherChapterSendsAHeldKeep()
    {
        var closing = new Open();
        closing.Reader.Keep(11);
        closing.Reader.Close();
        Assert.Equal([11ul], closing.Kept);
        // The keep goes before the close.
        var commands = closing.Sent.OfType<Command.Reader>().Select(c => c.V1).ToList();
        Assert.True(commands.FindIndex(c => c is ReaderCommand.NotACreditPage) < commands.FindIndex(c => c is ReaderCommand.Close));

        var next = new Open();
        next.Reader.Keep(22);
        next.Reader.OpenChapter("ch-2");
        Assert.Equal([22ul], next.Kept);
        Assert.Empty(next.Reader.Credits);
    }

    [Fact]
    public void TheNextLayoutPutsTheKeptPageInItsPlace()
    {
        var open = new Open();
        var reader = open.Reader;
        reader.Apply(new ReaderEvent.PageReady(7, "ch-1", 0, null, File("page-0.png")));
        reader.Apply(new ReaderEvent.PageReady(7, "ch-1", 3, null, File("page-3.png")));
        reader.ShowCredit(reader.ShowCredits(false)!);
        reader.Keep(11);
        reader.CommitKeep();
        Assert.Equal([11ul], open.Kept);
        reader.Apply(new ReaderEvent.Leaves(7, "ch-1", Leaves(6, end: true)));
        // The kept page was on screen: it stays, now page 1, and the others moved one on.
        Assert.Null(reader.CreditOn);
        Assert.Equal(0, reader.Current);
        Assert.Equal("credit-11.png", reader.Pages[new ReaderPageKey("ch-1", 0)].Path);
        Assert.Equal("page-0.png", reader.Pages[new ReaderPageKey("ch-1", 1)].Path);
        Assert.Equal("page-3.png", reader.Pages[new ReaderPageKey("ch-1", 4)].Path);
    }

    [Fact]
    public void ACreditPageOnScreenStaysWhileTheLayoutStillSkipsIt()
    {
        var open = new Open();
        var reader = open.Reader;
        reader.ShowCredit(reader.ShowCredits(true)!);
        reader.Apply(new ReaderEvent.Leaves(7, "ch-1", Leaves(5, end: true)));
        Assert.Equal(22ul, reader.CreditOn?.Fingerprint);
        Assert.Equal(4, reader.Current);
        reader.Apply(new ReaderEvent.CreditPages(7, "ch-1", [Credit(6, true, 22)], new Msg.Reading(new ReadingMsg.ReaderCreditPagesSkipped(1))));
        Assert.Equal(22ul, reader.CreditOn?.Fingerprint);
        // A layout that no longer skips it: the page under it shows.
        reader.Apply(new ReaderEvent.Leaves(7, "ch-1", Leaves(6, end: true)));
        Assert.True(SpinWait.SpinUntil(() => reader.CreditOn is null, 2_000));
    }

    // The group and Choose chapters

    static BookEditionRow Edition(bool chosen, params GroupChoice[] groups) =>
        new("source-a", "books.home.example", Chapters: 32, VersionLabel: groups[0].Label, Groups: groups, GroupChosen: chosen);

    [Fact]
    public void ThePickerOffersMostChaptersThenEachGroup()
    {
        var edition = Edition(false,
            new GroupChoice("Lanternlight Scans", 31, InUse: true),
            new GroupChoice("Paper Comets", 12, Language: "es"));
        Assert.True(GroupPicker.Offered(edition));
        Assert.Equal("Lanternlight Scans · most chapters", GroupPicker.Value(edition));
        var rows = GroupPicker.Rows(edition);
        Assert.Equal(new GroupPick(null, "Most chapters", "Now Lanternlight Scans", true), rows[0]);
        Assert.Equal(new GroupPick("Lanternlight Scans", "Lanternlight Scans", "31 of 32 chapters", false), rows[1]);
        Assert.Equal(new GroupPick("Paper Comets", "Paper Comets", "12 of 32 chapters · Spanish", false), rows[2]);

        var picked = Edition(true,
            new GroupChoice("Lanternlight Scans", 31),
            new GroupChoice("Paper Comets", 12, InUse: true, Language: "es"));
        Assert.Equal("Paper Comets · your pick", GroupPicker.Value(picked));
        Assert.False(GroupPicker.Rows(picked)[0].InUse);
        Assert.True(GroupPicker.Rows(picked)[2].InUse);
        Assert.False(GroupPicker.Offered(Edition(false, new GroupChoice("Lanternlight Scans", 31))));
    }

    [Fact]
    public void ARowNamesAnotherGroupInLowerCaseAfterADotAndItsLanguageByName()
    {
        var row = new ChapterRow("ch-9", "Chapter 9", new ChapterReadState.Read(),
            Line: [new Msg.Reading(new ReadingMsg.ReadingRowFromGroupIn("Bluefinch Group", "es"))], GroupLanguage: "es");
        Assert.Equal("From Bluefinch Group, Spanish", ReadingLines.Row(row));
        var after = row with
        {
            Line = [new Msg.Reading(new ReadingMsg.ReadingRowParts(2)), new Msg.Reading(new ReadingMsg.ReadingRowFromSource("Source A"))],
        };
        Assert.Equal("2 parts · from Source A", ReadingLines.Row(after));
    }

    static ChapterRow Row(int n, ChapterReadState? state = null, bool downloaded = false) =>
        new($"ch-{n}", $"Chapter {n}", state ?? new ChapterReadState.Unread(), Download: downloaded ? new ChapterDownloadState.Done() : null);

    static List<string> Ids(params int[] numbers) => [.. numbers.Select(n => $"ch-{n}")];

    [Fact]
    public void ChooseChaptersTicksTheNextTenFromThePlace()
    {
        // 1 to 4 read, 5 in progress: 5 to 14.
        var rows = Enumerable.Range(1, 20).Select(n => Row(n, n < 5 ? new ChapterReadState.Read() : n == 5 ? new ChapterReadState.InProgress(3, 20) : null)).ToList();
        Assert.Equal(Ids(5, 6, 7, 8, 9, 10, 11, 12, 13, 14), ChooseChapters.Start(rows));

        // 1 to 6 read, 2 unread behind; 8 downloaded: 7 and 9 to 17.
        rows = [.. Enumerable.Range(1, 20).Select(n => Row(n, n <= 6 && n != 2 && n != 3 ? new ChapterReadState.Read() : null, downloaded: n == 8))];
        Assert.Equal(Ids(7, 9, 10, 11, 12, 13, 14, 15, 16, 17), ChooseChapters.Start(rows));

        // In progress at 6 with 20 read after it still starts at 6.
        rows = [.. Enumerable.Range(1, 30).Select(n => Row(n, n == 6 ? new ChapterReadState.InProgress(1, null) : n <= 5 || n == 20 ? new ChapterReadState.Read() : null, downloaded: n == 8))];
        Assert.Equal(Ids(6, 7, 9, 10, 11, 12, 13, 14, 15, 16), ChooseChapters.Start(rows));

        // Three new chapters.
        Assert.Equal(Ids(1, 2, 3), ChooseChapters.Start([Row(1), Row(2), Row(3)]));

        // Unread, read, read: none after the place, so the first unread.
        Assert.Equal(Ids(1), ChooseChapters.Start([Row(1), Row(2, new ChapterReadState.Read()), Row(3, new ChapterReadState.Read())]));

        // 24 read of 30: 25 to 30.
        rows = [.. Enumerable.Range(1, 30).Select(n => Row(n, n <= 24 ? new ChapterReadState.Read() : null))];
        Assert.Equal(Ids(25, 26, 27, 28, 29, 30), ChooseChapters.Start(rows));
    }
}
