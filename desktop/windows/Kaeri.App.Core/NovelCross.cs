using System.Text;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// Where a novel's web view goes between the book's documents, and which of its pages keep a
/// place. Only the book's own documents keep one: a document of the book outside the reading
/// order (its contents page, a note) shows and keeps none, and a page that is not the book's,
/// such as an error's empty answer, keeps none and goes back once to the last place shown.
/// </summary>
public sealed class NovelCross
{
    /// <summary>What the view does.</summary>
    public abstract record Act
    {
        /// <summary>Keep the place the page said: a page of the book's own document.</summary>
        public sealed record Keep : Act;

        /// <summary>Show the page and its count, and keep no place.</summary>
        public sealed record Show : Act;

        /// <summary>Load this address, with its fragment.</summary>
        public sealed record Load(string Url) : Act;

        /// <summary>Past the last page of the last document: the end page.</summary>
        public sealed record End : Act;

        /// <summary>Nothing changes.</summary>
        public sealed record Stay : Act;
    }

    readonly IReadOnlyList<(string Url, bool Linear)> spine;
    readonly string root;
    bool recovering;

    /// <param name="spine">The book's documents in reading order, as the engine wrote their addresses.</param>
    /// <param name="entry">The first document's address, which names the open session's book.</param>
    /// <param name="opening">Where the chapter opened, with its fragment: what a page with no place shown yet goes back to.</param>
    public NovelCross(IReadOnlyList<(string Url, bool Linear)> spine, string entry, string opening)
    {
        this.spine = spine;
        Opening = opening;
        // kaeri-book://<session>/: the scheme, the session and the slash after it.
        var scheme = entry.IndexOf("://", StringComparison.Ordinal);
        if (scheme < 0)
        {
            root = entry;
        }
        else
        {
            var slash = entry.IndexOf('/', scheme + 3);
            root = slash < 0 ? entry + "/" : entry[..(slash + 1)];
        }
    }

    public string Opening { get; }

    /// <summary>The last place the book showed.</summary>
    public (int Spine, NovelPos Pos)? LastPlace { get; private set; }

    /// <summary>The address of the book's document shown, or null while none is.</summary>
    public string? Shown { get; private set; }

    /// <summary>The address being loaded, until its page is laid out or its load fails.</summary>
    public string? Pending { get; private set; }

    /// <summary>A load the view starts: its address, kept as the one pending.</summary>
    public string Load(string url)
    {
        Pending = Document(url);
        return url;
    }

    /// <summary>The spine document at <paramref name="address"/>, whatever its fragment and however its escapes are written.</summary>
    public int? Index(string? address)
    {
        if (address is null)
            return null;
        var path = Document(address);
        for (var i = 0; i < spine.Count; i++)
            if (Document(spine[i].Url) == path)
                return i;
        return null;
    }

    /// <summary>Whether <paramref name="address"/> names a file of the open book.</summary>
    public bool InBook(string? address) =>
        address is not null && Document(address).StartsWith(Document(root), StringComparison.Ordinal);

    /// <summary>A document's first layout, at the web view's <paramref name="address"/>, saying it is spine document <paramref name="spineIndex"/>.</summary>
    public Act Ready(string? address, int spineIndex, NovelPos pos)
    {
        Pending = null;
        var index = Index(address);
        if (index is { } i && i == spineIndex)
        {
            LastPlace = (i, pos);
            recovering = false;
            Shown = address;
            return new Act.Keep();
        }
        if (index is null && InBook(address))
        {
            recovering = false;
            Shown = address;
            return new Act.Show();
        }
        Shown = null;
        return Recover();
    }

    /// <summary>A turn on the page shown.</summary>
    public Act Turned(int spineIndex, NovelPos pos)
    {
        if (Index(Shown) is { } i && i == spineIndex)
        {
            LastPlace = (i, pos);
            return new Act.Keep();
        }
        return new Act.Show();
    }

    /// <summary>The last place the book showed, or where the chapter opened.</summary>
    string Back() =>
        LastPlace is { } last && last.Spine < spine.Count
            ? $"{spine[last.Spine].Url}#kaeri:pos={PosText(last.Pos)}"
            : Opening;

    Act Recover()
    {
        if (recovering)
            return new Act.Stay();
        recovering = true;
        return new Act.Load(Load(Back()));
    }

    /// <summary>Past the last page of the document shown: the next linear document, from the last place when the page is outside the reading order.</summary>
    public Act Next() => Step(1);

    /// <summary>Before the first page: the previous linear document, at its end.</summary>
    public Act Previous() => Step(-1);

    Act Step(int by)
    {
        if ((Index(Shown) ?? LastPlace?.Spine) is not { } current)
            return new Act.Load(Load(Back()));
        for (var i = current + by; i >= 0 && i < spine.Count; i += by)
            if (spine[i].Linear)
                return new Act.Load(Load(by < 0 ? spine[i].Url + "#kaeri:end" : spine[i].Url));
        return by > 0 ? new Act.End() : new Act.Stay();
    }

    /// <summary>A link the paginator reported: followed when it names a file of the book.</summary>
    public Act Link(string href, bool isInternal) =>
        isInternal && InBook(href) ? new Act.Load(Load(href)) : new Act.Stay();

    /// <summary>
    /// The main document at <paramref name="uri"/> did not come: the page shown stays with its
    /// keys, and with none of the book shown the chapter goes back to its last place, once.
    /// </summary>
    public Act Refused(string uri)
    {
        if (Pending is null || Pending != Document(uri))
            return new Act.Stay();
        Pending = null;
        return Index(Shown) is not null || InBook(Shown) ? new Act.Stay() : Recover();
    }

    /// <summary>Whether a main document's answer is refused: an error, or no body to show.</summary>
    public static bool Refuses(int status, bool empty) => status is < 200 or >= 300 || empty;

    public static string PosText(NovelPos pos) => $"{pos.Spine}/{string.Join('.', pos.Path)}/{pos.Offset}";

    /// <summary>The document an address names: without its fragment and with its escapes read.</summary>
    public static string Document(string address)
    {
        var hash = address.IndexOf('#');
        var path = hash < 0 ? address : address[..hash];
        if (!path.Contains('%'))
            return path;
        var bytes = new List<byte>(path.Length);
        for (var i = 0; i < path.Length; i++)
        {
            if (path[i] == '%' && i + 2 < path.Length && Uri.IsHexDigit(path[i + 1]) && Uri.IsHexDigit(path[i + 2]))
            {
                bytes.Add(Convert.ToByte(path.Substring(i + 1, 2), 16));
                i += 2;
            }
            else
            {
                bytes.AddRange(Encoding.UTF8.GetBytes(path[i].ToString()));
            }
        }
        return Encoding.UTF8.GetString([.. bytes]);
    }
}
