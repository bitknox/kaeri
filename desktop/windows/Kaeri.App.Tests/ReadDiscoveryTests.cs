using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;
using Medium = Kaeri.Engine.Core.Medium;

namespace Kaeri.App.Tests;

/// <summary>Reading's discovery: Search's From your sources as the store keeps it, and the words Home and Search show.</summary>
public class ReadDiscoveryTests
{
    static WorkTile Tile(string work, string title = "Lantern Orchard") => new(Work: work, Title: title, Medium: Medium.Manga);

    static ReadSearchQuery Words(string text, bool sourcesOnly = false, string? started = null) =>
        ReadingStore.EmptyQuery with { Text = text, SourcesOnly = sourcesOnly, Started = started };

    static ReadSourceGroup Group(string id, string name, ReadSourceState state, WorkTile[]? tiles = null, uint joined = 0) =>
        new(SourceId: id, SourceName: name, State: state, Tiles: tiles ?? [], Joined: joined, Kind: ReadSourceKind.Server);

    static ReadSourceSearch Found(ReadSearchQuery query, ReadAnilistState anilist, ReadSourceGroup[] groups, ReadSourceJoin[]? joins = null, bool done = false) =>
        new(Query: query, Anilist: anilist, Groups: groups, Joins: joins ?? [], Done: done);

    static ReadingStore Store(out List<Command> sent)
    {
        List<Command> commands = [];
        sent = commands;
        return new ReadingStore { Send = commands.Add };
    }

    [Fact]
    public void TheSourcesPartIsKeptForTheQueryAskedAndDroppedForAnOlderOne()
    {
        var store = Store(out _);
        store.RunSearch(Words("lantern"));
        store.Apply(new ReadingEvent.SourceSearch(Found(Words("lanter"), ReadAnilistState.Asking, [Group("folders", "Folders", new ReadSourceState.Asking())])));
        Assert.Null(store.Search.Sources);
        store.Apply(new ReadingEvent.SourceSearch(Found(Words("lantern"), ReadAnilistState.Asking, [Group("folders", "Folders", new ReadSourceState.Asking())])));
        Assert.NotNull(store.Search.Sources);
        Assert.True(store.Search.Loading);
    }

    [Fact]
    public void NewWordsLeaveTheOldSourcesBehindAndTheSameWordsKeepThem()
    {
        var store = Store(out _);
        store.RunSearch(Words("lantern"));
        store.Apply(new ReadingEvent.SourceSearch(Found(Words("lantern"), ReadAnilistState.Answered, [Group("srv", "Home library", new ReadSourceState.Nothing())], done: true)));
        store.RunSearch(Words("lantern", sourcesOnly: true));
        Assert.NotNull(store.Search.Sources);
        store.RunSearch(Words("orchard"));
        Assert.Null(store.Search.Sources);
    }

    [Fact]
    public void YourSourcesOnlyIsAnsweredByTheSourcesAndKeepsAniListsResultsForTheSameWords()
    {
        var store = Store(out var sent);
        store.RunSearch(Words("lantern"));
        store.Apply(new ReadingEvent.Search(Query: Words("lantern"), Page: 0, Tiles: [Tile("a:900101")], HasMore: true, OtherScope: null, Total: 40));
        store.RunSearch(Words("lantern", sourcesOnly: true));
        Assert.Contains(sent, c => c is Command.Reading { V1: ReadingCommand.Search { Query.SourcesOnly: true } });
        store.Apply(new ReadingEvent.SourceSearch(Found(Words("lantern", sourcesOnly: true), ReadAnilistState.Skipped, [Group("srv", "Home library", new ReadSourceState.Asking())])));
        Assert.False(store.Search.Loading);
        Assert.False(store.Search.HasMore);
        Assert.Single(store.Search.Tiles);

        store.RunSearch(Words("orchard", sourcesOnly: true));
        store.Apply(new ReadingEvent.SourceSearch(Found(Words("orchard", sourcesOnly: true), ReadAnilistState.Skipped, [Group("srv", "Home library", new ReadSourceState.Asking())])));
        Assert.Empty(store.Search.Tiles);
    }

