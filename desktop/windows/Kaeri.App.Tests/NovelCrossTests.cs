using Kaeri.App;
using Kaeri.Engine.Model;
using Xunit;

namespace Kaeri.App.Tests;

/// <summary>
/// A light novel's turn from one document of the book into another, as the engine serves it and
/// as the novel view drives <see cref="NovelCross"/>: only the open session's addresses answer,
/// each document of the reading order carries its spine index in its settings, and a document
/// outside the reading order (the contents page, the notes) says the first. Made-up book.
/// </summary>
public class NovelCrossTests
{
    /// <summary>
    /// "Paper Lantern Vol 1": three parts in the reading order with spaces (and a comma) in their
    /// file names, a contents page and a notes document in the manifest but outside the spine.
    /// </summary>
    static readonly string[] Parts =
    [
        "Text/Part 1 of Paper Lantern.xhtml",
        "Text/Part 2 of Paper Lantern.xhtml",
        "Text/Part 3, Harbour Lights.xhtml",
    ];

    static readonly string[] Outside = ["nav.xhtml", "Text/Notes for Part 1.xhtml"];

    /// <summary>The address the engine writes: spaces escaped, the comma kept as it is.</summary>
    static string Url(string path) => "kaeri-book://7/" + path.Replace(" ", "%20");

    /// <summary>
    /// The book as the engine answers it: a part listed as missing, a file the book does not have
    /// and another session's address answer 404 with an empty body.
    /// </summary>
    sealed class Book
    {
        public readonly List<string> Missing = [];

        /// <summary>Parts served without the reader's settings, which the paginator reads as its defaults.</summary>
        public readonly List<string> Bare = [];

        public (int Status, string Body) Answer(string address)
        {
            var path = NovelCross.Document(address);
            const string root = "kaeri-book://7/";
            var name = path.StartsWith(root, StringComparison.Ordinal) ? path[root.Length..] : "";
            if (!(Parts.Contains(name) || Outside.Contains(name)) || Missing.Contains(name))
                return (404, "");
            var spine = Math.Max(Array.IndexOf(Parts, name), 0);
            var setup = Bare.Contains(name) ? "" : $"<meta name=\"kaeri-setup\" content=\"{{&quot;spine&quot;:{spine}}}\"/>";
            return (200, $"<html><head>{setup}</head><body><p>The ferry keeper counted the lanterns.</p></body></html>");
        }
    }

    /// <summary>
    /// The web view's part, played as the novel view plays it: each load is answered by the book;
    /// a refused document leaves the page as it was, and anything else is laid out, the paginator
    /// on an empty answer reading its defaults: spine 0, one page.
    /// </summary>
    sealed class Web
    {
        public readonly Book Book;
        public readonly NovelCross Cross;
        public string? Source;
        public readonly List<string> Loads = [];
        public readonly List<(uint Spine, int Page, int Pages)> Places = [];
        public bool Ended;

        public Web(Book book)
        {
            Book = book;
            Cross = new NovelCross([.. Parts.Select(p => (Url(p), true))], Url(Parts[0]), Url(Parts[0]));
            Navigate(Cross.Load(Url(Parts[0])));
        }

        void Run(NovelCross.Act act)
        {
            switch (act)
            {
                case NovelCross.Act.Load load:
                    Navigate(load.Url);
                    break;
                case NovelCross.Act.End:
                    Ended = true;
                    break;
            }
        }

        void Navigate(string url)
        {
            Loads.Add(url);
            Assert.True(Loads.Count < 20, "loads without end: " + string.Join(", ", Loads));
            var (status, body) = Book.Answer(url);
            if (NovelCross.Refuses(status, body.Length == 0))
            {
                Run(Cross.Refused(url));
                return;
            }
            Source = url;
            var at = body.IndexOf("&quot;spine&quot;:", StringComparison.Ordinal);
            var spine = at < 0 ? 0 : int.Parse(body[(at + 18)..body.IndexOf('}', at)]);
            var pages = body.Length == 0 ? 1 : 12;
            var page = url.EndsWith("#kaeri:end", StringComparison.Ordinal) ? pages - 1 : 0;
            var pos = new NovelPos((uint)spine, 0, [(uint)page]);
            Paged(Cross.Ready(Source, spine, pos), pos, page, pages);
        }

        /// <summary>As NovelView.Paged: a page that is not the book's keeps nothing.</summary>
        void Paged(NovelCross.Act act, NovelPos pos, int page, int pages)
        {
            if (act is NovelCross.Act.Load or NovelCross.Act.Stay)
                Run(act);
            else if (act is NovelCross.Act.Keep)
                Places.Add((pos.Spine, page, pages));
        }

        /// <summary>A turn inside the document shown, to <paramref name="page"/>.</summary>
        public void Turn(int spine, int page)
        {
            var pos = new NovelPos((uint)spine, 7, [(uint)page]);
            Paged(Cross.Turned(spine, pos), pos, page, 12);
        }

        public void End() => Run(Cross.Next());

        public void Start() => Run(Cross.Previous());

        public void Link(string href, bool isInternal = true) => Run(Cross.Link(href, isInternal));
    }

    [Fact]
    public void APartThatDoesNotComeLeavesThePageAndKeepsNoPlace()
    {
        var book = new Book();
        book.Missing.Add(Parts[1]);
        var web = new Web(book);
        web.Turn(0, 11);
        var before = web.Places.Count;
        web.End();
        Assert.Equal(Url(Parts[1]), web.Loads[^1]);
        Assert.Equal(Url(Parts[0]), web.Source);
        Assert.All(web.Places.Skip(before), p => Assert.True(p.Spine == 0 && p.Pages > 1, $"a place from the empty answer: {p}"));
        // The next key asks for part 2 again, from part 1, rather than a part further on.
        web.End();
        Assert.Equal(Url(Parts[1]), web.Loads[^1]);
        Assert.False(web.Ended);
    }

    [Fact]
    public void APageThatIsNotTheBooksKeepsNoPlaceAndGoesBackOnce()
    {
        var book = new Book();
        book.Bare.Add(Parts[1]);
        var web = new Web(book);
        web.Turn(0, 11);
        var before = web.Places.Count;
        book.Missing.Add(Parts[0]);
        web.End();
        Assert.Empty(web.Places.Skip(before));
        // Back to part 1's last place, once, and no further when that does not come either.
        Assert.Equal(new[] { Url(Parts[1]), Url(Parts[0]) + "#kaeri:pos=0/11/7" }, web.Loads.Skip(1));
    }

    [Fact]
    public void DocumentsOutsideTheSpineAndEscapedNamesCrossAndKeepNoPlace()
    {
        var web = new Web(new Book());
        web.Turn(0, 5);

        // A note outside the spine: shown, no place kept.
        var note = Url(Outside[1]) + "#note-2";
        web.Link(note);
        Assert.Equal(note, web.Loads[^1]);
        Assert.Equal((0u, 5, 12), web.Places[^1]);

        // Past the note's end: part 2, from the last place shown in part 1, never the end page.
        web.End();
        Assert.False(web.Ended);
        Assert.Equal(Url(Parts[1]), web.Loads[^1]);
        Assert.Equal(1u, web.Places[^1].Spine);

        // The contents page outside the spine, then part 3 by a link that escapes its comma.
        web.Link(Url(Outside[0]));
        Assert.Equal(Url(Outside[0]), web.Loads[^1]);
        Assert.Equal(1u, web.Places[^1].Spine);
        const string third = "kaeri-book://7/Text/Part%203%2C%20Harbour%20Lights.xhtml";
        web.Link(third);
        Assert.Equal(third, web.Loads[^1]);
        Assert.Equal(2u, web.Places[^1].Spine);

        // Back from part 3's first page: part 2's last page.
        web.Start();
        Assert.Equal(Url(Parts[1]) + "#kaeri:end", web.Loads[^1]);
        Assert.Equal((1u, 11, 12), web.Places[^1]);

        // Out of the book, and a file the book does not have: the page stays.
        var loads = web.Loads.Count;
        web.Link("https://elsewhere.example/", isInternal: false);
        web.Link("kaeri-book://8/Text/Part%202%20of%20Paper%20Lantern.xhtml");
        Assert.Equal(loads, web.Loads.Count);
        web.Link(Url("Text/Part 9.xhtml"));
        Assert.Equal(Url(Parts[1]) + "#kaeri:end", web.Source);
        Assert.Equal((1u, 11, 12), web.Places[^1]);
        web.End();
        Assert.Equal(Url(Parts[2]), web.Loads[^1]);
    }

    [Fact]
    public void AnEmptyAnswerAndAnErrorAreRefused()
    {
        Assert.True(NovelCross.Refuses(200, empty: true));
        Assert.True(NovelCross.Refuses(404, empty: true));
        Assert.False(NovelCross.Refuses(200, empty: false));
    }
}