    [Fact]
    public void TryAgainAsksOneSourceForTheWordsInForce()
    {
        var store = Store(out var sent);
        store.RunSearch(Words("lantern"));
        store.RetrySource("plugin-northwind");
        Assert.Contains(sent, c => c is Command.Reading { V1: ReadingCommand.RetrySourceSearch { SourceId: "plugin-northwind", Query.Text: "lantern" } });
    }

    [Fact]
    public void AnyTimeIsNoStartedFilterButYourSourcesOnlyIsAnotherQuery()
    {
        Assert.True(ReadingStore.SameQuery(Words("lantern", started: "any"), Words("lantern")));
        Assert.False(ReadingStore.SameQuery(Words("lantern", started: "12-months"), Words("lantern")));
        Assert.False(ReadingStore.SameQuery(Words("lantern", sourcesOnly: true), Words("lantern")));
        Assert.False(ReadDiscovery.StartedSet(Words("lantern", started: "any")));
        Assert.True(ReadDiscovery.StartedSet(Words("lantern", started: "3-years")));
    }

    [Fact]
    public void AResultASourceHasSaysWhereOnItsThirdLine()
    {
        var found = Found(Words("lantern"), ReadAnilistState.Answered, [], [
            new ReadSourceJoin(Work: "a:900101", SourceIds: ["srv"], SourceNames: ["Home library"]),
            new ReadSourceJoin(Work: "a:900102", SourceIds: ["srv", "folders"], SourceNames: ["Home library", "Folders"]),
        ]);
        Assert.Equal("In Home library", ReadDiscovery.Placed(found, "a:900101"));
        Assert.Equal("In 2 of your sources", ReadDiscovery.Placed(found, "a:900102"));
        Assert.Null(ReadDiscovery.Placed(found, "a:900103"));
        Assert.True(ReadDiscovery.AnyPlaced(found, [Tile("a:900103"), Tile("a:900102")]));
        Assert.False(ReadDiscovery.AnyPlaced(found, [Tile("a:900103")]));
    }

    [Fact]
    public void SourcesThatFoundNothingShareOneLineAndTheRestTakeARow()
    {
        var found = Found(Words("lantern"), ReadAnilistState.Answered, [
            Group("srv", "Home library", new ReadSourceState.Hits(), [Tile("s:1")], joined: 1),
            Group("folders", "Comics folder", new ReadSourceState.Nothing()),
            Group("demo", "Demo", new ReadSourceState.Nothing()),
            Group("plugin", "Northwind", new ReadSourceState.TimedOut(15)),
            Group("other", "Second shelf", new ReadSourceState.Hits(), [], joined: 2),
        ], done: true);
        var groups = found.Groups!;
        Assert.Equal(["srv", "plugin"], groups.Where(ReadDiscovery.Slotted).Select(g => g.SourceId));
        Assert.Equal("Nothing found in Comics folder and Demo.", ReadDiscovery.NothingLine(found));
        Assert.False(ReadDiscovery.SourcesFoundNothing(found));
        Assert.Equal(4, ReadDiscovery.Hits(found));
        Assert.Equal("No answer in 15 s.", ReadDiscovery.Reason(groups[3].State));
        Assert.Equal("6 from AniList, 4 from your sources. Northwind didn't answer.", ReadDiscovery.Finished(found, 6));
        Assert.Null(ReadDiscovery.Finished(found with { Done = false }, 6));
    }

    [Fact]
    public void ASourceSaysItsKindAndAHitHowMuchTheSourceHolds()
    {
        Assert.Equal("Folder", ReadDiscovery.KindWords(ReadSourceKind.Folder));
        Assert.Equal("Server", ReadDiscovery.KindWords(ReadSourceKind.Server));
        Assert.Null(ReadDiscovery.KindWords(null));
        var hit = Tile("s:1") with { Amount = new Msg.Reading(new ReadingMsg.ReadingVolumes(2)) };
        Assert.Equal("2 volumes", ReadDiscovery.Amount(hit));
        Assert.Equal("", ReadDiscovery.Amount(Tile("s:2")));
    }

    static readonly Msg Japan = new Msg.Reading(new ReadingMsg.ReadingOriginJapan());
    static readonly Msg LightNovel = new Msg.Reading(new ReadingMsg.ReadingKindLightNovel());
    static readonly Msg Manga = new Msg.Reading(new ReadingMsg.ReadingKindManga());
    static readonly Msg Releasing = new Msg.Reading(new ReadingMsg.ReadingStatusReleasing());
    static readonly Msg Finished = new Msg.Reading(new ReadingMsg.ReadingStatusFinished());
    static readonly Msg Chapters = new Msg.Reading(new ReadingMsg.ReadingChapters(112));

    [Fact]
    public void AnAniListResultSaysItsFormatAndOrigin()
    {
        // The engine's key, as it sends it; the words come from the facts, through the catalogue.
        var tile = Tile("a:900101") with { FormatLabel = "novel", Facts = [LightNovel, Releasing], Origin = Japan };
        Assert.Equal("Light novel · Japan", ReadDiscovery.Caption(tile));
        Assert.Equal("Light novel", ReadDiscovery.Caption(tile with { Origin = null }));
        Assert.Null(ReadDiscovery.Caption(Tile("a:900102")));
        Assert.Equal("Not found in your sources", ReadDiscovery.Caption(tile with { NotInSources = true }));
        Assert.Equal("Series from AniList", ReadDiscovery.Total(null));
    }

    [Fact]
    public void ARowTileSaysTwoFactsAsItsRowDraws()
    {
        var tile = Tile("a:900103") with { FormatLabel = "manga", Facts = [Manga, Finished, Chapters], Origin = Japan };
        Assert.Equal("Japan · Finished", ReadDiscovery.RowCaption(ReadRowKind.Trending, tile));
        Assert.Equal("Japan · Finished", ReadDiscovery.RowCaption(ReadRowKind.New, tile));
        Assert.Equal("Japan · 112 chapters", ReadDiscovery.RowCaption(ReadRowKind.Finished, tile));
        Assert.Equal("Manga · Finished", ReadDiscovery.RowCaption(ReadRowKind.Because, tile));
        Assert.Equal("Manga · Finished", ReadDiscovery.RowCaption(ReadRowKind.MoreGenre, tile));
        var releasing = tile with { Facts = [Manga, Releasing] };
        Assert.Equal("Japan · Releasing", ReadDiscovery.RowCaption(ReadRowKind.Finished, releasing));
        Assert.Equal("Manga · Releasing", ReadDiscovery.RowCaption(ReadRowKind.Trending, releasing with { Origin = null }));
        Assert.Equal("Not found in your sources", ReadDiscovery.RowCaption(ReadRowKind.Trending, tile with { NotInSources = true }));
    }

    [Fact]
    public void OneShotsFoldIntoMangaInTheFilterBar()
    {
        Assert.Equal(["manga"], ReadDiscovery.FoldedFormats(["manga", "one-shot"]));
        Assert.Equal(["one-shot"], ReadDiscovery.FoldedFormats(["one-shot"]));
        Assert.Empty(ReadDiscovery.FoldedFormats(null));
        // Manga takes one-shots with it while the engine lists them apart, and lets them go with it.
        Assert.Equal(["manga", "one-shot"], ReadDiscovery.ToggleFormat(null, "manga", oneShotsApart: true));
        Assert.Equal(["manga"], ReadDiscovery.ToggleFormat([], "manga", oneShotsApart: false));
        Assert.Equal(["novel"], ReadDiscovery.ToggleFormat(["manga", "one-shot", "novel"], "manga", oneShotsApart: true));
        var choices = new[] { new ReadFilterChoice(Key: "manga", Label: new Msg.Reading(new ReadingMsg.ReadingKindManga())), new ReadFilterChoice(Key: "one-shot", Label: new Msg.Reading(new ReadingMsg.ReadingKindManga())) };
        Assert.Equal(["manga"], ReadDiscovery.FormatChoices(choices).Select(c => c.Key));
    }

    [Fact]
    public void BecauseYouFinishedSetsTheTitleApart()
    {
        var heading = ReadDiscovery.Heading(new Msg.HomeBecause("Lantern Orchard"));
        Assert.Equal("Because you finished ", heading.Lead);
        Assert.Equal("Lantern Orchard", heading.Title);
        Assert.Equal("Because you finished Lantern Orchard", heading.ToString());
        Assert.Equal("30 of 212 shown", ReadDiscovery.Shown(30, 212));
        Assert.Equal("30 shown", ReadDiscovery.Shown(30, null));
    }
}
